# Test fixtures

`public/` contains original, unrestricted inputs that may be committed and
redistributed with Ravel. Every fixture should state its purpose, author, and
licence.

`descriptors/` contains public metadata for restricted fixtures: authorized
source pages, exact versions, expected counts, and cryptographic hashes. It
never contains an archive or extracted third-party content.

`local/` is ignored except for its policy README. It is the only location in
this repository tree for reconstructed official Minecraft, ATM11, third-party
mod, private-world, or other restricted compatibility inputs. Tools may write
there only from authorized sources and must verify pinned hashes. Tests must
refer to those inputs through explicit environment or command-line paths and
must skip collection, not silently substitute content, when they are absent.

Never commit credentials, launcher tokens, official mod JARs, ATM-owned assets,
rights-sensitive notes, or derived restricted content.

## Pinned M0 fixture

The M0 compatibility oracle is CurseForge project `1148445`, file `8431787`:
All the Mods 11 `0.2.1` for Minecraft `26.1.2`, using NeoForge
`26.1.2.78`. Its client manifest contains exactly 235 project/file references.
The source archive and embedded manifest SHA-256 digests are recorded in
`descriptors/atm11-0.2.1.json`.

Reconstruct and verify it only under the ignored local boundary:

```console
cargo make ravel:fixture-reconstruct
cargo make ravel:fixture-verify
```

These commands use the descriptor's CurseForge CDN URL, admit the archive only
after its archive and manifest hashes match, and refuse an existing mismatched
archive instead of overwriting it.
