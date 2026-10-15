# ADG-2 — Rust mod execution and ABI

Status: measured at M1-W3; **decided at M3, in two halves.** Mods are
independently shipped and interchangeable, decided first and on what players
expect rather than on a number. The mechanism is decided now: a mod is
distributed as Rust source and compiled on the player's machine by a toolchain
Ravel ships, and Ravel accepts no compiled mod artifact — not an rlib, not a
dynamic library. **The deciding reason is that Ravel then owes no ABI.**
[`pack-compile.json`](../../evidence/m3/pack-compile.json) prices the mechanism
and shows it affordable; it does not select it. `SPEC.md` §3.4 states the same
decision in the product contract's own voice.

## What is decided and what is not

How a mod executes was not decided at M1. That was deliberate, and a correction
rather than an omission: this gate selected native dynamic libraries at M1-W3,
revised the selection to linked mods at M1-W6 on the same evidence, and the second
revision exposed that the deciding considerations — distribution, hot reload, who
links a mod set, what a package contains — are all properties of a loader that did
not exist yet. A decision that flips twice before anything is built is a decision
taken too early.

M3 decided, against the loader prototype. What M1 contributes is the cost of each
option and the trade each one makes, which is what a later decision needs and does
not have to re-measure. The two halves were taken separately and for different
reasons, and both are recorded below: the distribution property on what a modded
Minecraft player expects of a mod set, the mechanism on what each one obliges
Ravel to keep stable forever.

## Decision at M3

**A mod is independently shipped and interchangeable. A pack is not one binary.**

A player adds a mod by adding one thing to an instance, removes it by removing
that thing, and updates it by replacing it. A pack is a list of pinned mods plus
configuration and data, not an opaque artifact.

**The thing added is Rust source, and the player's machine compiles it with a
toolchain Ravel ships.** Ravel accepts no compiled mod artifact for
distribution — not an rlib, not a dynamic library. That half is under "The
mechanism" below; this half is why there is anything to distribute at all.

### Why, given the measurements said otherwise

Neither number that used to argue for linking survives contact with the question
it was answering.

- **Call cost does not decide this.** The gap is 0.625 ns, and this record's own
  arithmetic says it takes 160,000 mod calls in one tick to reach 0.1 ms. Nothing
  approaches that.
- **Iteration cost does not decide it either.** M2-W8 measured a relink at 598 ms
  and a pack compile at 880 ms, which was the objection linking was supposed to
  face and does not.

What decides it is a property no measurement here was addressed to. Modded
Minecraft is built on interchangeable mods, and every launcher and distribution
site in that ecosystem assumes it. A pack that ships as one binary removes the
behaviour experienced players most expect — adding a mod the pack author did not
choose, removing one they did — and asks them to accept an opaque artifact in
exchange for properties they cannot see. That is a worse trade than the
nanoseconds suggested.

### What this costs, and what buys most of it back

Two of the three advantages linking held are recoverable, and the M1 trial did not
attempt either because it was measuring a boundary rather than designing one.

- **A panicking mod.** A panic crossing a C boundary is undefined behaviour, so the
  trial's dynamic-library host died. It need not: the SDK generates every mod entry
  point and no author writes the boundary by hand, so the generated wrapper catches
  the panic on the mod's side and returns a failure the loader can attribute. What
  the trial measured is an unwrapped boundary, not the only available one.
- **Contract drift.** A symbol checked by name alone cannot notice a changed
  signature. An explicit ABI version and a schema digest checked at load turn that
  into a refusal with a message. Weaker than a compile error, and far from nothing.
- **Maintained unsafe.** Not recoverable if the mechanism is a dynamic library:
  `libloading` needs two calls, and that needs a `SPEC.md` §3.3 amendment and the
  first row in [`safety-boundary.md`](../architecture/safety-boundary.md). It does
  not arise at all if the mechanism is compiled source.

### The mechanism

**A mod is distributed as Rust source. Ravel ships a toolchain and compiles the
pack on the player's machine. Ravel accepts no compiled mod artifact.**

