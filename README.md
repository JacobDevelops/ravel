# Ravel

Ravel is an experimental Minecraft: Java Edition mod loader and SDK for safe,
natively compiled Rust mods. It supports more than one Minecraft version and
currently targets Minecraft 1.21.1, hosted by the official game runtime; the only
version data pinned today is 26.1.2, which is what every closed milestone
measured. Ravel does not load Java mods and does not use NeoForge, Fabric,
Forge, Quilt, or another mod loader as its bootstrap.

Ravel exists to make a large modpack cost significantly less RAM and CPU than
the same pack costs on the Java mod ecosystem. Safe Rust mods compiled to native
code are how it gets there.

The canonical repository is
[`github.com/JacobDevelops/ravel`](https://github.com/JacobDevelops/ravel).
Continuous integration uses GitHub Actions in `.github/workflows/ci.yml`.

The repository is at M3, the credible loader prototype. A milestone is closed
when [`evidence/`](evidence/) holds its `manifest.json`, so that directory is
the current answer to what has passed rather than this sentence. Interfaces and
package formats are not stable, and which parts of the architecture are still
behind an open decision gate is recorded in
[`docs/decisions/README.md`](docs/decisions/README.md).

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

## Fixture and evidence policy

Only original, unrestricted fixtures and sanitized evidence belong in this
repository. Official Minecraft or modpack files, mod JARs, credentials, private
worlds, rights-sensitive research, and raw captures stay in ignored local
storage or the separate private compatibility workspace. See
[`tests/fixtures/README.md`](tests/fixtures/README.md),
[`evidence/README.md`](evidence/README.md), and
[`docs/architecture/public-private-boundary.md`](docs/architecture/public-private-boundary.md).

## Licence

Original Ravel loader and SDK code is provisionally dual-licensed under the
[Apache License 2.0](LICENSE-APACHE) or [MIT License](LICENSE-MIT), at your
option. Third-party inputs retain their own licences and must pass the
provenance and publication gates before distribution.
