#!/usr/bin/env python3
"""Reads the acquired artifacts' declared metadata with a real TOML parser.

This is the producer's reader for M5-W3..W6. Per archive it opens the mod
declaration a loader reads, the archive's table of contents, and the first four
bytes of each entry, and it reads them through `zipfile` and `tomllib`, both of
which are Python's own standard library. Nothing is decompiled and no resource,
data file or script is read beyond those four bytes, which are read to tell an
archive from a resource and for nothing else.

The reason it is a parser rather than a scanner is a defect M5-W7's review found:
a line scanner recognises the spellings its author thought of, and `mods.toml` is
TOML, so a mod may write a dependency block in any spelling the grammar admits.
Three artifacts in this fixture write one the scanner did not recognise, and
their declarations were dropped without a node, an edge, or a diagnostic. A
parser has no such blind spot, and the one case a parser still cannot interpret —
a dependency declaration whose shape is valid TOML but is not a dependency
declaration — is a hard failure here rather than a silent drop.

The output is the tab-separated record stream every model in this workspace
consumes. `checks/recheck-metadata.sh` re-reads the same artifacts with a
different reader and requires the two to agree, so a defect in this file is a
gate failure rather than an invisible omission.

Two readers catch a defect the two of them disagree about, and nothing else.
Round 2's review found the rest of that sentence: this reader parsed every
dependency a jar-in-jar library declares and kept only the library's identity,
the second reader did exactly the same, they agreed, and 58 declarations left no
node, no edge and no diagnostic. So the second output of this reader is a
ledger. Every declaration site it parses is written down with the number of rows
the parser produced, every row it emits without a parse behind it is written
down as synthesised, and every parsed row it declines to emit is written down
one by one with the reason. `checks/reader-accounting.sh` holds the emitted
stream to that ledger: a reader that parses N rows and emits fewer cannot exit
0, and it cannot be fixed by keeping quiet, because the ledger is written where
the parser produces the rows rather than where they are used.

Round 3's review found the same class one level further up, where the ledger
could not reach it: both readers discovered nested libraries from the jar-in-jar
manifest alone, so an artifact that ships a library and lists none was read as
shipping none, and the accounting balanced because the rows never existed. One
artifact in this fixture does exactly that. So discovery is no longer a reading
of the manifest at all. A nested library is an entry in a jar-in-jar directory,
found in the table of contents this same reader writes, at any depth; an archive
anywhere else is declined by name into the ledger with a tabled reason. The
manifest is not read. `checks/reader-accounting.sh` re-derives from the tables of
contents which sites had to exist and holds this reader to them, so which
declarations exist is accounted for and not only what they said.
"""

from __future__ import annotations

import io
import sys
import tomllib
import zipfile
from collections.abc import Iterator
from typing import Any, TextIO
from zipfile import ZipFile

DECLARATION_NAMES = ("META-INF/neoforge.mods.toml", "META-INF/mods.toml")

# Where a loader looks for the libraries an artifact ships inside itself. This
# is a location and not a list, which is the whole repair: an artifact cannot
# hide a library from this reader by leaving it out of a manifest, because no
# manifest is consulted. The manifest that does exist in these directories names
# libraries rather than being one, so it is the one entry not descended into.
JARJAR_DIRS = ("META-INF/jarjar/", "META-INF/jars/")
JARJAR_MANIFEST_LEAF = "metadata.json"

# A zip local file header, a central directory header, and the empty-archive end
# record. An archive outside a jar-in-jar directory is ship's cargo rather than a
# library, and it is recognised by its bytes rather than by its name so that an
# archive under an extension nobody thought of is still counted. Two shaderpacks
# in this fixture ship one under `.placebo`.
ARCHIVE_SIGNATURES = (b"PK\x03\x04", b"PK\x05\x06", b"PK\x07\x08")

# Why a discovered archive was not descended into. Like NESTED_REFUSALS below,
# `checks/reader-accounting.sh` holds this table and admits no other reason, and
# it checks the site against the reason rather than believing the label: a
# decline claiming an archive is cargo is refused if the archive sits in a
# jar-in-jar directory.
DECLINED_KINDS = {
    "CARGO": "the archive is not in a jar-in-jar directory, so the artifact ships it rather than loading it",
    "UNREADABLE": "the reader found the archive and could not open it, so nothing below it was read",
}