That is the static-composition binding M1 measured, with the link performed
locally instead of by whoever built the pack. Interchangeability survives it
because a pack is still a list of pinned mods: adding one adds a source crate and
recompiles, removing one removes it and recompiles, and neither is an opaque
artifact a player cannot inspect.

**The deciding reason is not performance.** It is that source compilation is the
only mechanism that commits Ravel to nothing permanent. If Ravel compiles every
mod, it never owes ABI stability: `HostMod`, `Registrar`, `JavaHandle`, the hook
value types, all of it can change and the world recompiles. Rust has no stable
ABI — this record's own limits say rlib metadata is compiler-version locked — and
under every other mechanism that is the central problem to be managed for the
life of the project. Under this one it stops being a problem.

The concrete case is already in the tree. `ravel-minecraft` depends on `jni`,
because a hook argument that refers to a game object is a borrowed JNI local
reference and the lifetime that makes it safe to hold is `jni`'s.
[`dependency-rules.md`](../architecture/dependency-rules.md) records that no mod
can reach that crate today and that a change making it mod-reachable must seal
`JavaHandle::new` rather than move the dependency. M3 is walking towards that
change: hook observation is the capability mods are being built to want. Under
source compilation, sealing the constructor later is a recompile of every
installed mod. Under any binary mechanism it is an ABI break that cannot be
taken, and a version-pinned `jni` sits in every published mod's dependency graph
permanently. That is the compatibility surface source distribution avoids, and an
rlib hatch would reintroduce it for the minority of authors who want one in an
ecosystem that is mostly open already.

**The asymmetry decides the close calls.** Source-only is reversible and binary
distribution is not. Binary support can be added later if authors demand it,
against real demand rather than a guess. A compatibility surface cannot be
withdrawn once mods depend on it, so choosing it now would be choosing it
permanently on no evidence that anyone wants it.

Three further reasons, in the order they carry weight.

- **Only source lets the optimizer see across mod boundaries.** A dynamic library
  is a C ABI: every mod call is an opaque call through a resolved pointer and
  nothing inlines through it. This record already measures the shape of that —
  mod-to-mod is 2.34 ns linked against 2.97 ns through the dynamic library,
  because the second hop is an ordinary call rather than a second indirection —
  and inlining is the part the benchmark's one-arithmetic-step mod body cannot
  show at all. `SPEC.md` §6.1 and §6.2 are the written contract this serves, and
  §6.5 names "hook and boundary overhead" as one of the things re-examined if that
  contract fails. A mechanism that forecloses cross-mod optimization forecloses it
  before anyone has measured whether the pack needs it.
- **Source and rlib both keep the safety property; the dynamic library does not.**
  `libloading` needs two `unsafe fn` calls, and those need an accepted `SPEC.md`
  §3.3 amendment and the first row in
  [`safety-boundary.md`](../architecture/safety-boundary.md)'s empty table, whose
  bar is showing that no audited dependency can hold the operation. **This
  decision leaves that table empty.** Every crate in the mod boundary keeps
  `#![forbid(unsafe_code)]` with no exception and no lint relaxation.
- **Source is less work for a mod author, not more.** No per-platform build
  matrix, no release CI, no rebuild when Ravel bumps its toolchain. Under rlibs
  the last of those is not a courtesy: rlib metadata is compiler-version locked,
  so every toolchain bump breaks every published mod at once, and the coordination
  cost of that lands on Ravel rather than on the authors who caused none of it.

Interchangeability is the property both mechanisms deliver. They fail
differently, and the table is what the choice was made against.

| | Per-platform binary | Source compiled by a bundled toolchain |
| --- | --- | --- |
| Artifacts per mod | one per platform | one, portable |
| Rust's unstable ABI | must be answered with a C-shaped boundary the SDK owns and maintains | does not arise; one toolchain compiles the set |
| Mod does not match the API | refused at load, if the version check is built | fails to compile, with a real error |
| Panicking mod | needs the generated wrapper | caught natively |
| Maintained unsafe | two calls, needs the exception | none |
| Time from adding a mod to playing | immediate | a build, cacheable so it is paid once per change |
| `SPEC.md` §3.4 | consistent with it | needs it amended |

