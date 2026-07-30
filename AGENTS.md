# Agent instructions

Ravel is a Minecraft: Java Edition 26.1.2 mod loader and SDK for precompiled Rust mods. It hosts itself
inside the official game runtime and is not built on Fabric, NeoForge, Forge, or Quilt.

This file is the single source of agent instructions. Codex reads `AGENTS.md`; Claude Code reads it through
the `CLAUDE.md` symlink. Edit this file only — never fork the two.

## Read before changing code

- [`SPEC.md`](SPEC.md) is the product and capability contract.
- [`ROADMAP.md`](ROADMAP.md) is the dependency-ordered plan and defines the evidence gate for each milestone.
- [`docs/architecture/`](docs/architecture/) records the boundaries that already hold: layering, safety, and
  the public/private split.
- Before writing in a crate, read a sibling crate and match its patterns instead of inventing new ones.

## Environment

Everything runs inside the devenv shell, which direnv loads automatically. If `cargo` is not on `PATH`,
run it as `devenv shell -- <command>`. The toolchain is pinned to Rust 1.95.0 and edition 2024; do not
add a rustup override or bump the channel. The first build needs network access for the cargo registry.

## Verify before claiming done

```console
devenv tasks run ravel:verify
```

That gate runs formatting, typecheck, lint, tests, coverage, dependency audit, and the scaffold contract.
The individual scripts are `ravel-format`, `ravel-typecheck`, `ravel-lint`, `ravel-test`, `ravel-coverage`,
`ravel-audit`, and `ravel-verify-scaffold`.

Formatting and lint cover Nix, shell, TOML, and Markdown as well as Rust, so a docs-only change still has to
pass `ravel-format`. Report the real output. Never state that a gate passed without running it.

## Hard rules

- Every maintained crate root carries `#![forbid(unsafe_code)]`, and the workspace denies `clippy::all` and
  `clippy::pedantic`. Fix causes: no `#[allow]`, no `#[ignore]`, no skipped tests, no loosened assertions to
  reach green. Unsafe operations belong only in audited dependencies behind narrow safe wrappers.
- Respect the layering in [`docs/architecture/dependency-rules.md`](docs/architecture/dependency-rules.md).
  `ravel-core` depends on no other Ravel crate, the SDK exposes no bootstrap or JVM or version internals, and
  production crates never depend on `ravel-testkit`.
- The ADG-1 to ADG-6 decisions (JVM entry, Rust execution and ABI, bindings, state ownership, Prism
  integration, configuration UI) are open. Do not commit code that picks a mechanism before its ADR is
  accepted. Empty boundary crates are the correct placeholder.
- Never track mod JARs, official Minecraft or modpack assets, credentials, private worlds, or raw captures.
  Authorized local material goes in `tests/fixtures/local/`, which is ignored. `ravel-verify-scaffold`
  enforces this and will fail the gate.
- Adding or removing a crate or test mod means updating `CRATE_NAMES` or `TEST_MOD_NAMES` in
  [`tools/ravel-xtask/src/lib.rs`](tools/ravel-xtask/src/lib.rs) and the workspace dependency list in
  `Cargo.toml`.

## Version control

- This is a jj repository. Use `jj st`, `jj diff`, `jj log`, `jj show`. Never use `git diff`, `git log`, or
  `git commit`.
- One logical change is one commit with a conventional message, scoped to the files you touched
  (`jj commit <paths>`) so a shared working copy does not sweep up other work.
- Never push and never create or merge a pull request without explicit approval in the current turn.
  Stopping at a local commit is completion.

## Scope

The repository is at the M0 scaffold stage, so most crates are intentionally empty. Implement what the
current work package asks and nothing beyond it; the roadmap order is a dependency order. Prefer the standard
library and dependencies already in `Cargo.lock`. A new dependency needs a reason and has to pass
`cargo deny check` for advisories, licences, bans, and sources.
