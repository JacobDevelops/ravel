# Agent instructions

Ravel is a Minecraft: Java Edition mod loader and SDK for natively compiled Rust mods. It hosts itself
inside the official game runtime and is not built on Fabric, NeoForge, Forge, or Quilt. It supports more than
one Minecraft version: a package declares the versions it runs on in `compatibility.minecraft` as an exact
version or a semver range, and per-version hooks, mappings and stubs live under `minecraft/versions/<version>/`.
The current target is `1.21.1`, the version the ATM10 To the Sky 2.0.2 proof workload runs on.
Both `1.21.1` and `26.1.2` have pinned version data.

This file is the single source of agent instructions. Codex reads `AGENTS.md`; Claude Code reads it through
the `CLAUDE.md` symlink. Edit this file only — never fork the two.

Project skills live once in [`.agents/skills/`](.agents/skills/) and are exposed to Claude Code through
per-skill symlinks in `.claude/skills/`. Adding a skill means adding its symlink; never copy the directory.

## Read before changing code

- [`SPEC.md`](SPEC.md) is the product and capability contract.
- [`ROADMAP.md`](ROADMAP.md) is the dependency-ordered plan and defines the evidence gate for each milestone.
- [`docs/architecture/`](docs/architecture/) records the boundaries that already hold: layering, safety, and
  the public/private split.
- Before writing in a crate, read a sibling crate and match its patterns instead of inventing new ones.

## Environment

Everything runs with mise: run `mise install` once, then activate mise in your shell or use
`mise exec -- <command>`. Rustup manages the undated nightly toolchain in `rust-toolchain.toml`,
with edition 2024. The first build needs network access for the cargo registry.

## Verify before claiming done

```console
cargo make ravel:verify
```

That gate runs formatting, typecheck, lint, tests, coverage, dependency audit, and the scaffold contract.
The individual tasks are `ravel:fmt`, `ravel:typecheck`, `ravel:lint`, `ravel:test`, `ravel:coverage`,
`ravel:audit`, and `ravel:verify-scaffold`.

Formatting and lint cover shell, TOML, and Markdown as well as Rust, so a docs-only change still has to
pass `cargo make ravel:fmt`. Report the real output. Never state that a gate passed without running it.

`ravel:verify` checks every milestone manifest for schema conformance, digest binding,
and the recorded arithmetic. It deliberately does not ask whether a milestone is closed.
Whether the independent review passed and whether any finding is blocking are
verdicts a manifest records rather than facts the gate derives, so a green `ravel:verify` says the evidence is
intact and says nothing about closure. Closure is `cargo make ravel:verify-milestone <id>`, and an open
milestone exits 1 there and is expected to. Ask that task which milestones are closed; do not trust a list
written here, and do not write one — a milestone closes and this sentence would go stale unnoticed.

Judge the gate by its exit status, not an individual task's test summary.

## Hard rules

- Every maintained crate root carries `#![forbid(unsafe_code)]`, `ravel-bootstrap` included, and no
  exception exists. The table in
  [`docs/architecture/safety-boundary.md`](docs/architecture/safety-boundary.md) is empty and
  stays empty unless deliberately changed. The workspace denies `clippy::all` and
  `clippy::pedantic`. Fix causes: no `#[allow]`, no `#[ignore]`, no skipped tests, no loosened assertions to
  reach green. Everywhere else, unsafe operations belong only in audited dependencies behind narrow safe
  wrappers. Measurement experiments under `experiments/` may hold unsafe and are fenced out of the
  production graph.
- Respect the layering in [`docs/architecture/dependency-rules.md`](docs/architecture/dependency-rules.md).
  `ravel-core` depends on no other Ravel crate, the SDK exposes no bootstrap or JVM or version internals, and
  production crates never depend on `ravel-testkit`.
- Follow the mechanisms in [`docs/architecture/overview.md`](docs/architecture/overview.md#mechanisms):
  configuration is declared in data and Ravel-owned screens are drawn in Java.
- `tests/fixtures/local/` is gitignored local material. Never track credentials.
- Adding or removing a crate or test mod means updating `CRATE_NAMES` or `TEST_MOD_NAMES` in
  [`tools/ravel-xtask/src/lib.rs`](tools/ravel-xtask/src/lib.rs) and, if another member depends on it,
  its `[workspace.dependencies]` entry in `Cargo.toml`.

## Version control

This is a jj repository, and the history is a stack of changes rather than a stream of commits. A change has
a stable change id that survives every rewrite, so the way to fix work is to edit the change that is wrong,
not to append another commit on top. Read [`docs/jj-stacked-changes.md`](docs/jj-stacked-changes.md) before
your first commit in a session; it holds the commands and the review loop.

- Use `jj st`, `jj diff`, `jj log`, `jj show`, `jj evolog`. Never use `git commit`, `git rebase`,
  `git cherry-pick`, or `git push`, and never read history with `git diff` or `git log`.
- A work package is a short stack of purposeful changes, each one reviewable on its own and each one
  described up front with a conventional message. Prefer three changes that each say something over ten that
  narrate your editing session.
- Revise in place. `jj squash --into <change>` folds new work into the change it belongs to, `jj describe`
  fixes a message, and `jj edit <change>` reopens a change so descendants rebase onto it automatically.
  Never write `wip`, `fixup`, or "address review" changes; that is the git workaround this repo does not use.
- Scope every squash or commit to the files you touched (`jj squash <paths>`, `jj commit <paths>`) so a
  shared working copy does not sweep up other work.
- Refer to work by change id, never by commit hash. Hashes change on every rewrite; change ids do not.
- Never push and never create or merge a pull request without explicit approval in the current turn.
  Stopping at a described local change is completion. When approval exists, pushing a revised change is the
  normal path, not a force push to apologise for.
- `cargo make ravel:verify-stack` enforces this contract and runs inside `ravel:verify`.

## Scope

Implement what the current work package asks and nothing beyond it; the roadmap order is a dependency
order. Prefer the standard library and dependencies already in `Cargo.lock`. A new dependency needs a
reason and has to pass `cargo deny check` for advisories, licences, bans, and sources.
