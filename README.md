# Ravel

Ravel is an experimental Minecraft: Java Edition mod loader and SDK for safe,
natively compiled Rust mods. It supports more than one Minecraft version and
currently targets Minecraft 1.21.1, hosted by the official game runtime.
Both 1.21.1 and 26.1.2 have pinned version data. Ravel does not load Java mods
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
`mise exec --`. Rustup provisions the undated nightly in `rust-toolchain.toml`.
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

GitHub Actions provides the Linux gate. M0 through M10 are
Linux-first; Apple Silicon verification is deferred until Linux M10 has already
passed. When the post-Linux-M10 Mac phase begins, native verification runs on an
actual `aarch64-darwin` host using
[`docs/runbooks/darwin-verification.md`](docs/runbooks/darwin-verification.md).

Ravel-maintained Rust and public mod-author code forbid unsafe code. Audited
dependencies may encapsulate unavoidable operating-system or JVM boundaries.

## Fixtures and evidence

Credentials, launcher tokens, private-world data, and raw captures stay out of
source control. Fixture reconstruction is documented in
[`tests/fixtures/README.md`](tests/fixtures/README.md); recorded results live in
[`evidence/`](evidence/).

## Licence

Original Ravel loader and SDK code is provisionally dual-licensed under the
[Apache License 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT), at your
option. Third-party inputs retain their own licences.