The second column reproduces what makes a Java archive pleasant — one portable
artifact, no ABI fragility, failure that arrives early and legibly — which is the
experience this decision is trying not to lose. It costs a shipped toolchain and a
build.

**M1 named compile time as the deciding measurement, and it was not.** The
measurement was taken — `pack-compile.json`, under "What compiling a pack costs"
below — and its own `whatThisDoesNotSay` says the honest thing about what it
settles: "Nothing here selects an ADG-2 mechanism." What it did was price the
mechanism the ABI argument selects, and clear it. 113 ms of marginal cold build
per mod, 32.0 s for a 256-mod pack, and — the figure that actually decides whether
a player can live with this — a 1.6 s warm rebuild, because the cold build is paid
on install and the warm one is paid every time anything changes.

Had those numbers come out badly the ABI argument would not have carried the day
on its own, so the measurement is a veto that was not exercised rather than a
tiebreak that was. Recording it as the deciding evidence would misstate why the
decision went the way it did, and `SPEC.md` §3.4 states the reason in the
contract's own voice: Ravel compiles every mod, therefore Ravel owes no ABI.

### Rejected

- **An rlib shipped by the author.** It keeps closed-source mods possible and it
  needs no unsafe, so it is the only mechanism that loses to source on nothing but
  the compatibility argument. That argument is enough. An rlib is linked into the
  loader's build, so the crate boundary it links against is a published API Ravel
  can never change without breaking every mod compiled before the change — the
  permanent surface described above, bought for a minority of authors. It also
  reintroduces the per-platform build matrix and release CI that source
  distribution removes, and rlib metadata is compiler-version locked, so a
  toolchain bump invalidates every published artifact at once.
- **A native dynamic library.** The only candidate that breaks the safety
  boundary: `libloading` needs two `unsafe fn` calls, and an unsafe call is not
  something an attribute can generate away. That takes a `SPEC.md` §3.3 amendment
  and the first row in `safety-boundary.md`, whose bar is showing that no audited
  dependency can hold the operation rather than that none was found — the same
  rule on which ADG-1 rejected JVMTI at four calls and the hosted VM at two. It
  also dies with any mod that panics, because a panic crossing the C ABI is
  undefined behaviour rather than a catchable event, and its symbol-by-name check
  cannot notice a changed signature. It is rejected, not deferred.
- **Whole-pack link-time optimization as a default.** Fat LTO with one codegen
  unit costs 43× on the warm path — 77.4 s against 1.79 s at 256 mods — which
  turns every mod update into the cold path and then some. It halves the binary,
  and that is worth having as something a player runs deliberately once a pack is
  settled. As a default it would make the mechanism unusable, so it is an opt-in
  action and not a build profile.
- **WebAssembly and a helper process** are rejected on M1's evidence and nothing
  here revisits them: 4.26× the call cost buys a restriction `SPEC.md` §10 does
  not want, and 3.2 µs a call ends the argument for synchronous mod code.

### What this closes and what it does not

This closes ADG-2. Both halves are decided and no part of the gate is deferred.

It does not close the schema reconciliation the gate was blocking, and that
reconciliation is more urgent than "stale assumption" suggests.
[`schemas/package.schema.json`](../../schemas/package.schema.json) requires a
`targets` array of at least one entry, each carrying a `platform`, an
`architecture`, and artifacts with a `path`, a `role`, and a `sha256`. Shipped
source has none of those. **The schema does not refuse a source package, though —
it forces one to be described as something it is not.** `platform` and
`architecture` are bare non-empty strings with no enumeration, and `role` admits
`other`, so a source package validates by writing `platform: "any"` and declaring
its source files as artifacts. A schema that refused would at least fail loudly.
This one passes and records a per-target binary that does not exist, which is
worse, and it is enforced by `ravel:verify` so every package built between now and
the fix carries the misdescription.

What this decision contributes is the shape to write instead of leaving the
reconciliation guessing: a package declares portable source and its integrity, and
the per-target structure goes. That is a schema edit and a `ravel-package` change,
and this ADR makes neither.

