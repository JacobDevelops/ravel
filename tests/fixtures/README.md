# Test fixtures

`public/` contains small tracked test inputs.

`descriptors/` contains source pages, exact versions, expected counts, and
cryptographic hashes for fixture reconstruction.

`local/` holds ignored compatibility inputs. Reconstruction verifies pinned
hashes. Tests refer to these inputs through explicit environment or command-line
paths and skip collection, rather than silently substituting content, when they
are absent.

Never commit credentials or launcher tokens.

## Pinned M0 fixture

The M0 compatibility oracle is CurseForge project `1148445`, file `8431787`:
All the Mods 11 `0.2.1` for Minecraft `26.1.2`, using NeoForge
`26.1.2.78`. Its client manifest contains exactly 235 project/file references.
The source archive and embedded manifest SHA-256 digests are recorded in
`descriptors/atm11-0.2.1.json`.

Reconstruct and verify the local fixture:

```console
cargo make ravel:fixture-reconstruct
cargo make ravel:fixture-verify
```

These commands use the descriptor's CurseForge CDN URL, admit the archive only
after its archive and manifest hashes match, and refuse an existing mismatched
archive instead of overwriting it.
