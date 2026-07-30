# Synthetic evidence fixtures

These original JSON documents exercise the public evidence schema and milestone
verifier. They contain synthetic names, timestamps, versions, results, and
repeated hexadecimal canaries rather than measurements from Minecraft, ATM11,
Prism, or a native target machine. Their `evidenceKind` is `synthetic`; the real
milestone entry point deliberately rejects them.

- `complete-m0.json` represents a structurally complete, successful M0 bundle.
- `incomplete-m0.json` deliberately violates M0 closure invariants.
- `invalid-hash-m0.json` contains a malformed SHA-256 value.

The fixtures are licensed under the repository's MIT OR Apache-2.0 terms. Their
content hashes are intentionally not evidence inputs: the tests load them
directly from the tracked source tree and verify their expected outcomes.
