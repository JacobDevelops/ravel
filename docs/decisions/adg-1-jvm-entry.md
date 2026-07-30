# ADG-1 — JVM entry and hook mechanism

Status: accepted at M1-W2. It needs no `SPEC.md` §3.3 amendment and no unsafe
exception: the premise that an exported symbol cannot be encapsulated turned out to
be false, and the correction is recorded under "Unsafe surface" below.

## Decision

Ravel enters the official game process as a `java.lang.instrument` premain
agent, rewrites its hook targets with the runtime's own `java.lang.classfile`
API, and binds hook callbacks to Rust through an exported, JNI-mangled symbol
resolved by `System.loadLibrary`.

Prism launches this with stock JVM arguments: one `-javaagent` and one
`-Djava.library.path`. Nothing else about the launch changes, so the runtime,
garbage collector, module configuration, and class path stay exactly what the
launcher and Mojang chose.

## Context

The loader has to reach code inside a JVM that Ravel does not start, rewrite
methods in the pinned 26.1.2 client, and call Rust from those methods often
enough that per-call cost matters. Four mechanisms could do it, and the question
was which one costs least in call time, memory, startup, unsafe surface, and
launcher integration.

## Evidence

Measured on `x86_64-linux`, hardware profile `linux-reference-v1`, against the
pinned Minecraft 26.1.2 client jar. Reproduce with:

```console
devenv tasks run ravel:spike-adg-1
```

Results live in [`evidence/m1/spikes/adg-1/`](../../evidence/m1/spikes/adg-1/)
and are checked by `ravel:verify-spikes`. Every candidate ran the same workload
through the same trial class, so the difference between rows is the mechanism.

| Candidate | ns/call p50/p95/p99 | Classes | Retained RSS | Startup | Installed | Unsafe ops | Prism |
| --- | --- | --- | --- | --- | --- | --- | --- |
| Control, no hook | 0.47 / 2.83 / 2.98 | 1485 | 59.7 MiB | 21 ms | 8 KiB | 0 | not evaluated |
| Agent, callback stays in Java | 0.63 / 3.45 / 5.80 | 1641 | 63.7 MiB | 36 ms | 17 KiB | 0 | stock JVM arguments |
| Agent, exported JNI symbol | 10.02 / 12.53 / 12.53 | 1641 | 63.8 MiB | 36 ms | 719 KiB | 0 | stock JVM arguments |
| Rust-hosted VM, `RegisterNatives` | 10.02 / 12.36 / 12.38 | 1639 | 64.9 MiB | 36 ms | 964 KiB | 2 | stock wrapper command |

Each trial reports whether it actually rewrote the pinned target, and the runner
refuses to record a result that disagrees with the mechanism it claims to
measure. The retained class counts corroborate it: only the transforming
candidates carry the extra classes: 156 for the two agent candidates and 154 for
the hosted VM, which transforms through the same agent jar.

What the numbers say:

- **The boundary costs about 9.5 ns a call, and the binding does not.** 10.02 ns
  against the control's 0.47 ns. The two native candidates have the same median to
  three decimal places, so choosing between an exported symbol and
  `RegisterNatives` is not a performance decision.
- **Staying inside the JVM is about sixteen times cheaper, but it is not free.**
  The injected Java callback's median is 0.63 ns against the control's 0.47 ns, a
  difference below what a batch mean can resolve per call. Its 99th percentile is
  not: 5.80 ns against 2.98 ns, so the injected callback has a tail the control
  does not, and it is still 1.7× under the 10 ns boundary. A hook
  that only needs to observe or veto, and can be expressed in the injected Java,
  should not cross the boundary at all. This constrains hook design from M2 onward
  and is an input to ADG-4.
- **The transform, not the boundary, is what costs memory and startup.** Both
  agent candidates retain 156 more classes than the control, from
  `java.lang.classfile`, and every candidate reaches readiness 14–15 ms after
  the control regardless of what it binds. The boundary adds nothing measurable
  on top of the transform.
- **A primitive crossing allocates nothing.** Allocated bytes per call read zero
  for all four candidates over exactly 1.28 M invocations. Allocation counts are
  `unavailable`: the runtime exposes allocated bytes per thread but not counts.
  This is a property of the signature measured, not of the boundary: the trial
  passes one `int` and returns a `boolean`. A hook taking an object reference
  creates a JNI local reference, and nothing here prices that.
- **Everything fails closed.** Invalid input, callback failure, an unavailable
  native runtime, a deliberate crash, and a hung candidate each end the trial
  without producing a result. A native abort ends the process on signal 6; a
  Java-side halt exits with a status; a candidate that never announces readiness
  is killed by the harness.

