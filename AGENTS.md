# Agent instructions

Ravel is a Minecraft: Java Edition 26.1.2 mod loader and SDK for natively compiled Rust mods. It hosts itself
inside the official game runtime and is not built on Fabric, NeoForge, Forge, or Quilt.

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

- Every maintained crate root carries `#![forbid(unsafe_code)]`, `ravel-bootstrap` included, and no
  exception exists. The table in
  [`docs/architecture/safety-boundary.md`](docs/architecture/safety-boundary.md) is empty and adding a row
  takes an accepted ADR plus an independent review, so assume the answer is no. The workspace denies
  `clippy::all` and
  `clippy::pedantic`. Fix causes: no `#[allow]`, no `#[ignore]`, no skipped tests, no loosened assertions to
  reach green. Everywhere else, unsafe operations belong only in audited dependencies behind narrow safe
  wrappers. Measurement experiments under `experiments/` may hold unsafe and are fenced out of the
  production graph.
- Respect the layering in [`docs/architecture/dependency-rules.md`](docs/architecture/dependency-rules.md).
  `ravel-core` depends on no other Ravel crate, the SDK exposes no bootstrap or JVM or version internals, and
  production crates never depend on `ravel-testkit`.
- ADG-1 to ADG-5 are accepted; read [`docs/decisions/`](docs/decisions/) before writing anything that
  touches JVM entry, mod execution, bindings, state ownership, or launcher integration, and follow the
  mechanism the ADR selected. ADG-6 has not started; do not commit code that picks its mechanism, and an
  empty boundary crate remains the correct placeholder for it.
- Never track mod JARs, official Minecraft or modpack assets, credentials, private worlds, or raw captures.
  Authorized local material goes in `tests/fixtures/local/`, which is ignored. `ravel-verify-scaffold`
  enforces this and will fail the gate.
- Adding or removing a crate or test mod means updating `CRATE_NAMES` or `TEST_MOD_NAMES` in
  [`tools/ravel-xtask/src/lib.rs`](tools/ravel-xtask/src/lib.rs) and the workspace dependency list in
  `Cargo.toml`.

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
- `devenv tasks run ravel:verify-stack` enforces this contract and runs inside `ravel:verify`.

## Scope

The repository is at the M1 architecture stage: the decisions are recorded and measured, and most crates are
still intentionally empty because M2 is what fills them. Implement what the
current work package asks and nothing beyond it; the roadmap order is a dependency order. Prefer the standard
library and dependencies already in `Cargo.lock`. A new dependency needs a reason and has to pass
`cargo deny check` for advisories, licences, bans, and sources.
