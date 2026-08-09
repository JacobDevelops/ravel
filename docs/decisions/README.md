# Architecture decisions

One record per architecture decision gate. Each one states what was decided, the
Linux evidence it rests on, what was rejected and why, and what the evidence does
not cover. Results live in [`evidence/m1/spikes/`](../../evidence/m1/spikes/) and
are checked by `devenv tasks run ravel:verify-spikes`.

A decision is not accepted because it is plausible. It is accepted because a
measurement exists, a rejection is explicit, and the limits are written down.

| Gate | Decision | Status |
| --- | --- | --- |
| [ADG-1](adg-1-jvm-entry.md) | JVM entry and hook mechanism | accepted (M1-W2); amended before M3-W3 for the sibling Java artifact, at M3-W3 for the measured one-loader model, and before M3-W4 for one receiver-aware adapter |
| [ADG-2](adg-2-mod-execution.md) | Rust mod execution and ABI | accepted (M3): interchangeable mods, distributed as source and compiled on the player's machine |
| [ADG-3](adg-3-bindings.md) | Minecraft bindings | accepted (M1-W4) |
| [ADG-4](adg-4-state-ownership.md) | State ownership | accepted (M1-W5); amended (M3-W0) to admit opaque handles |
| [ADG-5](adg-5-prism-integration.md) | Prism integration | accepted: two stock instance arguments, launch observed |
| [ADG-6](adg-6-configuration-ui.md) | Configuration schema and Ravel-owned UI | accepted before M3-W3: declared in data, drawn in Java |
| [ADG-7](adg-7-world-persistence.md) | World persistence adapter | accepted before M3-W4: one bounded Rust aggregate through vanilla saved data |

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
./jvm/harness/run-datafix-byte-preservation.sh # ADG-7 pinned fixer measurement
```

The last one is a script rather than a task because no task runs it: it additionally
needs a real Minecraft library root for the fixer's own dependencies, so it stays a
measurement ADG-7 records rather than a gate. ADG-7 says what that costs.

The shared measurement contract these tasks report through is
[`docs/architecture/m1-measurement-gate.md`](../architecture/m1-measurement-gate.md),
implemented by [`experiments/spike-harness`](../../experiments/spike-harness).
