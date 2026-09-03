# ADG-1 — JVM entry and hook mechanism

Status: accepted at M1-W2; amended by [ADG-6](adg-6-configuration-ui.md) before
M3-W3 to say what else Ravel puts into the game process, amended again at M3-W3
on a measurement that found one class loader where this record described a
delegating pair, and amended by [ADG-7](adg-7-world-persistence.md) before M3-W4
to admit one receiver-aware Java-adapter mode, amended again while scoping
the second version directory to record that a second Minecraft version reaches
none of this, and amended once more when the game names those adapters resolve
stopped being literals in a shared source and started being generated. The
amendments are recorded below
and each consequence they correct is marked where it appears. Nothing about the
entry mechanism changed. It needs no `SPEC.md` §3.3 amendment and no unsafe
exception: the premise that an exported symbol cannot be encapsulated turned out
to be false, and the correction is recorded under "Unsafe surface" below.

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
  either. **Amended before M3-W3:** the count is wrong and the description of the
  jar is right. ADG-6 adds a third artifact — a version-specific Java screen —
  and it goes beside the agent jar rather than inside it. See "Amendment: what
  the agent jar carries".
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
- The export count scales with the hook surface, not with the crate. Inventory
  entries with heterogeneous descriptors cannot share one `extern "system"`
  signature, so there is one generated export per hook shape. The figure is not
  written here: `hooks.toml` is what the surface is counted from, and
  `docs/architecture/m3-hook-targets.md` states the count against it under a gate
  that recomputes it. That is harmless
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

## Amendment: what the agent jar carries

Amended before M3-W3, on [ADG-6](adg-6-configuration-ui.md)'s account. ADG-6
draws Ravel's settings screen in Java, which puts a third piece of Java into the
game process and asks whether it belongs in this jar. **It does not.** The jar
holds the entry point and the generated hook surface, exactly as the consequence
above says; the screen is a separate, version-specific artifact. What is
corrected is the count, not the description.

### What the sentence said

"Ravel ships two artifacts into the game process: a small premain agent jar and
one native library. The agent jar is a Ravel artifact, not another mod loader,
and it holds only the entry point and the generated hook surface."

Read as a description of M1's jar, that is accurate and still is. Read as a
prediction of everything Ravel would ever put into the process, it was wrong,
because M1 had no screens. The sentence needed testing rather than repealing,
and the answer is that its restrictive half holds and its arithmetic does not.

### Why the jar stays minimal

Two independent reasons, and neither is about what is convenient for M3-W3.

**The jar builds without the game, and that is a property rather than an
accident.** `jvm/build/build-agent.sh` says so where it compiles: "No source in
this jar declares a game type: the transformer manipulates byte arrays and
descriptor strings, and the persistence, content and outward adapters resolve the
game classes they need by name at runtime. So nothing here needs the client jar on the class path, and
the absent `-cp` is what enforces that rather than records it — which is also what
lets this run on a machine without the restricted fixture." `jvm/README.md` says
the same for both build steps, and
[`m3-hook-targets.md`](../architecture/m3-hook-targets.md) records that the
generator declares every reference parameter as `java.lang.Object` precisely so
that generated `Hook.java` declares no game type and still compiles without the
client jar. Neither artifact is free of game *names*: `Hook.java`'s
target table carries the internal name of every class it transforms, and the
three adapters between them resolve twenty-one classes by name — all of which the invariant
below permits, because a string is not a compile-time reference and needs nothing
on the class path. Those twenty-one names are generated rather than written, and so is
every member the adapters reach on them; the amendment below records why and what
carries them.
A screen extends `net.minecraft.client.gui.screens.Screen` and cannot be compiled
without it. Putting the screen in this jar would make the licensed client jar a
build input for the agent, for every contributor on every build, in a public
repository whose
[public/private boundary](../architecture/public-private-boundary.md) says
official Minecraft files are never tracked and authorized material lives only in
an ignored directory. That is a licensing and contribution property, not a
packaging preference.

**The jar is on the wrong side of the delegation.** M2-W5 resolved the class
loader question this record left open, and `jvm/README.md` records the answer:
the agent jar is appended to the system class path before `premain` runs, so
`Hook` is loaded by the system class loader, while "Stock Minecraft 26.1 runs on
the application class loader, which delegates to the system loader, so it finds
that same copy". Delegation runs from child to parent. A game class therefore
resolves `Hook`, and a class in the agent jar does *not* resolve
`net.minecraft.*`. A `Screen` subclass placed in this jar would fail to resolve
its own superclass at launch. This is a mechanism failure rather than a policy
one, and it would arrive as a `NoClassDefFoundError` in a running client rather
than as a build error.