# What a nested library declares about itself that the containing artifact does
# not inherit. Each one is refused by name and written into the ledger row by
# row; `checks/reader-accounting.sh` holds this table and admits no other
# refusal, so a kind that carries a fact — a dependency, a mod identity — has no
# reason available to it and cannot leave the stream at all.
NESTED_REFUSALS = {
    "LOADER": "a nested library states its own loader, which is not the containing artifact's",
    "LICENSE": "a nested library states its own licence, which is not the containing artifact's",
    "MIXIN": "a nested library configures its own mixins, which are not the containing artifact's",
}


class Unreadable(Exception):
    """A declaration this reader will not guess at."""


class Parsed:
    """The rows a declaration produced, counted where each one is constructed.

    The ledger's `parsed` figure is this counter and never `len(rows)`. Round 3's
    review showed why: a defect inside the parser that drops a row after building
    it reduces both sides of the accounting equation together, and the equation
    balances on a reading that lost rows. Counting at construction closes that.

    What no counter of this kind can see is a parser that never builds the row —
    a `continue` before the row exists. Nothing the reader writes about itself
    can catch that, which is what the second reader in
    `checks/recheck-metadata.sh` is for, and it is stated here rather than left
    to be rediscovered.
    """

    def __init__(self) -> None:
        self.rows: list[tuple[str, ...]] = []
        self.produced = 0

    def make(self, *row: str) -> None:
        self.produced += 1
        self.rows.append(tuple(row))


def declaration(archive: zipfile.ZipFile) -> bytes | None:
    for name in DECLARATION_NAMES:
        try:
            return archive.read(name)
        except KeyError:
            continue
    return None


def field(table: dict[str, Any], name: str) -> Any:
    """A declared key, matched the way a loader matches it: without case."""
    wanted = name.lower()
    for key, value in table.items():
        if key.lower() == wanted:
            return value
    return None


def scalar(value: Any) -> str:
    if value is None:
        return ""
    if isinstance(value, bool):
        return "true" if value else "false"
    return str(value)


def dependency_rows(parsed: dict[str, Any], mod_ids: list[str], where: str, out: Parsed) -> None:
    """Every declared dependency, in every spelling the grammar admits.

    `[[dependencies.<modid>]]` and `[dependencies]` with an inline array value
    parse to the same structure, so both arrive here as a table of owner to list.
    A bare `[[dependencies]]` parses to a list instead; it names no owner, so the
    owner is the file's own mod, and a file that declares anything other than
    exactly one mod cannot supply one and fails.
    """
    declared = field(parsed, "dependencies")
    if declared is None:
        return

    if isinstance(declared, dict):
        groups = list(declared.items())
    elif isinstance(declared, list):
        if len(mod_ids) != 1:
            raise Unreadable(
                f"{where}: dependencies are declared with no owner and the file "
                f"declares {len(mod_ids)} mods, so no owner can be attributed"
            )
        groups = [(mod_ids[0], declared)]
    else:
        raise Unreadable(f"{where}: dependencies are a {type(declared).__name__}, not a table or an array")

    for owner, entries in groups:
        if isinstance(entries, dict):
            entries = [entries]
        if not isinstance(entries, list):
            raise Unreadable(f"{where}: dependencies.{owner} is a {type(entries).__name__}, not an array of tables")
        for entry in entries:
            if not isinstance(entry, dict):
                raise Unreadable(f"{where}: a dependencies.{owner} entry is a {type(entry).__name__}, not a table")
            dep_id = field(entry, "modId")
            if dep_id is None or scalar(dep_id) == "":
                raise Unreadable(f"{where}: a dependencies.{owner} entry declares no modId")
            out.make(
                "DEP",
                scalar(owner),
                scalar(dep_id),
                scalar(field(entry, "type")).lower(),
                scalar(field(entry, "versionRange")),
                scalar(field(entry, "side")).upper(),
            )


