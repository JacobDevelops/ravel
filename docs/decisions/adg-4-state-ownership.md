# ADG-4 — State ownership

Status: accepted at M1-W5.

## Decision

The game keeps its state. Ravel duplicates nothing: a flow that needs the game's
values receives them as arguments and returns what it computed, and the Java side
writes the result back into the object it already owns.

When boundary traffic becomes a problem, the answer is fewer crossings — batch a
tick's worth of work into one call — not a mirror. A mirror is admissible only
where a measurement shows it is needed, and it carries the burden of proving it
cannot go stale.

## Context

Something has to hold entity position, velocity, and the rest while a mod
reasons about it. Three owners are possible: the JVM object, the loader, or both.
The roadmap asks for duplicated state and boundary traffic to be measured under
one representative flow, because those are the two costs that scale with the
number of mods and the number of entities.

## Evidence

Measured on `x86_64-linux`, hardware profile `linux-reference-v1`, through the
JNI boundary ADG-1 selected. Reproduce with:

```console
devenv tasks run ravel:spike-adg-4
```

Results live in [`evidence/m1/spikes/adg-4/`](../../evidence/m1/spikes/adg-4/)
and are checked by `ravel:verify-spikes`. One flow — read four doubles, compute
one value, store it — ran 1.28 M times under each owner.

| Owner | ns/flow p50/p95/p99 | Duplicated | Bytes per flow |
| --- | --- | --- | --- |
| Java owns it | 8.61 / 13.63 / 16.13 | 0 B | 40 B |
| Both mirror it | 7.83 / 9.55 / 12.53 | 32 B | 16 B |
| The loader owns it | 8.92 / 10.50 / 14.88 | 8 B | 24 B |

- **The crossing dominates; the bytes are noise.** All three medians land within
  1.09 ns of each other, between 7.83 and 8.92 ns, and all three sit just under
  the bare JNI boundary ADG-1 measured at 10.02 ns. Moving 40 bytes across the
  boundary costs no more than moving 16. At this size the argument list is free
  and the call is not.
- **The tails are wider than the medians and do not rank the same way.** The 99th
  percentiles spread 3.59 ns, from 12.53 to 16.13, and Java-owned state has the
  widest tail while having a median in the middle. Nothing in this decision rests
  on the tails, but "all three are equivalent" is a statement about medians only,
  and M2-W8's budget should watch the tail rather than assume it tracks.
- **Mirroring buys 24 bytes a flow and 0.8 ns.** That is the entire measured
  benefit of holding a second copy, and it is inside the run-to-run spread of the
  other two models.
- **Duplication is the only cost that is not noise.** 32 bytes per mirrored object
  is a real number once it is multiplied by the entities in a loaded world, and it
  buys a difference this measurement cannot resolve.
- **Every model fails closed.** Invalid input, a failing flow, an unavailable
  native runtime, a deliberate crash, and a hung trial each ended without a
  result, in all three models.

## Why Java keeps its state

Because the measurement says the bytes do not matter, the decision falls to the
things that are not measured in nanoseconds.

**One copy cannot be stale.** A mirror can, and nothing in the mechanism notices:
a stale mirror produces a wrong value with no failing call to attribute it to.
That is the worst shape a bug can have, and mirroring buys 0.8 ns to introduce
it.

**The game's own tools stay true.** With Java authoritative, a debugger, a crash
report, and any future in-game diagnostic all read the one copy. If the loader
owned the state, the game could not show the state it appears to own, and Ravel
would owe a way to read it — which is another boundary crossing this flow does
not count.

**It matches what Ravel is.** Ravel hosts itself inside the official game. The
game's world is the game's; a loader that takes ownership of it has to be right
about every field, forever, across versions.

The loader still owns *its own* state — mod instances, registries, load plans —
which is not the game's and never crosses back. That is not what this gate is
about.

## Consequences

- Hook signatures carry values, not references to state the loader keeps. The
  generated binding passes what the flow needs and returns what it produced.
- Reducing traffic means reducing crossings. M2-W8's regression budget should
  count crossings per tick, because that is the number this evidence says
  matters.
- A mirror needs a measurement and a staleness argument before it is added, and
  the ADR that adds one has to say how divergence is detected.
- `ravel-data` and `ravel-save` hold Ravel's own state, which is separate from
  this decision. Saved data is Ravel's by definition; the world's entities are
  not.

## Limits of this evidence

- The flow carries four doubles. Nothing here says what an object graph costs to
  move or to mirror, and that is the question M3 will actually face once real
  counterparts exist. What holds regardless is the ratio: a crossing costs about
  10 ns and a few more arguments cost nothing.
- Java-authoritative is measured shipping the state by value. The same model can
  be implemented by reading the object's fields from native code instead, which
  costs one extra crossing per field — about 8.6 ns each by this gate's own
  measurement, not the 10.02 ns ADG-1 records. The two figures are not
  interchangeable and the difference is instructive: ADG-1 measures a crossing
  reached through an agent-rewritten call site and a generated hook class, while
  these flows call the native method directly. That is why a flow carrying 40
  bytes measures 1.41 ns *less* than ADG-1's crossing carrying 16. Use ADG-1's
  figure for a hook, this gate's for a field read, and do not add one to the other
  expecting either to be the bare boundary. The Java-authoritative result carries
  this as a deviation, and its wording — "which ADG-1 prices at about ten
  nanoseconds each" — is the attribution corrected here. The other two results do
  not carry it. The file keeps its wording because correcting it means re-running
  the gate and moving numbers that are otherwise right.
- One thread, one object. Contention between the game thread and a mod thread
  over the same state is a real cost that this flow does not exercise, and it is
  an argument against mirroring rather than for it.
- The measurement includes the JNI crossing, so it is a floor for a real hook
  rather than a prediction of one.
- Trials ran on the development runtime 25.0.4 rather than the pinned Mojang
  25.0.1, recorded as a non-blocking deviation on all three results. Setting
  `RAVEL_SPIKE_JAVA` to the pinned runtime removes the difference. ADG-1 carries
  the same deviation, so the two gates' figures are comparable to each other and
  neither is a measurement of the runtime players will use.