> **Corrected at M3-W3. The mechanism in this paragraph is wrong; what it
> concludes about `Hook` is right.** There is no delegation and no parent/child
> pair. See "Amendment: there is one loader, not two" below.

### Why this is a correction rather than a reversal

Nothing this ADR decided is being loosened. The entry mechanism, the exported
symbol, the loader binding, and the zero-unsafe result are untouched, and the
restrictive clause — "only the entry point and the generated hook surface" — is
upheld rather than widened. What changes is a count in the same sentence: three
artifacts enter the game process, not two. `jvm/README.md` carried the same
arithmetic in its opening line, and this paragraph claimed it had been corrected
in the same pass when it had not: that file went on saying two artefacts entered
the process, promising a third "with M3-W3", and deferring where the screen's
sources lived, for as long as `jvm/screen/` existed beside it. It records the
count and the reason now, and the deferral is gone because M3-W3 answered it.

The distinction matters because the tempting reading was the opposite one. It
would have been easy to treat "only the entry point and the generated hook
surface" as narrower than the reasoning behind it and widen it to admit a
generated screen, by analogy with ADG-4's handle amendment. The two reasons above
are why that analogy fails: ADG-4's sentence was stricter than its own reasoning,
and this one is not.

## Amendment: there is one loader, not two

Amended at M3-W3, on a measurement rather than on an argument.
[`evidence/m3/class-loader-identity.json`](../../evidence/m3/class-loader-identity.json)
recorded, from inside one launch of the pinned client through the stock
launcher, the identity of the loader that loaded a rewritten `net.minecraft`
class and the identity of `ClassLoader.getSystemClassLoader()`. **They are the
same object** — `jdk.internal.loader.ClassLoaders$AppClassLoader`, whose only
parents are the platform and bootstrap loaders. The launch rewrote 10 classes and
24 methods, so it is a launch that actually exercised the path.

**What this corrects and what it leaves standing.** The paragraph marked above
reasons from `jvm/README.md`'s "delegates to the system loader", and delegation
needs two loaders. There is one object playing both parts, so "the jar is on the
wrong side of the delegation" describes a geometry the pinned launch does not
have. Its conclusion still holds and now holds more simply: a game class resolves
`Hook`, because the loader that loads the game class is the loader `Hook` is on.
`jvm/README.md` has been corrected in the same terms.

**The decision is untouched.** The entry mechanism, the exported symbol, the
loader binding, the zero-unsafe result, and the restrictive clause are all
exactly as accepted. This amends a mechanism, in the way the amendment above
amends a count.

### The correction admits a third mechanism this record excluded

The two options below were written under the false premise. If the system loader
were the wrong side of a delegation, appending to it would be pointless, and that
is why a third mechanism was never listed: **the agent appends a jar to the
system class loader's search path itself, through the `Instrumentation` it
already holds.**

M3-W3 selected it, and it is better than either option below on their own terms.
It costs [ADG-5](adg-5-prism-integration.md) nothing that the first option would
have cost: the *screen jar* reaches the loader through an argument Ravel already
passes, and nothing puts that jar anywhere. What the clause here originally said
was broader — that no launcher puts anything anywhere — and that was false of the
launches M3 went on to run: each of them staged the native library into the
instance's natives directory from the instance's `WrapperCommand`. That is ADG-5's
own outstanding correction rather than this mechanism's, it is recorded as an M3
deviation, and ADG-5's amendment is where the fix is chosen. The claim that holds
here is about this artifact. And the screen stays ordinary readable Java compiled ahead of
time, which is what the second option gives up by emitting a GUI as
`java.lang.classfile` builder calls.

Two properties make it work, and both were measured rather than assumed.
Appending is not defining: `appendToSystemClassLoaderSearch` at `premain` only
widens where the loader may look, so no game class is forced to load before the
client bootstraps, and defining the screen stays lazy behind a hook. And an entry
appended at `premain` is still found by a `Class.forName` long afterwards, on the
same `AppClassLoader` that loads the game.

### The two screen-artifact candidates M3-W3 superseded

The screen names game types, so it has to be loaded by a loader that resolves
them, and it has to reach Rust, which means reaching `Hook` on the system loader.
The delegation above gives the second for free once the first holds. Before the
measurement above admitted the third mechanism, this record did not pick between:

