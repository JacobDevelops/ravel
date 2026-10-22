# Stacked changes with jj

Ravel tracks history as a stack of changes, not a stream of commits. Every change carries a change id that
survives rewriting, so a change is a living unit of work: you keep editing it until it is right, and only its
commit hash moves. The canonical remote is GitHub; the repository's stacked-change
policy remains the same regardless of hosting.

Fixing work by appending another commit is a git workaround for immutable history. It does not apply here and
it destroys the property the review model depends on.

## Setup

```console
jj config set --repo git.write-change-id-header true
```

Change ids are then written into exported git commits, preserving change identity across rewrites.
`cargo make ravel:verify-stack` fails until this is set.

## Working

```console
jj new -m "feat: add package resolver"   # open a described change before you edit
jj st                                     # what is in the change right now
jj diff                                   # the change under review
jj commit <paths>                          # close it and open the next one, scoped to your files
```

Describe the change before you write code. The message is the plan; if you cannot write it, the change is not
one purposeful unit yet.

## Planning a work package

Plan the whole package first: build the stack out of empty changes, each already carrying its conventional
message, then fill them in from the bottom.

```console
jj new -m "feat: add package resolver"       # the first slot
jj new -m "test: cover resolver conflicts"   # the next slot on top of it
jj new -A <change> -m "docs: ..."            # insert a slot into the middle later
jj edit <change>                             # drop into a slot and write the code
```

An empty change with a conventional message is a plan and the gate accepts it. An empty change with no
message is litter; abandon it.

## Revising

The change id in the left column of `jj log` is the handle for everything below.

```console
jj squash --into <change>       # fold the work in @ into an earlier change
jj squash --into <change> <paths>
jj describe <change> -m "..."   # correct a message
jj edit <change>                # reopen a change; descendants rebase automatically
jj new -A <change> -m "..."     # insert a new change into the middle of the stack
jj abandon <change>             # drop a change that should not exist
jj evolog -r <change>           # every past version of this change
jj undo                         # reverse the last jj operation
```

A review comment on change `xskxqtyw` is answered by editing `xskxqtyw`. Never by a change called
`fix: address review`.

Conflicts live inside the change that carries them; jj does not stop the rebase. Run `jj resolve` on that
change and the descendants settle.

## Lane workspaces

Keep the default checkout reserved for coordination. Run integration in its own long-lived workspace,
not in the default checkout:

```console
tools/lane.sh up integrate main
tools/lane.sh up loader main --prewarm
tools/lane.sh land loader <integration-tip>
tools/lane.sh down loader
```

Run the script from a workspace other than the lane it manages. `up <name> <base>` creates
`../ravel-<name>` or rebases the existing lane onto the base without discarding its warm `target/`.
It links each local fixture directory from the primary checkout; fixture files are not linked.
The optional `--prewarm` runs `cargo build --workspace --all-targets --locked` through `numen run`.
Keep lanes between rounds. `down` snapshots the lane and refuses removal unless its working copy is empty
and its parent is already an ancestor of `main`. After integrating elsewhere, use `down <name> --force`
only when discarding that workspace is intentional; removal also discards its warm cache.

`land <name> [onto]` defaults to `main`, drops the lane's empty undescribed working-copy child,
rebases its stack, and opens a new child above the landed tip. It resolves package-manifest conflicts
only when all three versions differ exclusively in source/provenance digest fields. It regenerates
digests after each conflict-free change and folds them into that change. Other conflicts remain visible
and make the command fail; resolve those in the lane and rerun `land`.
Every non-empty change being landed must be described, even below empty or described children.
Rebasing preserves the stack's ancestry and moves descendant lanes along with it.
Each lifecycle command refreshes its target lane if another lane's rebase left that workspace stale.
Land lanes onto the integration tip and run the full gate in `ravel-integrate`; move the default
workspace only when integration is complete.

Regenerate all maintained package manifests with
`cargo run -q -p ravel-xtask -- package digests --all`; add `--check` to report every stale package
without writing. The single-package form, `package digests <directory> [--check]`, remains available.
Normal `mise exec -- cargo` builds inherit the host's mbx Cargo wrapper; coverage deliberately disables
mbx with `MBX_DISABLE=1`. A missing `RUSTC_WRAPPER` alone does not mean the Cargo wrapper is inactive.

## Stack shape

- One work package is a short stack, usually two to five changes. Each one compiles, passes the gate, and
  reads as a decision.
- Split when a change does two things (`jj split`), squash when two changes are one thing.
- Order the stack the way a reviewer reads it: contracts and types first, then behaviour, then docs and
  evidence.
- Reference changes by change id in notes, handoffs, and commit trailers. Hashes are round-specific.

## Submitting

Only with explicit approval in the current turn.

```console
jj git push -c @        # create the bookmark for this change and push the stack
jj git push             # push revised changes onto the same pull requests
```

On GitHub, create a pull request per reviewable bookmark, targeting the preceding bookmark for a
dependent change. Keep the same bookmarks when revising; GitHub does not map change ids to PRs.

## Not this

- `git commit`, `git rebase`, `git cherry-pick`, `git commit --amend`, `git push`.
- `git diff` or `git log` for reading history; use `jj diff`, `jj log`, `jj show`, `jj evolog`.
- Changes named `wip`, `fixup`, `squash!`, `address review`, or `review comments`.
- Undescribed changes anywhere behind `@`, empty or not. An empty change with a conventional message is a
  planned slot; an empty change with no message is litter, so abandon it (`jj abandon <change>`).

`cargo make ravel:verify-stack` checks the last two, plus unresolved conflicts, and runs as part of
`ravel:verify`.