## Why the exported symbol rather than the hosted VM

Both bind the same callback at the same cost, so the decision rests on what
else each one drags in.

- **Unsafe surface: zero against two.** The spike wrote the export by hand as
  `#[unsafe(no_mangle)] extern "system"`, but Ravel does not have to. The pinned
  `jni` 0.22.4 ships a `#[jni::jni_mangle("class", "method")]` attribute that emits
  the export for you, and a crate keeping `#![forbid(unsafe_code)]` verbatim — no
  `allow`, no `expect`, no relaxation — compiles clean and exports the JNI-mangled
  symbol. Verified by building it and reading the symbol table. Because the
  attribute takes class and method as string literals, ADG-3's generator can emit
  it, so the declaration is generated rather than written.

  The hosted VM cannot reach zero the same way: `Env::register_native_methods` and
  `NativeMethod::from_raw_parts` are both `unsafe fn` in that crate, and an unsafe
  call is not something an attribute can hide. The recorded counts are what a
  production implementation would need in maintained crates, which is what the
  result schema asks for, so this candidate records zero and the hosted VM two.
  Neither number describes the spike's own hand-written source.
- **Launcher integration.** A `-javaagent` is a stock Prism JVM argument. The
  hosted VM is a wrapper command, which means Ravel would have to reconstruct
  the launcher's entire JVM invocation — class path, natives directory, module
  flags, memory and collector settings — and keep reconstructing it correctly
  across game and launcher updates. That is a large compatibility surface bought
  for no measured gain.
- **Runtime ownership.** As an agent, the game runs on the runtime Mojang pinned
  and Prism selected. Hosting the VM makes that Ravel's problem.

The hosted VM is the better mechanism on exactly one axis: crash attribution,
because the process is the Ravel binary rather than `java`. That is worth
recording but not worth the compatibility surface, and native crash attribution
is recoverable through the failure diagnostics M2-W7 already owes.

Keep the hosted candidate as the documented fallback. It is the mechanism to
revisit if Ravel ever needs to do work before a VM exists.

## Rejected

- **JVMTI native agent.** Reaches the same hook through a raw C environment
  table: an exported `Agent_OnLoad`, a `jvmtiEnv` function-table dereference per
  capability, event callback registration, and class retransformation. It buys
  nothing that `java.lang.instrument` does not already provide for this hook
  shape, and adds unsafe surface no audited crate encapsulates. Recorded in
  `adg1-jvmti-native-agent.json`.
- **External Java bytecode library inside the agent.** The pinned runtime ships
  `java.lang.classfile` (JEP 484), so a shaded rewriting library would add a
  Java dependency to every launch for an API the runtime already provides, and
  would have to be version-matched to the runtime by hand on every game update.
  Recorded in `adg1-external-bytecode-library.json`.
- **A helper process instead of an in-process boundary.** This is the only
  mechanism that needs no maintained unsafe Rust at all, which is why it is
  named here rather than ignored. It cannot preserve a 10 ns hook callback, and
  its real evidence belongs to ADG-2, which measures a helper process against
  the in-process alternatives. If ADG-2 finds the process model viable for mod
  execution, the hook boundary still has to cross into Ravel's own process, so
  ADG-1's mechanism stands either way.

## Consequences

- Ravel ships two artifacts into the game process: a small premain agent jar and
  one native library. The agent jar is a Ravel artifact, not another mod loader,
  and it holds only the entry point and the generated hook surface. What that
  native library *contains* is ADG-2's question, not this one: under a linked mod
  set it is also where every installed mod lands, and under per-mod dynamic
  libraries it stays small. ADG-2 is deferred to M3, so this ADR does not claim
  either.
- **Who calls `System.loadLibrary`, and from which class loader, is unresolved.**
  JNI binds a loaded library to the loader that loaded it and resolves native
  methods only against that loader's copy of the declaring class, so a second copy
  of the generated hook class under a different loader fails at the first hook
  invocation with `UnsatisfiedLinkError` — at launch, not at build. The generated
  class lives in the agent jar on the system class path, which makes this
  tractable, but nothing here or in ADG-3 or ADG-5 states the rule. M2-W5 owes it,
  and it is safe Java either way, so it bears on whether the mechanism works rather
  than on the unsafe count.
- No Ravel crate holds maintained unsafe code, and none needs an exception. Every
  maintained crate keeps `#![forbid(unsafe_code)]`, including whichever one carries
  the export, because `jni_mangle` writes the declaration. `SPEC.md` §3.3 records
  this rather than an exception, and the M1-W8 reviewer is what found it.