**Closure, 2026-08-08:** that reconciliation has landed as a breaking
schema-v1-in-place migration. `targets` and compiled-artifact declarations are
refused. `source` now contains the repository and a non-empty list of portable
source files, each with a package-relative path and SHA-256 digest; provenance
remains independently integrity-checked.

`SPEC.md` §3.4 says the execution and distribution models are both open and that
the document states no requirement about local compilation. That sentence is now
wrong in the other direction and needs amending to state the requirement. §3.3 is
untouched, which is one of the reasons this mechanism was chosen.

## Context

`SPEC.md` §10 says installed mods are fully trusted and may do anything their
execution model allows, with sandboxing explicitly not an initial requirement. So
the question ADG-2 answers is not "how do we contain a mod" but "what does it cost
to call one, and what does its contract cost to keep stable".

`SPEC.md` §3.4 also said users install mod packages that arrive already built and
never compile Rust locally. That was an assumption written before anything was
built, and this gate is the first evidence bearing on it. The measurements say the
packaging model is not free: buying per-mod binaries costs the safety property and
the failure containment, and it buys them back from a distribution convenience
nobody has tested yet. §3.4 was amended to record the open question rather than
the assumption, and M3 answers it against that assumption: a player does compile
Rust locally, and no package arrives built. §3.4 now states that in the product
contract's own voice, with the ABI obligation as the deciding reason, so this
record and the spec argue the decision the same way rather than the spec citing
this one.

## Evidence

Two measurements, taken at different milestones for different questions. M1's
four-binding spike priced the mod-execution boundary and decided nothing; M3's
pack compile priced the mechanism this record selects.

### The four bindings (M1-W3)

Measured on `x86_64-linux`, hardware profile `linux-reference-v1`. Reproduce with:

```console
cargo make ravel:spike-adg-2
```

Results live in [`evidence/m1/spikes/adg-2/`](../../evidence/m1/spikes/adg-2/)
and are checked by `ravel:verify-spikes`. One mod body, written once in safe Rust,
is bound four ways; the host, workload, warm-up, batching, failure probes, and
report shape are shared, so the rows differ by the binding.

| Binding | ns/call p50/p95/p99 | Mod-to-mod ns | Retained RSS | Ready | Separate mod artifact | Maintained unsafe ops |
| --- | --- | --- | --- | --- | --- | --- |
| Static composition | 2.50 / 2.52 / 2.98 | 2.34 | 3.3 MiB | 0.38 ms | none | 0 |
| Native dynamic library | 1.88 / 1.89 / 2.50 | 2.97 | 3.6 MiB | 0.53 ms | 397 KiB | 2 |
| WebAssembly sandbox | 10.64 / 19.25 / 21.14 | 12.36 | 11.4 MiB | 10.63 ms | 18 KiB | 0 |
| Helper process | 3167 / 3755 / 4967 | 6559 | 5.5 MiB | 1.13 ms | 449 KiB | 0 |

Static composition has no separate mod artifact by construction. Its result
records 13.8 MiB of installed bytes, which is the shared host binary the four
trials all run from, and that binary carries the WebAssembly runtime for the
sandbox trial. What the same host measures without that runtime is not recorded in
any result, so the runtime's share is an unrecorded observation rather than
evidence, and the sandbox argument below is qualified accordingly.

What the numbers say:

- **A dynamic library call is free at this scale.** 1.88 ns is a call through a
  resolved function pointer, which is what it is. Symbols are resolved once at
  load, as a loader would; resolving per call would have measured the dynamic
  linker's hash table instead.
- **A linked call is 0.625 ns dearer and buys three things.** Zero maintained
  unsafe, a panicking mod the host survives, and a contract mismatch that fails
  the build. Nothing in `SPEC.md` yet states how many mod calls a tick makes, so
  the honest form of the cost is a rate: it takes 160,000 mod calls in a tick for
  the difference to reach 0.1 ms. M2-W8's regression budget is where that count
  starts being measured.
- **The sandbox costs about 5.7× the dynamic library's median and 10× its 95th
  percentile**, or 4.26× and 7.65× measured against the leading candidate. In
  absolute terms 10.64 ns is the same order as the JNI hook boundary ADG-1
  measured, so WebAssembly is not slow in any general sense — it is slow relative
  to the alternative it is competing with, and its tail is much wider.
