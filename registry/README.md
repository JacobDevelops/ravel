# Ravel compatibility registry

This directory is part of the Ravel repository. It contains the rights-aware
compatibility inventory, counterpart specifications and implementations, and the
checks that maintain them. The counterpart crates under `mods/` are members of
the root Cargo workspace: they use the root `Cargo.toml`, `Cargo.lock`, SDK
workspace dependency, and target directory. There is no separate registry checkout
or workspace.

Paths stored in registry records are relative to this directory unless the record
explicitly identifies a root-repository path. Historical milestone evidence under
`../evidence/` retains its recorded layout and immutable material bindings.
Explicit machinery amendments maintain the live script digests and counters;
the M6 counterpart set and classified note counts are also checked live.
The live public-name collision scan excludes registry material and frozen root
evidence: historical failed-check transcripts are not current public-source
identifier use. Counterpart source-name checks remain separately enforced.

## Layout

- `inventory/<fixture>/` holds pinned references, dependency graphs, behaviour
  surface counts, rights verdicts, and counterpart plans.
- `provenance/` records fixture authority and rights decisions, plus counterpart
  provenance. Rights remain surface-specific; permission to observe behaviour
  does not imply permission to copy code, names, or assets.
- `specifications/` holds counterpart behaviour contracts.
- `mods/` contains the counterpart crates and their package manifests. Package
  source and provenance digests must match the files they declare.
- `fixtures/` holds synthetic checker fixtures, including malformed aliases and
  filenames containing newlines. These are not acquired game artifacts.
- `waves/` holds selection and capability-routing records and their Python
  producers.
- `proposals/` and `resolutions/` track SDK capability proposals and their
  dispositions. Proposal test cases describe capabilities and are not necessarily
  compiled workspace tests.
- `records/` holds current counterpart note-row counts.
- `checks/` contains verification scripts, vocabularies, and reader accounting.

## Local acquisition material

Acquired mod archives, extracted pack contents, pack-owned scripts and quests,
branding assets, official Minecraft files, credentials, worlds, and raw captures
must not be tracked here. The registry `.gitignore` reserves `artifacts/`,
`extracted/`, and `scratch/` for local work. The tracked-file boundary check is
scoped to `registry/`; it rejects restricted file kinds, machine-local paths,
credential material, and transcribed pack override structure. It preserves
filenames as NUL-delimited entries when obtaining the tracked set.

Fixture acquisition lives at the repository root's ignored
`tests/fixtures/local/`, not inside this directory. `checks/public-repo.sh`
derives both the registry and repository roots from its own location. It does
not discover sibling checkouts or accept an external repository override.
`RAVEL_FIXTURE_ID` selects the fixture, defaulting to `atm10-tts-2.0.2`.

The full fixture checks require the pinned archive and extracted manifest under
`tests/fixtures/local/<fixture>/`, plus the acquired instance beneath the local
launcher's application directory. The source-name check additionally requires
`tests/fixtures/local/m6-clean-room/src/` and its `fetch-manifest.txt`.
`RAVEL_CLEAN_ROOM_SRC`, `RAVEL_CLEAN_ROOM_MANIFEST`, and `RAVEL_WAVE` can select
explicit clean-room inputs and a specification wave. Missing required inputs
fail the fixture gate.

## Verification

From the repository root, inside the mise environment:

```console
cargo make ravel:registry-verify
```

This is the portable offline gate. It checks tracked-file hygiene, read-primitive
accounting, proposals, the foundation wave, counterpart note rows, counterpart
identifier and alias fixtures, package integrity, counterpart formatting,
Clippy, tests, and specification identifiers. It also validates inventory
provenance against the root schema, rights vocabulary and licence-status decisions, provenance record
consistency, and source-name record obligations. Source-name checks inspect
available clean-room trees and loudly report each absent tree without treating
it as a pass. Cargo operations select counterpart packages with `-p` in the root
workspace and use offline resolution; dependencies and toolchains must already
be installed.

**The offline gate does not verify acquired fixtures.** It prints a prominent
skip notice and names every omitted check. To require all fixture obligations:

```console
cargo make ravel:registry-verify-fixture
```

That runs the offline checks plus metadata accounting, reference reconciliation,
graphs, surfaces, rights, observed archive reads, provenance, plans, and
clean-room source-name checks. It fails rather than skips when local inputs are
missing. The corresponding script entrypoints, usable from any working
directory, are `registry/checks/verify.sh --offline` and
`registry/checks/verify.sh --fixture` (the paths here assume the repository root).

The scripts require Bash, Git (or `jj` when `.jj/` exists), Cargo, `jq`, Python 3, `taplo`, `unzip`,
`check-jsonschema`, SHA-1/SHA-256 tools, and ordinary GNU text utilities. A missing
tool fails the check that requires it; an offline skip is never reported as a
successful fixture verification.

The root evidence gate continues to validate historical milestone records.
Registry checks also recompute the M5 `boundArtifacts` digests and `primitiveScan`
figures. Material bindings remain immutable; changes to check machinery require
an explicit `machineryAmendments` entry and refreshed machinery digests/figures.
The M6 counterpart summary must match the current package set and each package's
classified note counts (`exercised`, `declarableOnly`, and `observableAreas`).
Current package integrity, declared read primitives, note-row records, and fixture
derivations remain independently enforced.

## Regenerating records

Verification does not regenerate tracked records.

Counterpart packages are maintained directly under `registry/mods/`, registered
in the root workspace inventory, and validated against the root package and
provenance schemas. There is no counterpart skeleton generator: package source
and provenance digests must bind actual reviewed implementation files, and
source access does not itself establish redistribution rights.

Run record producers intentionally:

```console
registry/checks/rebuild-references.sh
python3 registry/waves/select.py
python3 registry/waves/route.py
```

The reference producer reads the pinned manifest's project/file identity fields
and hashes the archive without opening it. Other inventory producers do inspect
acquired archives under stricter accounting: declarations, tables of contents,
and the leading bytes needed to identify nested archives, not decompiled class
bodies or copied resources. Nested libraries are discovered recursively, and an
independent metadata reader and emitted-row accounting check the extraction.
`checks/verify-reads.sh` reruns producers in a registry sandbox, reading local
fixture inputs in place under observation; it never copies acquired material
into tracked registry files.

The wave scripts derive their registry root from `__file__`, so their invocation
does not depend on the caller's working directory. Selection reads committed
plans and rights; routing reads committed surfaces and plans. Routing capability
verdicts are authored claims about the host, not proof of in-game behaviour.
Counterpart tests likewise do not establish gameplay integration; each
counterpart's host blockers state the remaining runtime obligations.
