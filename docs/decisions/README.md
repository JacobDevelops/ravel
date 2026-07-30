# Architecture decisions

One record per architecture decision gate. Each one states what was decided, the
Linux evidence it rests on, what was rejected and why, and what the evidence does
not cover. Results live in [`evidence/m1/spikes/`](../../evidence/m1/spikes/) and
are checked by `devenv tasks run ravel:verify-spikes`.

A decision is not accepted because it is plausible. It is accepted because a
measurement exists, a rejection is explicit, and the limits are written down.

| Gate | Decision | Status |
| --- | --- | --- |
| [ADG-1](adg-1-jvm-entry.md) | JVM entry and hook mechanism | accepted (M1-W2) |
| [ADG-2](adg-2-mod-execution.md) | Rust mod execution and ABI | measured (M1-W3); decision deferred to M3 |
| [ADG-3](adg-3-bindings.md) | Minecraft bindings | accepted (M1-W4) |
| [ADG-4](adg-4-state-ownership.md) | State ownership | accepted (M1-W5) |
| [ADG-5](adg-5-prism-integration.md) | Prism integration | accepted: two stock instance arguments, launch observed |
| ADG-6 | Configuration schema and UI | not started; due before M3 closes |

## Reproducing the evidence

Each gate has one task. All of them need the pinned local fixtures described in
[`tests/fixtures/local/README.md`](../../tests/fixtures/local/README.md), and none
of them authenticates or launches the game.

```console
devenv tasks run ravel:spike-adg-1
devenv tasks run ravel:spike-adg-2
devenv tasks run ravel:spike-adg-3
devenv tasks run ravel:spike-adg-4
devenv tasks run ravel:spike-adg-5
```

The shared measurement contract these tasks report through is
[`docs/architecture/m1-measurement-gate.md`](../architecture/m1-measurement-gate.md),
implemented by [`experiments/spike-harness`](../../experiments/spike-harness).