- Ship it as a jar the launcher puts on the game's class path. That works, and it
  costs [ADG-5](adg-5-prism-integration.md)'s property that Ravel adds arguments
  to a stock instance and nothing else.
- Have the agent define the screen's classes into the game's own loader, using
  the `java.lang.classfile` machinery the transformer already runs. This keeps
  ADG-5 intact and is the one to try first.

M3-W3 owed the choice, in the same way M2-W5 owed `System.loadLibrary`, and
answered it with the appended sibling jar above. It is safe Java, so the choice
bears on whether the mechanism works rather than on the unsafe count.

> **Answered at M3-W3, and with neither of these two.** The measurement above
> admitted a third mechanism that this list excluded on a false premise, and that
> is the one selected. See "The correction admits a third mechanism this record
> excluded".

### What is still forbidden

- The agent jar must keep declaring no game type and must keep compiling with no
  client jar on the class path and no licensed fixture. That was an observation
  about M1's jar; from here it is an invariant, and it is enforced by construction
  rather than by a comment: `jvm/build/build-agent.sh` runs `javac` with no `-cp`
  at all, so a source declaring a game type fails to compile. What is *not* an
  invariant is that the jar mentions no game class anywhere — ADG-7's persistence
  adapter names the classes it needs as strings and resolves them at runtime, which
  this rule permits. The amendment below says what that costs and what pins it.
- The screen artifact declares no native methods of its own. Every crossing still
  goes through the generated `Hook`, which is what makes the loading loader and
  the declaring loader the same one by construction. A second class binding its
  own library reintroduces exactly the `UnsatisfiedLinkError` the consequence
  above was written to prevent.
- Nothing here admits a call from Rust into the game. ADG-6 refused that, and a
  third artifact does not provide it: the screen is Java calling Java, and its
  traffic to Rust runs in the direction this ADR already established.

## Amendment: one receiver-aware Java adapter

Amended before M3-W4 on [ADG-7](adg-7-world-persistence.md)'s account. The
version-specific sibling Java artifact may carry the persistence adapter as well
as screens. This is an artifact-scope widening only: the agent jar still declares
no game type and still compiles with no client jar on the class path, and generated
`dev.ravel.jvm.Hook` remains the sole JNI surface.

What the widening does add, and what an earlier form of this sentence concealed by
saying the agent jar "names no game type", is a set of game names the always
present jar resolves reflectively.
`jvm/bootstrap/dev/ravel/jvm/Persistence.java` and
`jvm/bootstrap/dev/ravel/jvm/Content.java` name the game classes they need as
string constants, resolve each through `Class.forName` against the system class
loader, and reach their members by name — by arity in the first and by parameter
type in the second, where three `Registry.register` overloads share one arity. None of those names appears in
`hooks.toml`, `stubs.toml`, `stubs.lock` or `mappings.lock`, so ADG-3's drift gate
does not reach them: the *optional* screen jar compiles against stubs generated
from a pinned lock and the *required* agent jar has no such binding. Neither of
this decision's two reasons is damaged — the licensing and contribution argument
above and the loader argument as corrected both rest on the jar declaring no game
type, which still holds — but the coupling is real and is recorded rather than
implied. `verify_persistence_reflection` pins the class set for both adapters so a new unpinned name
cannot be added silently, and states in its own doc comment that it cannot detect a
game-side rename because the names are in no lock.
`evidence/m3/manifest.json` records the whole surface as a named non-blocking
deviation, and `ROADMAP.md` records bringing it under `stubs.toml` and
`mappings.lock` as owed work whose price is the re-pin ceremony against the pinned
jar.

The transformer may use one receiver-aware Java-adapter injection mode for
ADG-7's save seam. That mode is distinct from ordinary native observation hooks
and does not widen them to receive local slot zero. It extends the existing
`HookTarget` and prepend transformation rather than creating another registry,
transformer, or exporter.

Persistence capability is required and fails closed unless its installation
handshake proves the world-open prologue is active; finding the sibling jar or
class is insufficient. Screens remain optional. Java still initiates the two
batched JNI calls, and no native method is declared outside generated `Hook`.
Nothing here selects a general mechanism for Rust to call into Minecraft or
otherwise reverses the boundary direction ADG-1 accepted.

## Amendment: a prologue may refuse the method it precedes

Amended before Phase C on [ADG-7](adg-7-world-persistence.md)'s account. A prepend
injected for an opted-in target may return from the host method before the original
body runs. Ordinary observation hooks are unchanged: they return nothing, they
cannot branch, and they fall through unconditionally as they always have.