def mod_rows(parsed: dict[str, Any], where: str, out: Parsed) -> list[str]:
    """Read every mod identity and retain the owners for dependency attribution."""
    mods = field(parsed, "mods") or []
    if isinstance(mods, dict):
        mods = [mods]
    mod_ids = []
    for mod in mods:
        if not isinstance(mod, dict):
            raise Unreadable(f"{where}: a mods entry is a {type(mod).__name__}, not a table")
        mod_id = field(mod, "modId")
        if mod_id is None or scalar(mod_id) == "":
            raise Unreadable(f"{where}: a mods entry declares no modId")
        mod_ids.append(scalar(mod_id))
        out.make(
            "MOD",
            scalar(mod_id),
            scalar(field(mod, "version")),
            scalar(field(mod, "displayName")),
            scalar(field(mod, "logoFile")),
            "",
        )
    return mod_ids


def records(raw: bytes, where: str) -> Parsed:
    try:
        parsed = tomllib.loads(raw.decode("utf-8", "surrogateescape"))
    except tomllib.TOMLDecodeError as error:
        raise Unreadable(f"{where}: declaration is not valid TOML: {error}") from error

    out = Parsed()
    loader = field(parsed, "modLoader")
    if loader is not None:
        out.make("LOADER", scalar(loader), "", "", "", "")
    licence = field(parsed, "license")
    if licence is not None:
        out.make("LICENSE", scalar(licence), "", "", "", "")
    mod_ids = mod_rows(parsed, where, out)

    for mixin in field(parsed, "mixins") or []:
        if isinstance(mixin, dict):
            config = field(mixin, "config")
            if config is not None:
                out.make("MIXIN", scalar(config), "", "", "", "")

    dependency_rows(parsed, mod_ids, where, out)
    return out


def emit(out: TextIO, filename: str, row: tuple[str, ...]) -> None:
    out.write("\t".join((filename,) + tuple(row)) + "\n")


def parsed(ledger: TextIO, filename: str, site: str, rows: int) -> None:
    """How many rows the parser built for a site, counted as it built them."""
    ledger.write(f"PARSED\t{filename}\t{site}\t{rows}\n")


def site(ledger: TextIO, filename: str, where: str) -> None:
    """A nested archive this reader found and descended into.

    Written after the archive opens and never before. Round 4's review found
    this line above the open: a library neither reader could open was recorded
    `READ`, wrote no table of contents, and so silently removed the very listing
    the requirement induction uses to decide that anything below it had to be
    read. The reader was the only witness to its own failure. An archive that
    does not open is now a decline, and a decline in a jar-in-jar directory is a
    gate failure, so the induction ends in a diagnostic rather than in a claim.
    """
    ledger.write(f"SITE\t{filename}\t{where}\tREAD\t\n")


def declined(ledger: TextIO, filename: str, where: str, kind: str) -> None:
    """A nested archive this reader found and did not descend into."""
    ledger.write(f"SITE\t{filename}\t{where}\t{kind}\t{DECLINED_KINDS.get(kind, '')}\n")


def synthesised(ledger: TextIO, filename: str, site: str, why: str) -> None:
    """A row this reader emitted that no parsed declaration is behind."""
    ledger.write(f"SYNTH\t{filename}\t{site}\t1\t{why}\n")


def refused(ledger: TextIO, filename: str, site: str, row: tuple[str, ...]) -> None:
    """A parsed row this reader declines to emit, written out one row at a time.

    The kind is taken off the row rather than passed in, and the row itself is
    written beside it. A refusal is the one way out of the accounting equation,
    so what it is a refusal of has to be the row's own word for itself: a reader
    that refused a dependency under the licence line's reason would otherwise
    balance the books by mislabelling, and now records `DEP` against a reason
    that admits no such thing.
    """
    kind = row[0]
    reason = NESTED_REFUSALS.get(kind, "")
    ledger.write(f"REFUSED\t{filename}\t{site}\t{kind}\t{reason}\t{'|'.join(row)}\n")


def in_jarjar(name: str) -> bool:
    return any(name.startswith(prefix) for prefix in JARJAR_DIRS)


