---
name: jj-stacked-change-squashing
description: "Tangled-remote jj repos only: squash a path into the highest change owning it, recover botched batches with op restore, never squash in plain repos."
user-invocable: false
origin: auto-extracted
---

# jj stacked changes: squash targeting and recovery

**Extracted:** 2026-07-31
**Context:** jj repos whose remote is a tangled knot, where the review unit is a
stack of rewritten changes rather than appended commits.

## Applies only to stacked repos

```console
jj git remote list | grep -qE 'tangled\.org|knot' && echo stacked || echo plain
```

Override with `JJ_STACKED=1` / `JJ_STACKED=0` when detection is wrong: a knot on a
custom domain, or a tangled repo to be treated plainly.

**If this reports `plain`, do not squash at all.** Squashing is banned outside the
stacked workflow — revise by appending instead, and stop reading here. Everything
below assumes rewriting the stack is the intended operation.

## Problem

`jj squash --into <change> <paths>` replays that path's diff onto the target. If any
change *between* the target and `@` also edits that path, the replay has nothing to
attach to and jj conflicts **every change from the target upward**, not just the two
involved. `jj st` reports a clean working copy while `jj log` carries six CONFLICT
markers, so the first symptom is usually a stack-contract check failing for no
visible reason.

Three instances in one session, each a different file, none recognised until the
third: a shared `lib.rs` and a test fixture both owned mid-stack, and a doc edited by
two changes at once.

## Solution

Find who owns the path before choosing a target:

```console
jj log -r 'main..@' --no-graph -T 'change_id.short() ++ " " ++ description.first_line() ++ "\n"'
jj diff -r <change> --name-only | grep <path>
```

Squash into the **highest** change that already owns the path, not the one that feels
thematically right, and split a batch on ownership boundaries:

```console
jj squash --into <low>  schemas/evidence.schema.json evidence/m0/manifest.json
jj squash --into <high> tools/xtask/src/lib.rs tests/fixtures/complete-m1.json
```

Conflicts do not stop the rebase and do not appear in `jj st`, so check explicitly
after every batch:

```console
jj log -r 'main..@' --no-graph -T 'if(conflict, change_id.short() ++ " CONFLICT\n", "")'
```

## Recovery

`jj undo` reverses one operation and is useless after a multi-squash batch. Restore
the whole repo state instead — working copy included, so no edits are lost:

```console
jj op log --limit 10 --no-graph -T 'id.short() ++ "  " ++ description ++ "\n"'
jj op restore <snapshot-id-from-before-the-batch>
```

Pick the `snapshot working copy` entry immediately preceding the first squash. Then
redo one squash at a time with per-step status, never `set -e` chaining, so a failure
is visible rather than aborting the rest:

```console
squash() { t=$1; shift; jj squash --into "$t" "$@" -u >/dev/null 2>&1 && echo "ok $t" || echo "FAIL $t"; }
```

## Divergent change ids

If anything outside jj takes `.git/index.lock` mid-squash — a concurrent agent, an
IDE, a background `git` call — jj records the rewrite without completing it and two
commits end up sharing one change id. Same recovery. Detect with:

```console
jj log -r 'main..@' --no-graph -T 'if(divergent, change_id.short() ++ " DIVERGENT\n", "")'
```

## When to Use

- Before any `jj squash --into` targeting a change more than one step below `@`
- When a stack-contract or CI check fails on a stack `jj st` calls clean
- When a squash batch reports success but the next gate run fails inexplicably
- When two agents share one working copy