ADG-7's open refusal has no other mechanism. It requires a refused world load to
stop before `doWorldLoad`'s body reaches its first persistent mutation, while
forbidding the refusal to leak a raw exception, retain the world lock, or crash the
callback. Throwing — the stop mechanism this decision already contemplated —
satisfies the first and violates the rest: it unwinds into game code Ravel does not
author, and nothing there catches it. Making a throw safe would mean wrapping the
original body in a handler, a larger rewrite of the same method for a weaker
guarantee.

The power is bounded by what carries it. The opt-in is per target and arrives as
generated data on `HookTarget`, so the methods a prologue may refuse are exactly
those the reviewed inventory names and the lock pins: the set is visible in a diff
and cannot be widened by editing Java. The verdict is a boolean, so a hook says only
whether to return and never what to return, and no value crosses back into game
control flow. A target that returns anything but `void` cannot opt in at all. The
transformer, the target registry, the JNI exporter, and the native declaration class
remain one each.

It grants nothing else. A prologue may not alter arguments, substitute a return
value, resume a method it declined, or observe the receiver it was never given. Rust
owns the decision; Java owns cleanup and presentation. Nothing here reverses the
direction ADG-1 accepted: Java calls Rust and asks, and Rust answers.

## Amendment: a second Minecraft version changes nothing here

Amended while scoping the second version directory, on
[Version selection is a build-time choice](version-selection-is-a-build-time-choice.md)'s
account. This amendment adds no mechanism and removes none. It exists because a
reader arriving at a repository that tracks two Minecraft versions would
reasonably expect this record to have moved, and it has not.

The entry mechanism stays one `-javaagent`, one `System.loadLibrary`, one
`dev.ravel.jvm.Hook`, and one exported JNI symbol per hook, mangled against that
class with no version component anywhere in the name. That survives a second
version *because* selection is a build-time choice: one build generates against
one version directory and emits that version's exports, so two inventories that
both review the same capability never render into one library and the mangled
names cannot collide. The hand-written exports on `dev.ravel.jvm.Hook` are
untouched for the same reason, and so is the single `links` entry
`ravel-bootstrap` reads its generated exports through.

What would reach this record is the option that decision rejected: a single
library carrying several versions, which needs either a version inside the
mangled symbol name or a second `Hook` class in the same namespace. Either one is
a change to the mechanism, and neither is made here. If that option is ever
reopened — the condition is named in that record — this record is amended then,
with a measurement, rather than assumed to permit it now.

## Amendment: the adapters' game names are generated, not written

Amended when a second Minecraft version made the shared adapters' string
literals wrong twice over. This amendment changes no mechanism and adds no
entry point. It records where one set of names comes from, which is a question
the record had never had to ask because there had only ever been one version to
be right about.

`jvm/bootstrap` is one source tree and every tracked version compiles from it.
The adapters used to spell their game classes as string literals — `MINECRAFT =
"net.minecraft.client.Minecraft"` and seventeen more like it — and that is wrong
in two independent ways the moment a second version exists. A class one version
calls `net.minecraft.resources.Identifier` another calls `ResourceLocation`, so
one spelling in one shared file asserts one version's names over both. And a
version whose class files are obfuscated links none of those names at all, while
a check comparing Mojang names against a declaration of Mojang names passes and
says nothing: the runtime failure it was supposed to catch is invisible to it by
construction.

So the names are generated, by the mechanism that already generates the other
name the agent jar carries. Each version's `reflection.toml` declares, for each
class an adapter reaches, a **role** — the word the shared source spells, the
same under every version because it names what the class is to Ravel — beside the
**Mojang name** a reviewer reads and the **runtime name** the JVM links. The
binding generator renders the roles into `dev.ravel.jvm.GameNames`, a
package-private class in the same generated tree as `Hook.java`, with the runtime
names as values and the Mojang names as comments. The adapters resolve
`GameNames.<role>`. That is the same two-audience split `HookTarget` has always
made, applied to the one name surface that had been left out of it.

Nothing about the entry mechanism moves. `GameNames` declares no native method
and no game type, so ADG-1's one exported symbol surface is still
`dev.ravel.jvm.Hook` and the jar still compiles with no `-cp`. What the jar
carries is unchanged in kind — string names, resolved at runtime — and changed
only in where the strings come from.

