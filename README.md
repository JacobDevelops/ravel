# Ravel

Ravel is an experimental Minecraft: Java Edition mod loader and SDK for safe,
precompiled Rust mods. The first target is Minecraft 26.1.2 hosted by the
official game runtime. Ravel does not load Java mods and does not use NeoForge,
Fabric, Forge, Quilt, or another mod loader as its bootstrap.

The canonical repository is
[`tangled.org/jacobdevelops.tngl.sh/ravel`](https://tangled.org/jacobdevelops.tngl.sh/ravel).
Continuous integration uses the Tangled Spindle workflow under
`.tangled/workflows/`.

The repository is at the M0 scaffold stage. Interfaces and package formats are
not stable, and the runtime architecture remains subject to the evidence gates
in the roadmap.

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

The fast development gate is:

```console
devenv tasks run ravel:verify
```

The full environment gate used by Tangled also builds the pinned devenv and
flake checks:

```console
devenv tasks run ravel:verify-environment
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

Tangled Spindles currently provide the Linux gate. M0 through M10 are
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
