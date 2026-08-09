# ADG-4 — State ownership

Status: accepted at M1-W5; amended at M3-W0 to admit opaque handles. The
amendment is recorded under "Amendment: opaque handles" below and the
consequence it corrects is marked where it appears. Nothing about who owns state
changed.

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
  **Amended at M3-W0:** a hook may also carry an opaque handle to an object the
  *game* keeps, which is not what this bullet forbids. See "Amendment: opaque
  handles".
- Reducing traffic means reducing crossings. M2-W8's regression budget should
  count crossings per tick, because that is the number this evidence says
  matters.
- A mirror needs a measurement and a staleness argument before it is added, and
  the ADR that adds one has to say how divergence is detected.
- `ravel-data` and `ravel-save` hold Ravel's own state, which is separate from
  this decision. Saved data is Ravel's by definition; the world's entities are
  not.

## Amendment: opaque handles

Amended at M3-W0. A hook may carry an **opaque handle** to an object the game
owns: a reference the loader can hold for the duration of a call and hand back
to the game, and cannot read into. Everything above about who owns state stands.

### Why this is a correction rather than a reversal

The consequence this amends says hook signatures carry no "references to state
**the loader keeps**". A handle is a reference to state the *game* keeps, which
is the arrangement this gate selected rather than the one it refused. The
generator implemented the bullet as "no object may cross at all", which is
stricter than the sentence and stricter than the reasoning behind it.

Both concerns that decided this gate are untouched by a handle:

- **Staleness.** The argument was that a mirror "can be stale, and nothing in the
  mechanism notices". A handle cannot be stale, because it is not a second copy:
  it names the one authoritative object, and reading through it reads what the
  game currently holds. The failure mode the gate was built to prevent does not
  arise.
- **Duplication.** The number that decided the gate was 32 bytes per mirrored
  object multiplied by the entities in a loaded world. A handle costs a pointer
  and duplicates no field, so that multiplication does not happen.

A third argument, not made when this gate was decided, runs the same way. A
handle is **version-agnostic by construction**: it is opaque, so its shape is
identical in every Minecraft version. A copy is not — its field layout is
version-specific, so every game update would move it, and the remapping work
would land on whoever maintains the copy. Of the two, the handle is the one that
does not accumulate version debt.

### What forced the amendment

The reviewed inventory held six entries, all of them against
`net.minecraft.util.StringUtil` and `net.minecraft.util.SmoothDouble`, carrying
`(I)Z`, `(Ljava/lang/String;Z)Ljava/lang/String;`, `(DD)D` and `D`. Those are the
only shapes the generator admits, and it was not a coincidence: they are what is
left once every object is refused.

M3 resolved real targets for registration, ticks, commands, resource reload,
saved data and screens, recorded in
[`../architecture/m3-hook-targets.md`](../architecture/m3-hook-targets.md).
Essentially all of them carry game objects — `ResourceKey`, `BooleanSupplier`,
`CommandBuildContext`, `SavedDataType`, `GuiGraphicsExtractor`. Under the strict
reading Ravel can filter chat text and smooth a mouse delta and do nothing else,
which is not a loader.

### The residual risk is lifetime, not staleness

A handle names a live object, so the question it raises is *how long it stays
live*. A JNI reference is valid for the frame that produced it unless it is
promoted deliberately; a handle held past its frame is a use-after-free, and a
handle promoted and never released pins an object the collector wants.

That risk belongs to a safe wrapper whose type ties a handle to the call that
produced it, so a handle that outlives its frame does not compile. No measurement
here covers it, and it is a bounded and well-understood problem, which is what
distinguishes it from the open-ended divergence a mirror would have introduced.

**Built and proved at M3, so this is no longer owed work.**
`JavaHandle<'local>` in `crates/ravel-minecraft/src/lib.rs` holds
`&'local JObject<'local>` and a declared class name and nothing else, and three
`compile_fail` doctests beside it kill the three escapes by type rather than by
convention: returning a handle from the frame that produced it, sending one to a
thread, and an observer hoarding them. The wrapper's proof is where a reader
should look for it, which is why this record points at it rather than describing
work to be done.

### What is still forbidden

Reading through a handle and keeping the result is a mirror, and every rule above
still applies to it: it needs a measurement, a staleness argument, and an ADR
saying how divergence is detected. The amendment admits a reference that crosses;
it does not admit a copy that stays.

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