def is_archive(archive: zipfile.ZipFile, info: zipfile.ZipInfo, where: str) -> bool:
    if info.file_size < 4:
        return False
    try:
        with archive.open(info) as handle:
            return handle.read(4) in ARCHIVE_SIGNATURES
    except (zipfile.BadZipFile, OSError, EOFError) as error:
        raise Unreadable(f"{where}: an entry could not be read far enough to tell what it is: {error}") from error


def nested_sites(archive: zipfile.ZipFile, where: str) -> Iterator[tuple[str, str | None]]:
    """Every nested archive this archive holds, from its table of contents.

    No manifest is consulted. A library is an entry in a jar-in-jar directory,
    which is where a loader looks for one, so an artifact that ships a library
    and lists it nowhere is read exactly like one that lists it. Everything else
    is an archive only if its bytes say so, and is declined by name rather than
    read, because nothing outside those directories is loaded.
    """
    for info in archive.infolist():
        if info.is_dir():
            continue
        name = info.filename
        if in_jarjar(name):
            if name.rsplit("/", 1)[-1] == JARJAR_MANIFEST_LEAF:
                continue
            yield name, None
        elif is_archive(archive, info, where):
            yield name, "CARGO"


def descend(
    archive: ZipFile, filename: str, prefix: str,
    meta: TextIO, ledger: TextIO, nested_entries: TextIO,
) -> None:
    for name, decline in nested_sites(archive, f"{filename}!{prefix}" if prefix else filename):
        path = f"{prefix}!{name}" if prefix else name
        where = f"{filename}!{path}"
        if decline is not None:
            declined(ledger, filename, where, decline)
            continue
        read_nested(archive, name, path, filename, meta, ledger, nested_entries)


def read_nested(
    container: ZipFile, name: str, path: str, filename: str,
    meta: TextIO, ledger: TextIO, nested_entries: TextIO,
) -> None:
    """One nested library, and everything it ships in turn.

    The walk recurses rather than stopping at the first level, because a library
    a library ships is loaded on the same terms and round 3's review found one in
    this fixture. The site path carries every container it sits inside, so a
    record names where the declaration was found and not merely its leaf name.
    """
    where = f"{filename}!{path}"
    inner = None
    unreadable = None
    try:
        blob = container.read(name)
    except KeyError:
        blob = None
        unreadable = "the table of contents names a nested jar the archive does not hold"
    if blob is not None:
        try:
            inner = zipfile.ZipFile(io.BytesIO(blob))
        except zipfile.BadZipFile:
            unreadable = "the nested jar is not an archive"
    if inner is None:
        # Not a site this reader read. Saying so is the whole point: the
        # placeholder row still balances the accounting, and the disposition
        # tells reader-accounting.sh that a library a loader would have loaded
        # was left unread, which it refuses.
        declined(ledger, filename, where, "UNREADABLE")
        emit(meta, filename, ("NESTED", path, "", "", "", ""))
        synthesised(ledger, filename, where, unreadable)
        return

    site(ledger, filename, where)
    with inner:
        for entry in inner.namelist():
            nested_entries.write(f"{filename}\t{path}\t{entry}\n")

        inner_raw = declaration(inner)
        if inner_raw is None:
            emit(meta, filename, ("NESTED", path, "", "", "", ""))
            synthesised(ledger, filename, where, "the nested jar declares no mod")
        else:
            # A nested library's declaration is a declaration. Its dependencies
            # are the containing artifact's to carry, because the library ships
            # inside it and loads with it; what it says about its own loader,
            # licence and mixins is its own and is refused by name rather than
            # attributed to the artifact around it.
            emit_nested_declaration(inner_raw, where, filename, path, meta, ledger)

        descend(inner, filename, path, meta, ledger, nested_entries)


def emit_nested_declaration(
    raw: bytes, where: str, filename: str, path: str, meta: TextIO, ledger: TextIO,
) -> None:
    out = records(raw, where)
    parsed(ledger, filename, where, out.produced)
    wrote = False
    for row in out.rows:
        if row[0] == "MOD":
            emit(meta, filename, ("NESTED", path, row[1], row[2], "", ""))
            wrote = True
        elif row[0] == "DEP":
            emit(meta, filename, ("NESTEDDEP",) + tuple(row[1:]))
        elif row[0] in NESTED_REFUSALS:
            refused(ledger, filename, where, row)
        else:
            raise Unreadable(f"{where}: the reader has no disposition for a {row[0]} record")
    if not wrote:
        emit(meta, filename, ("NESTED", path, "", "", "", ""))
        synthesised(ledger, filename, where, "the nested declaration names no mod")