- **The helper process costs 1267× the leading candidate's median, 1689× the dylib's.** A synchronous per-call boundary
  over a pipe is 3.2 µs. Batching amortizes that, but a hook whose answer decides
  what the game does next cannot be batched, so the number stands as the cost of
  the model for the thing mods mostly do.
- **Nothing allocates per call.** All four bindings measured zero allocations and
  zero allocated bytes per call, counted by a host-side counting allocator, so no
  binding pays for argument marshalling at this shape. The three in-process
  bindings ran 1.28 M invocations each; the helper process ran 16,000, because at
  3.2 µs a call the same count would have taken an hour.
- **Mod-to-mod calls cost one extra hop, not one extra boundary.** Every binding
  routes them through the host, because the host is the only party that knows
  which mods exist. In-process that is a second function-pointer call, so 2.97 ns
  against 1.88 ns; over a pipe it is a second round trip, so 6.6 µs.
- **Only the sandbox and the process contain a failing mod.** Observed, not
  argued: a mod that panics or aborts takes the dynamic-library host down with it
  on signal 6, while the WebAssembly host catches a trap and the process host
  reads an empty pipe and keeps running. A statically linked mod is the middle
  case: the host catches its panic, because unwinding stays inside one Rust
  program, but cannot survive it aborting.
- **Only the sandbox and the wire protocol type their contract.** The dynamic
  loader checks a symbol by name alone, so a changed signature is undetectable at
  load; the WebAssembly module declares the type of every import and export, and
  the runtime rejects a mismatch at instantiation. The process contract is a wire
  protocol that can carry its own version and refuse an unknown opcode without
  loading anything. All four refused an absent entry point before calling it,
  because the host asks first.

### What compiling a pack costs (M3)

Measured on the same `x86_64-linux` profile, `rustc` 1.95.0, release profile,
sixteen build jobs unless a variant says otherwise. Reproduce with:

```console
cargo make ravel:measure-pack-compile
cargo make ravel:measure-pack-compile-variants
```

The result is [`pack-compile.json`](../../evidence/m3/pack-compile.json). The mod
is generated but not trivial: 1,161 lines, eleven derived types, sixteen
monomorphizations, eight feature blocks and twenty-four registry claims, sized
against `test-mods/registries` by derives and generic instantiations rather than
by lines, because that is what Rust charges for. It is still a floor, and a far
higher one than M2-W8's one-struct crates.

| Pack size | Cold | Cold less baseline | Warm after one mod changes | Build cache | Binary |
| --- | --- | --- | --- | --- | --- |
| 0 (host only) | 2.53 s | — | — | 33.9 MB | 0.45 MB |
| 1 | 3.13 s | 0.60 s | 0.96 s | 46.5 MB | 0.66 MB |
| 16 | 4.55 s | 2.02 s | 0.90 s | 88.1 MB | 2.33 MB |
| 64 | 9.82 s | 7.29 s | 1.05 s | 221 MB | 7.68 MB |
| 256 | 32.0 s | 29.5 s | 1.63 s | 754 MB | 29.0 MB |

What the numbers say:

- **The marginal cost of a mod is 113 ms cold**, a secant across the whole
  measured range rather than a fitted slope, and it is amortized throughput on a
  sixteen-thread host rather than what one mod costs to compile. From sixteen
  mods up the per-mod figure is flat — 126 ms, 114 ms, 115 ms — so the curve is
  linear and a pack twice the size costs twice as much and nothing worse. The
  one-mod point at 604 ms is fixed cost that has not yet been amortized, not a
  steeper slope.
- **The warm path is what makes the mechanism livable, and it is nearly flat.**
  Changing one mod and rebuilding costs 0.96 s at one mod and 1.63 s at 256 — a
  1.7× rise across a 256-fold change in pack size, because cargo rebuilds the
  changed crate and relinks and nothing else. Adding a mod is between the warm and
  cold figures and was not measured.