Two checks hold it, in two places, because they need two different things.
`verify_persistence_reflection` in `tools/ravel-xtask/src/lib.rs` refuses a
game-class literal anywhere under `jvm/bootstrap` outright, and holds the roles
the sources spell and the roles every tracked version declares to each other in
both directions. It does not re-derive a runtime name, and that is deliberate:
what a Mojang name links to is decided by the version's mapping source, which
only a build that selected that version holds, so the same rule written here
would be the weaker of two copies. The generator owns it instead —
`mapping::verify_reflection` in `ravel-minecraft-adapter` refuses a declared
runtime name the mapping does not resolve its Mojang name to, on every build that
can read that mapping, which under an unobfuscated source is every build at all
because the identity needs no file.

The transformer stopped naming a game class at the same time and for the same
reason. It identified five seams by the class and member it was about to rewrite,
which is a Mojang name in a shared source twice over, and it already held the one
name that survives a game update: `HookTarget.hookMethod`, rendered from the hook
id ADG-3 calls the only durable name. Each of those five now names its hook
instead. Two of them dropped a descriptor comparison with it, because a hook id
renders one hook name and the overload it was there to tell apart carries a
different one. This is the same idiom `Persistence.java` already used for its
save-seam marker, generalised rather than invented.

The figure above moves from seventeen to eighteen for a reason worth stating: the
declared set is now the adapters' whole reflective surface rather than the part of
it `hooks.toml` did not already pin. A class covered by a hook target used to be
left out of the declaration, which was correct when the declaration existed only
to catch an undeclared literal. It is wrong now, because the declaration is what
the constants are generated from, and a class with no role has nothing to be
reached by.

It moves again, from eighteen to twenty-one, because the surface declares the
members as well, and then down to thirteen when content registration left the
agent jar entirely. The persistence adapter used to spell every member name it asked
for as a literal, which was 26.1.2's spelling asserted over every version and the
obfuscated name of none: 1.21.1 links `getWorldPath` as `a` and `LevelResource.ROOT`
as `l`. Declaring the members brought their owners in with them — the level a
storage hangs off, the world being opened, the loaded world and the registry
layers under it — because a member is declared against the class that has it.

A member is declared by its descriptor rather than by an argument count, and the
generated constant carries both the linked name and the linked descriptor. Under
an obfuscated version a name and a width picks no single member: 1.21.1 links both
`DimensionDataStorage.computeIfAbsent` and its `set` as a two-argument `a`, so a
lookup by width would read a world with whichever the class file happened to
declare first.

The declared members also cost the set one class. `SoundInstance` was listed only
so the outward adapter could name it as a parameter type, and a descriptor-declared
member carries its parameter types itself, so nothing resolves that name any more
and the mapping translates it either way.

The eight the fall to thirteen removed are content registration's — the registry,
the built-in holders, the root keys, both value types and both properties
builders. They are seeded in `stubs.toml` now and compiled against rather than
reflected, because 26.1.2's builders demand a registry key stamped before a value
is constructed and 1.21.1 declares no setter for one. That is a different call
rather than a different name, which is the line this record already draws between
a role binding and a per-version source.

Four came back, to seventeen, and the reason is the one case where reflection is
the better tool rather than the tolerated one. Registering a block puts it in a
registry and nothing else; what makes it usable is a walk over its states into
the id mapper the network protocol encodes a block update through, which vanilla
runs once over its own blocks and never again. A block registered afterwards
places and then disconnects the session, which is what the seventh launch did.
The walk cannot be compiled beside the registration: the state list is a Guava
type, neither tracked jar carries Guava, and a stub declaring that return type
would not compile. Reflection names no type, so the block, its state definition,
the id mapper and the class that carries a state's cache step are declared here
and reached by name.

Three more came with it, to twenty, when a world being created turned out to
need the mods' data staged before the create-world screen loads its own. The
screen holds the staging directory and the reload behind two private methods, and
the configuration those need behind three public ones, so the screen, its state
and its settings are all declared. This is the same case as the block walk rather
than a new one: what is reached is a member the surface pins, not a name a source
spells, and nothing here could be compiled against instead — a private method is
not on any stub's surface at all.

One more, to twenty-one, when a real client showed the staged pack written but
never selected: the screen's apply reads the repository's selection and makes
none, so the pack repository is declared for its reload, its selected ids and
the setter that puts the staged pack among them, and the screen's own
`applyNewPackConfig` is declared beside those for the load that apply skips when
the selection it reads is the one the screen already holds.

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