- The export count scales with the hook surface, not with the crate. Six inventory
  entries with heterogeneous descriptors cannot share one `extern "system"`
  signature, so there is one generated export per hook shape. That is harmless
  while every one is generated and safe; it would not have been under a
  hand-written exception, where the alternative was a single
  `(int hookId, Object[] args)` dispatcher whose per-call array allocation would
  contradict the zero-allocation result above. Recording the dilemma because the
  chosen mechanism is what dissolves it.
- The hook surface is generated, which is ADG-3's subject: the agent needs a
  target class, method, and descriptor per hook, extracted from the pinned jar.
- Hook design has a floor from now on, not a cost model: a primitive crossing
  costs at least about 9.5 ns over an uninstrumented call and allocates nothing,
  measured at a warm monomorphic call site with a trivial body. A real hook — an
  object reference, a megamorphic site, a scattered working set — costs more by an
  amount M2 has to measure. The floor is already enough to say that per-entity or
  per-tick-per-block hooks either stay in the injected Java or batch.

## Limits of this evidence

- The hook target is `net.minecraft.util.StringUtil.isAllowedChatCharacter`,
  chosen because it initializes without the libraries the client jar does not
  bundle. It is a representative small static method, not a real gameplay call
  site. That a real 26.1.2 client launches with the agent attached is ADG-5's
  and M2-W5's evidence, not this ADR's.
- Callback samples are batch means: one boundary transition costs less than a
  `nanoTime` pair, so trials time batches of 64 and the harness divides after
  taking percentiles. Percentiles are therefore labelled `estimated`.
- **The measured shape has no panic guard, and the veto path is unmeasured.** The
  trial's hot path deliberately does not upgrade its environment, because a hook
  that only reads its arguments should not pay for an attach guard and a
  `catch_unwind` per call. `jni`'s own documentation says a native method must use
  `catch_unwind` — via `EnvUnowned::with_env` — to be safe, and a Rust panic that
  unwinds out of `extern "system"` aborts the process rather than propagating. So
  10.02 ns is the cost of the unguarded shape. A hook that *vetoes by throwing*, the
  capability named on the first entry in `hooks.toml`, has to upgrade, and that cost
  was never measured. Neither is the cost of guarding the plumbing between the
  extern frame and the mod call, which is where a panic in generated binding code
  would otherwise abort. Both belong to M2-W5, and neither needs unsafe.
- Trials ran on the development runtime 25.0.4 rather than the pinned Mojang
  25.0.1, recorded as a non-blocking deviation in each of the four measured
  results. The two rejected candidates record no deviations, because they were
  rejected before a trial ran. Setting
  `RAVEL_SPIKE_JAVA` to the pinned runtime removes the difference.
- Startup is a single observation per candidate, and repeated runs move it by a
  few milliseconds. No conclusion here depends on the ordering of startup times,
  only on the transform accounting for the whole delta.
- Sub-nanosecond medians move between runs, because at that scale the figure is
  really the JIT's success at eliminating the call, and because a batch mean
  divided by 64 has a floor of about 0.016 ns that the control and the Java
  callback both sit a few steps above. The 10 ns boundary cost is stable to two
  decimal places across runs; nothing here rests on the difference between 0.47 ns
  and 0.63 ns beyond both being far under the boundary.
- **The recorded percentiles are not per-call percentiles, and no per-call tail
  can be recovered from this evidence.** Trials time batches of 64 and the harness
  takes percentiles of the batch totals before dividing, which is why every figure
  in the table is an exact multiple of 1/64 ns. A percentile of 64-call means is a
  statement about averages, not about single crossings: averaging suppresses
  dispersion, so the real per-call spread is wider than these numbers by an amount
  this data cannot quantify, and one very slow crossing inside a batch is
  arithmetically indistinguishable from a uniformly slower batch. The p95 and p99
  of the selected candidate are the same value, 12.53 ns, which is what a
  collapsed visible range looks like. The medians are sound; the tails should not
  be read as a bound on worst-case crossing cost, and anything that needs one —
  a frame-time budget above all — has to measure unbatched over a window long
  enough to contain collection, safepoint, and scheduler events. The timed region
  here totals about 13 ms.
- Every candidate is comparable against the control rather than against each other
  only. The control installs no callback, so its callback distribution records the
  unhooked cost of the same call site, which is what makes 0.63 ns and 10.02 ns
  differences rather than absolute costs. The control result carries this as a
  deviation, and it is stated here because a reader comparing two hooked rows
  directly is comparing two numbers that each already contain the call site.