- **A four-core player pays 73.8 s cold at 256 mods against the control's
  30.5 s**, and pays it
  in a smaller memory footprint, not a larger one: peak resident memory over the
  cargo process tree is 754 MB at four jobs against 2.85 GB at sixteen. Build
  memory is set by how many `rustc` processes run at once, so the machine least
  able to afford the wall-clock time is the one least at risk of failing the build
  outright.
- **A dependency graph costs little cold and a great deal warm.** A sixteen-deep
  spine at 256 mods builds cold in 33.2 s against the control's 30.5 s, +8.9%,
  because a deep graph converts parallel work into serial work. Changing a leaf
  stays warm at 1.76 s. Changing the spine's root costs 29.8 s against 1.79 s,
  which is the cold path in all but name. That is the honest worst case of this
  mechanism: updating a popular library mod recompiles everything that depends on
  it.
- **Whole-pack link-time optimization is not affordable as a default.** Fat LTO
  with one codegen unit halves the binary, 14.2 MB against 29.0 MB, and costs
  104 s cold and 77.4 s warm against 30.5 s and 1.79 s. 43× on the warm path is
  why it is an opt-in action rather than a profile.
- **The shipped toolchain is 196 MB** — the `rustc` sysroot, its standard library
  for the target, and cargo, excluding the linker and C runtime, which come from
  the system. Apparent size summed per path, so a shipped toolchain would
  compress.

## Why linking

Against the dynamic library, the dynamic library is 0.625 ns faster per call and
ships a 397 KiB artifact per mod per target. Everything else in the table runs the
other way. Linking resolves nothing at load, so it needs no `libloading` and no
maintained unsafe; it catches a panicking mod on the host-to-mod path, which the
dynamic-library host does not; it makes mod-to-mod calls cheaper rather than
dearer, 2.34 ns against 2.97 ns, because the second hop is an ordinary call and not
a second pointer indirection; and it turns contract drift into a build error, which
is the strongest form of the
version check the C ABI needs a hand-written symbol to approximate.

The dynamic library's advantage is entirely in distribution: a mod can be built by
its author, shipped alone, and dropped into an existing install. That is a real
property, and the mechanism above buys most of it back without the safety cost — a
mod is still shipped alone and added alone, and what it ships is source. What is
genuinely given up is dropping in a file and playing immediately.

What linking costs is that the mod set has to be linked, by somebody, before it
runs — and per-mod hot reload is gone, since replacing a mod means relinking. M3
answers the first: the player's machine links, from source, with a toolchain Ravel
ships, and the M3 measurement above is what that costs. The second is not
recovered and is recorded under "Consequences".

Against WebAssembly, the sandbox's advantages are containment and a typed
contract.

Containment is two separate arguments and they deserve separate answers. The
security one is settled by `SPEC.md` §10: mods are fully trusted and sandboxing is
not an initial requirement, and a sandbox would actively *restrict* what §10 says
trusted mods may do, since filesystem and native access would have to be
re-offered as host imports, mod by mod. Buying a restriction the spec does not
want, at 4.26× the selected call, is the wrong trade.

The reliability one is not settled by §10, and it is the argument that used to
favour the sandbox. Linking answers most of it directly. A mod that panics — the
ordinary bug, and the one that actually happens in modded Minecraft — is caught by
the host, observed rather than argued: unwinding stays inside one Rust program, so
the host kept running and reported which mod failed. What remains uncontained is a
mod that aborts its own process, and against that a sandbox is genuinely stronger.
It is also the rarer failure, and a sandbox contains only the crash and not the
bug: a mod that traps every tick is as unplayable as one that aborts. Paying 4.26×
on every call to convert a rare diagnosable crash into a degraded session is the
wrong trade, and if aborting mods turn out to dominate real use that is measurable
and it reopens this gate.

The typed contract is a real advantage over the C ABI, and it is not an advantage
over linking: a linked mod that does not satisfy the contract fails to compile,
which is a stricter check than instantiation-time validation and it happens before
anything ships.

The sandbox's 18 KiB mod artifact is genuinely attractive, and it is the one
number that flatters it — until the runtime is counted. The host binary carrying
the WebAssembly runtime measures 13.8 MiB, and the runtime is the bulk of it, so
the sandbox moves most of that from every mod into the loader once. The binary
without the runtime was never recorded, so the size of the shift is not evidence
here and nothing in this decision rests on it: the call cost is what decides.

