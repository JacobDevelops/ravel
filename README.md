# Ravel

Ravel is an experimental Minecraft: Java Edition mod loader and SDK for safe,
natively compiled Rust mods, hosted by the official game runtime.
The default build target is 26.1.2; the forward proof workload is native behavioural parity
with SevTech: Ages 3.2.3 / Minecraft 1.12.2 on pinned JDK 27.
Historical M0–M5 records measured ATM11 0.2.1 (26.1.2); M6 measured ATM10 To the Sky 2.0.2 (1.21.1).
Forward proof is planned, not a claim of playable SevTech support. Ravel does not load Java mods
and does not use NeoForge, Fabric, Forge, Quilt, or another mod loader as its bootstrap.

Ravel exists to make a large modpack cost significantly less RAM and CPU than
the same pack costs on the Java mod ecosystem. Safe Rust mods compiled to native
code are how it gets there.

The canonical repository is
[`github.com/JacobDevelops/ravel`](https://github.com/JacobDevelops/ravel).
Continuous integration uses GitHub Actions in `.github/workflows/ci.yml`.

Interfaces and package formats are not stable. Ask
`cargo make ravel:verify-milestone <id>` whether a milestone is closed;
[`evidence/`](evidence/) records what it measured. The current mechanisms are described in
[`docs/architecture/overview.md`](docs/architecture/overview.md#mechanisms).

## Writing a mod

[`docs/guide/`](docs/guide/) is the author's documentation: the crate, the
package manifest, content, the lifecycle, state, what conformance is checked to
mean, and what does not work yet. The API reference is the `ravel-sdk` rustdoc,
and [`examples/`](examples/) holds the worked mods.

## Project contract

- [`SPEC.md`](SPEC.md) defines the product and capability contract.
- [`ROADMAP.md`](ROADMAP.md) defines the dependency-ordered implementation plan.
- [`docs/architecture/`](docs/architecture/) records boundaries that already
  follow from those documents. Architecture choices stay undecided until their
  named evidence gates pass.
- [`schemas/`](schemas/) contains the initial machine-readable interchange
  contracts.
- [`AGENTS.md`](AGENTS.md) is the working contract for coding agents, read by
  Codex directly and by Claude Code through the `CLAUDE.md` symlink.

## Development

Run `mise install` once, then activate mise in your shell or prefix commands with
`mise exec --`. Rustup provisions Rust 1.95.0 from `rust-toolchain.toml`.
The development and CI gate is:

```console
cargo make ravel:verify
```

Build an installed-package host and reconstruct or verify the pinned fixture with:

```console
cargo make ravel:host-build <instance> <cache> <minecraft>
cargo make ravel:fixture-reconstruct
cargo make ravel:fixture-verify
```

These tasks run `ravel-xtask`; the standalone `ravel-cli` has been removed.

Generate the selected Minecraft version's Java sources before building the JVM jars:

```console
cargo run -p ravel-xtask -- generate java
```

`RAVEL_MINECRAFT_VERSION` selects the version, as it does for Rust builds. Rust build
scripts write only their Cargo `OUT_DIR`; Java generation is an explicit step.
Generator inputs must pin the client jar, client mappings, and every declared
auxiliary artifact with a 64-digit hexadecimal SHA-256 hash; a byte-length-only
pin is not admitted.

Performance/resource capture and budget tooling are retired. Closed measurements
and their manifest-bound inputs remain historical evidence.

The underlying code checks can also be run individually:

```console
cargo fmt --all --check
cargo check --workspace --all-targets --all-features
cargo clippy --workspace --all-targets --all-features -- -D warnings
cargo nextest run --workspace --all-features
cargo test --workspace --doc
cargo deny check
```

GitHub Actions provides the Linux gate. Forward Linux completion is WP9;
historical M0–M10 criteria remain available for explicit replay.
Apple Silicon verification follows forward Linux completion and runs on an
actual `aarch64-darwin` host using
[`docs/runbooks/darwin-verification.md`](docs/runbooks/darwin-verification.md).

Ravel-maintained Rust and public mod-author code forbid unsafe code. Audited
dependencies may encapsulate unavoidable operating-system or JVM boundaries.
The scaffold gate discovers Cargo members under `crates`, `test-mods`,
`registry/mods` and `examples`; adding a member does not require a second
inventory in the gate.

## Fixtures and evidence

Credentials, launcher tokens, private-world data, and raw captures stay out of
source control. Fixture reconstruction is documented in
[`tests/fixtures/README.md`](tests/fixtures/README.md); recorded results live in
[`evidence/`](evidence/).

The integrity gate discovers milestone manifests under `evidence/mN/` rather than
keeping a second list of milestones. JSON predicates and recorded arithmetic run
in-process; JSON Schema validation remains delegated to `check-jsonschema`.
Integrity checks bind the records, not their closure verdict: use
`cargo make ravel:verify-milestone <id>` to ask whether a milestone is closed.

## Licence

Original Ravel loader and SDK code is provisionally dual-licensed under the
[Apache License 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT), at your
option. Third-party inputs retain their own licences.
