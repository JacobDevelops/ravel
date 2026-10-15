# Fixture descriptors

Descriptors contain source metadata, expected versions, counts, and cryptographic
hashes. Reconstruction uses the publisher's download endpoint and writes to the
ignored `tests/fixtures/local/` directory.

## Where a runtime-inputs pin comes from

`runtime-inputs-1.21.1.json` pins the launcher-side inputs of the 1.21.1 target
SPEC.md names. Every field is copied from a publisher's own record, never from a
local build or a summary:

- `minecraft.metadata` is the entry for `1.21.1` in Mojang's version manifest v2
  (`https://piston-meta.mojang.com/mc/game/version_manifest_v2.json`). Its `sha1`
  is the manifest's own; the `sha256` is of the same bytes.
  `minimumLauncherVersion`, `assetIndex`, `javaComponent` and `javaMajorVersion`
  come from the per-version document that entry points at.
- `platforms[].java` is the `java-runtime-delta` entry for that platform in the
  Mojang runtime index, so the runtime pinned here is the one `javaComponent`
  requires rather than a runtime chosen separately.
- `platforms[].prism` names a released asset of one Prism Launcher tag. A pin
  never uses `releases/latest/download`: that URL moves, so the digest beside it
  would stop describing the file it names.

Re-pinning means fetching each of those again and replacing the whole set, not
editing one field. The digests are what make the descriptor checkable, so a value
that cannot be fetched is left unpinned and reported rather than guessed.

`macos-aarch64` is pinned because the schema requires both platforms. Native Mac
launch attestations are deferred, so that row is a pin without a launch behind it.