Against a helper process, 3.2 µs per call ends the argument for synchronous mod
code. It stays the right model for anything asynchronous and out-of-band, which
is a later decision about tooling and not this one.

The 0.625 ns linking gives up is not a mystery: the linked host catches unwinding
on every call and the dynamic-library host cannot, because a panic crossing the C
ABI is undefined behaviour rather than a catchable event. The gap is the price of
the containment, and it is the containment that was the sandbox's best argument.

## Consequences

- **No maintained unsafe operation exists in the mod boundary.** No `libloading`,
  no symbol resolution, no `extern "C"`, so every crate on the mod side keeps
  `#![forbid(unsafe_code)]` and `safety-boundary.md`'s table stays empty.
- **Ravel owes no ABI stability.** The mod-facing API is a source API. Changing
  `HostMod`, `Registrar` or a handle type breaks builds, not installations, and a
  breaking change is a recompile rather than a migration. This is the property the
  decision was made for and it is also the obligation it creates: nothing else in
  the repository may quietly start treating a compiled artifact as an interface.
- **Ravel owns compile failures.** When a mod does not build on a player's
  machine, that is a Ravel bug report and not the author's, whatever the cause. It
  is an operational cost with no technical mitigation on this side of the choice:
  the diagnostics, the toolchain pinning, the reproduction of a failure on
  hardware nobody on the project owns. It is the largest cost of this decision and
  it is paid in support rather than in milliseconds.
- **A player installs 196 MB of toolchain and keeps a build cache resident.** The
  cache is 754 MB at 256 mods and it is not optional: it is what makes an update
  cost 1.6 s instead of 32 s, so evicting it turns every subsequent change into a
  cold build. Together with the 29 MB pack binary that is about a gigabyte of disk
  for a large pack, before the game.
- **The macOS signing obligation dissolves rather than being solved.** Apple
  Silicon refuses to load unsigned executable code, and `SPEC.md` §5.3 carried
  that as an open cost from the day it was written. Because the binary is produced
  on the machine that will run it, there is no other machine it could have been
  signed on: Ravel ad-hoc signs what it just compiled, a mod author never enrols
  with Apple, and Ravel never signs a binary containing third-party code. §5.3
  records the residue, which is narrow and technical — whether ad-hoc signing
  needs a full Xcode toolchain present. This decision did not set out to solve
  that problem and solved it anyway, which is worth noticing rather than claiming.
- **`package.schema.json` was wrong in a way that validated.** The original
  consequence is recorded under "What this closes and what it does not". The
  2026-08-08 closure migrated schema v1 in place to source-file declarations and
  made the old per-target form fail.
- **First install costs a minute on a modest machine**, and more on a slow one:
  73.8 s at 256 mods with four build jobs. That is a one-off, and it is a one-off
  the player experiences before they have played anything.
- **Replacing one mod means relinking the set.** No per-mod hot reload and no
  dropping a file into a directory. The measurement is what makes that acceptable
  — 1.6 s for an ordinary mod — and the honest exception is a library mod that
  much of the pack depends on, measured at 29.8 s.
- **A panicking mod does not end the game on the host-to-mod path.** The loader
  catches it and names it, so M2-W7's crash attribution reports a caught failure
  rather than reading a core dump. A mod that aborts still ends the process. The
  measured mod-to-mod path has no such guard: `StaticMod::peer` calls through
  `host_peer`, an `extern "C" fn`, so a panic there unwinds out of an `extern "C"`
  frame and aborts exactly as the dynamic library does. Production has to guard
  that path too, and this gate never measured what that costs.
- **The SDK owns the whole boundary and it is ordinary Rust.** No mod author
  writes `extern "C"`; `ravel-sdk-macros` generates registration, and a contract
  change that breaks a mod breaks its build.
- **Closed-source mods are not supported.** Source distribution means the source
  is distributed. That is a product consequence rather than a technical one, and
  it is the one this decision would be reopened for if the ecosystem turns out to
  need it — which is what the reversibility above is for.

