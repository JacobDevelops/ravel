#!/usr/bin/env python3
"""Reconcile standalone acquisition bytes without interpreting legacy mod metadata."""
import hashlib
import json
from pathlib import Path
import sys
import zipfile


def refuse(message):
    raise SystemExit("artifact acquisition refused: " + message)


def validate_manifest_references(manifest, references):
    fields = lambda row: (row["projectID"], row["fileID"], row.get("required"), row.get("isLocked"))
    actual = [fields(row) for row in manifest["files"]]
    expected = [(row["projectId"], row["fileId"], row["required"], row["locked"]) for row in references]
    if sorted(actual) != sorted(expected):
        refuse("manifest reference identity, required or isLocked differs")


def main() -> None:
    root, fixture_id, references_path, table_path, paths_path = sys.argv[1:]
    root = Path(root).resolve()
    local = (root / "tests/fixtures/local" / fixture_id).resolve()
    document = json.loads(Path(references_path).read_text())
    if document["fixture"]["id"] != fixture_id:
        refuse("reference ledger fixture identity differs")

    def resolve(path):
        candidate = (root / path.replace("${FIXTURE_ID}", fixture_id)).resolve()
        if not candidate.is_relative_to(local):
            refuse("artifact path escapes selected fixture")
        return candidate

    def pinned(pin):
        data = resolve(pin["path"]).read_bytes()
        if hashlib.sha256(data).hexdigest() != pin["sha256"]:
            refuse("pinned acquisition input SHA256 differs: " + pin["path"])
        return data

    pinned(document["fixture"]["archive"])
    manifest = json.loads(pinned(document["fixture"]["manifest"]))
    manifest_pairs = [(r["projectID"], r["fileID"]) for r in manifest["files"]]
    reference_pairs = [(r["projectId"], r["fileId"]) for r in document["references"]]
    acquired_pairs = [(r["projectId"], r["fileId"]) for r in document["acquiredArtifacts"]]
    if len(set(reference_pairs)) != len(reference_pairs) or sorted(manifest_pairs) != sorted(reference_pairs) or sorted(acquired_pairs) != sorted(reference_pairs):
        refuse("manifest, references and acquired file identities differ")
    validate_manifest_references(manifest, document["references"])
    rows, paths, seen = [], [], set()
    with zipfile.ZipFile(resolve(document["fixture"]["archive"]["path"])) as archive:
        bundled_entries = {r["archiveEntry"] for r in document["bundledArtifacts"]}
        if bundled_entries != {p for p in archive.namelist() if p.startswith("overrides/mods/") and p.endswith(".jar")}:
            refuse("bundled override ledger differs from pinned archive")
        for record in document["acquiredArtifacts"] + document["bundledArtifacts"]:
            name = record["artifact"]
            if Path(name).name != name or name in seen or any(c in name for c in "\t\r\n"):
                refuse("invalid or duplicate acquired artifact name")
            seen.add(name)
            path = resolve(record["path"])
            data = path.read_bytes()
            if path.name != name or record["status"] != "fetched" or len(data) != record["size"] or hashlib.sha256(data).hexdigest() != record["sha256"] or hashlib.sha1(data).hexdigest() != record["sha1"]:
                refuse("acquired artifact bytes differ: " + name)
            bundled = "archiveEntry" in record
            if bundled and archive.read(record["archiveEntry"]) != data:
                refuse("bundled bytes differ from pinned archive: " + name)
            rows.append("\t".join(("mod", name, record["sha256"], str(record.get("projectId", 0)), str(record.get("fileId", 0)), "bundled-override" if bundled else "reference-sha256")))
            paths.append(name + "\t" + str(path))
    actual = {p.resolve() for folder in (local / "jars", local / "extracted/overrides/mods") for p in folder.glob("*.jar")}
    if actual != {Path(row.split("\t")[1]) for row in paths}:
        refuse("acquisition ledger does not exactly cover local jars")
    Path(table_path).write_text("\n".join(sorted(rows)) + "\n")
    Path(paths_path).write_text("\n".join(sorted(paths)) + "\n")


if __name__ == "__main__":
    main()