def read_mod(
    path: str, filename: str, meta: TextIO, entries: TextIO,
    ledger: TextIO, nested_entries: TextIO,
) -> None:
    with zipfile.ZipFile(path) as archive:
        for name in archive.namelist():
            entries.write(f"{filename}\t{name}\n")

        raw = declaration(archive)
        if raw is None:
            emit(meta, filename, ("NOMETA", "", "", "", "", ""))
            synthesised(ledger, filename, filename, "the archive declares no mod")
        else:
            out = records(raw, filename)
            parsed(ledger, filename, filename, out.produced)
            for row in out.rows:
                emit(meta, filename, row)

        descend(archive, filename, "", meta, ledger, nested_entries)


def read_shaderpack(
    path: str, filename: str, meta: TextIO, entries: TextIO,
    ledger: TextIO, nested_entries: TextIO,
) -> None:
    """A shaderpack is catalogued rather than read.

    It declares no mod, so nothing here parses one. What it ships is still walked
    on the same terms as a mod's, because "which archives does this artifact
    carry" is a question the accounting asks of every acquired artifact and not
    only of the ones that answer it interestingly. Two shaderpacks in this
    fixture ship an archive under a `.placebo` extension.
    """
    with zipfile.ZipFile(path) as archive:
        for name in archive.namelist():
            entries.write(f"{filename}\t{name}\n")
        descend(archive, filename, "", meta, ledger, nested_entries)


def read_resourcepack(
    path: str, filename: str, meta: TextIO, entries: TextIO,
    ledger: TextIO, nested_entries: TextIO,
) -> None:
    """Resource packs declare no mods but still belong in the artifact census.

    This pack pins SmoothFont and ClassicMekTextures. Their archive listings
    and nested sites are accounted on the same terms as the other artifacts.
    """
    with zipfile.ZipFile(path) as archive:
        for name in archive.namelist():
            entries.write(f"{filename}\t{name}\n")
        descend(archive, filename, "", meta, ledger, nested_entries)


def read_table(path: str) -> list[list[str]]:
    """Read the workspace-owned artifact TSV, ignoring blank lines."""
    with open(path, encoding="utf-8") as handle:
        return [line.rstrip("\n").split("\t") for line in handle if line.rstrip("\n")]


def read_artifacts(
    rows: list[list[str]], instance_root: str,
    meta: TextIO, entries: TextIO, ledger: TextIO, nested_entries: TextIO,
) -> None:
    # Keep the historical mod, shaderpack, resourcepack emission order.
    for kind, reader in (
        ("mod", read_mod), ("shaderpack", read_shaderpack), ("resourcepack", read_resourcepack),
    ):
        for row in rows:
            if row[0] != kind:
                continue
            filename = row[1]
            path = f"{instance_root}/{kind}s/{filename}"
            reader(path, filename, meta, entries, ledger, nested_entries)


def main(argv: list[str]) -> int:
    if len(argv) != 7:
        print(
            "usage: modmeta.py <artifact-tsv> <instance-root> <meta-tsv> <entries-tsv>"
            " <ledger-tsv> <nested-entries-tsv>",
            file=sys.stderr,
        )
        return 2
    artifact_tsv, instance_root, meta_path, entries_path, ledger_path, nested_path = argv[1:]
    rows = read_table(artifact_tsv)
    if not rows:
        print("no acquired artifacts to read declared metadata from", file=sys.stderr)
        return 1

    try:
        with (
            open(meta_path, "w", encoding="utf-8") as meta,
            open(entries_path, "w", encoding="utf-8") as entries,
            open(ledger_path, "w", encoding="utf-8") as ledger,
            open(nested_path, "w", encoding="utf-8") as nested_entries,
        ):
            read_artifacts(rows, instance_root, meta, entries, ledger, nested_entries)
    except Unreadable as error:
        print(f"declared metadata could not be read: {error}", file=sys.stderr)
        return 1

    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