## Limits of this evidence

### The four bindings

- These trials host no JVM. What they measure is the mod-execution boundary on
  its own; reaching it from the game costs what ADG-1 measured, and the two add.
  A hook that calls into a mod therefore costs about 10 ns to leave the JVM plus
  about 2 ns to enter the mod.
- The mod body is one cheap arithmetic step. That is deliberate — it makes the
  boundary the thing being measured — but it means these numbers are the floor,
  not a prediction of what a real mod costs.
- The helper process is measured unbatched, over a pipe, in one direction at a
  time. A shared-memory ring or a batching protocol would do better; neither can
  make a synchronous hook's answer arrive sooner, which is why the unbatched
  number is the one that matters here.
- Memory for the helper process is the sum of two processes sampled at the same
  boundary by two collectors, so it is a close total rather than a single
  reading.
- One host binary serves all four bindings, so its size is not attributable per
  candidate. No runtime cost is quoted anywhere in this record, because the binary
  built without the WebAssembly runtime was never recorded. The runtime's share of
  the 13.8 MiB is an unrecorded observation, not evidence.
- The linked trial links one mod. It measures no link time, binary growth, or
  compile time for a pack-sized set; the M3 measurement is what supplies those.
- Rust has no stable ABI and rlib metadata is compiler-version locked, so every
  mod in a set must be built by the same toolchain as the loader. Inside one link
  that is automatic, and shipping the toolchain is what makes it automatic on a
  player's machine too.
- The single-call and mod-to-mod figures are not guarded the same way. The linked
  single call at 2.50 ns runs inside `catch_unwind`; the 2.34 ns mod-to-mod figure
  does not, because `StaticMod::peer` calls straight through. That is why a path
  crossing the boundary twice measures *less* than one crossing it once, and it
  means the mod-to-mod row compares an unguarded linked call against the dynamic
  library. Guarding it would cost something this gate did not measure.

### The pack compile

The record's own `whatThisDoesNotSay` list is the source for these, and it is part
of the measurement rather than commentary on it.

- **The mods are generated, so they are alike, and none depends on another.** A
  real pack has mods of different sizes and a real dependency graph, and cargo
  cannot start a dependent crate until its dependency finishes, so a real set of
  the same total size parallelizes worse than this one. The chain variant is one
  graph, sixteen deep with uniform fan-out; a real pack's depth is unknown and its
  hub libraries differ wildly in popularity.
- **Nothing depends on anything outside serde and two Ravel crates.** Real mods
  pull real dependency trees, compiled once per pack rather than once per mod.
  That cost is omitted entirely rather than underestimated, and it lands on the
  cold path — which is the path this decision can least afford to be wrong about.
- **Every figure is a floor.** The synthetic mod is 1,161 lines against a real
  large mod's thousands, and each figure is a median of three runs on one machine
  on one afternoon. Three runs is not a distribution and nothing here is a
  percentile.
- **The four-core figure limits cargo's job count on a sixteen-thread machine.**
  It is not a four-core machine: the cores are the same fast cores, the cache is
  the whole chip's, and memory bandwidth is not contended the way a smaller part
  would contend it. Read it as the parallelism a player has, not as the machine.
- **Peak memory is a 25-millisecond sampled sum over the process tree**, so a
  spike shorter than the interval is missed and the true peak is at or above what
  is recorded. It understates the LTO variant most, whose peak is one short-lived
  process rather than a plateau.
- **The build ran against a warm cargo registry and a warm page cache and
  downloaded nothing.** A first install also fetches and unpacks the dependency
  crates, and that is not in the 73.8 s.
- **The warm rebuild changes a mod; it does not add one.** Adding a mod compiles a
  crate that was never compiled and relinks, which is between the warm and cold
  figures and is not measured.
- **This is the release profile with cargo's defaults.** A cheaper profile would
  compile faster and produce a slower game, and nothing here prices that trade.
- **Fat LTO is the expensive end of link-time optimization.** Thin LTO would cost
  less and buy less, and is not measured, so the opt-in above is specified against
  the only variant that has a number.
