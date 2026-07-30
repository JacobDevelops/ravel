# ADG-2 — Rust mod execution and ABI

Status: **measured at M1-W3; the decision is deferred to M3.** Four candidates were
measured and the measurements stand. Choosing between them is not.

## What is decided and what is not

Nothing about how a mod executes is decided here. That is deliberate, and it is a
correction rather than an omission: this gate selected native dynamic libraries at
M1-W3, revised the selection to linked mods at M1-W6 on the same evidence, and the
second revision exposed that the deciding considerations — distribution, hot
reload, who links a mod set, what a package contains — are all properties of a
loader that does not exist yet. A decision that flips twice before anything is
built is a decision taken too early.

So M3 decides, against the loader prototype. What M1 contributes is the cost of
each option and the trade each one makes, which is what a later decision needs and
does not have to re-measure.

**The leading candidate is a mod linked into the loader**, on this evidence: 2.50 ns
a call, no maintained unsafe anywhere in the mod boundary, a panicking mod the host
survives, contract drift that fails the build, and cheaper mod-to-mod calls than
any alternative. What it gives up is independent per-mod distribution, and that is
exactly the property no one has tested yet.

**The alternative to beat it is a native dynamic library**: 0.625 ns faster, and it
buys drop-in per-mod binaries for two maintained unsafe calls and a process that
dies whenever any mod panics. Neither WebAssembly nor a helper process is close on
call cost, and both stay recorded for the day their reason for existing arrives.

That alternative is retained under a bar it has not cleared. The M1 measurement
gate's failure actions reject a candidate requiring a maintained unsafe *call*, and
`libloading` needs two — the same rule on which ADG-1 rejected JVMTI at four and
the hosted VM at two. This gate does not reject the dynamic library, because M3
decides the execution model and rejecting the only drop-in candidate now would
decide it here. Choosing it at M3 therefore costs more than a preference: it takes
an accepted `SPEC.md` §3.3 amendment and a row in
[`safety-boundary.md`](../architecture/safety-boundary.md), whose bar is showing
that no audited dependency can hold the operation rather than that none was found.
Two `unsafe fn` calls in a crate that exists do not obviously clear it. A reader
should treat the dynamic library as measured and retained, not as equally
available.

Deferring has a consequence worth stating: several documents in this repository
still assume the dynamic-library shape, including `schemas/package.schema.json`,
which requires per-target artifacts and is enforced by `ravel:verify`. Those are
now recorded assumptions rather than settled contracts, and M3 owns reconciling
them with whatever it picks.

## Context

`SPEC.md` §10 says installed mods are fully trusted and may do anything their
execution model allows, with sandboxing explicitly not an initial requirement. So
the question ADG-2 answers is not "how do we contain a mod" but "what does it cost
to call one, and what does its contract cost to keep stable".

`SPEC.md` §3.4 also said users install precompiled mod packages and never compile
Rust locally. That was an assumption written before anything was built, and this
gate is the first evidence bearing on it. The measurements say the packaging model
is not free: buying per-mod binaries costs the safety property and the failure
containment, and it buys them back from a distribution convenience nobody has
tested yet. §3.4 is amended to record the open question rather than the
assumption.

## Evidence

Measured on `x86_64-linux`, hardware profile `linux-reference-v1`. Reproduce with:

```console
devenv tasks run ravel:spike-adg-2
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
property and it is why the candidate stays measured. It is not a property anything
has yet tested, and it costs the two safety properties above to hold. The order
this repository works in is that the loader teaches us what mods need, so the
convenience is deferred rather than bought on credit.

What linking costs is that the mod set has to be linked, by somebody, before it
runs — and per-mod hot reload is gone, since replacing a mod means relinking. Both
land on M2, which is where a loader capable of feeling them exists.

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

## What follows if the leading candidate wins

Nothing here is decided. These are the consequences M3 would be accepting by
choosing linked mods, stated so the choice can be made with them in view.

- **No maintained unsafe operation would exist in the mod boundary.** No
  `libloading`, no symbol resolution, no `extern "C"`, so every crate on the mod
  side keeps `#![forbid(unsafe_code)]`. The dynamic library is the candidate this
  costs: it needs two.
- **The unit of distribution would be a linked set.** Either the installing machine
  links a pinned toolchain's output, or a pack is built once and installed as a
  binary. `SPEC.md` §3.4 records this as open, and picking now would be picking
  without the loader that has to live with it.
- **Replacing one mod would mean relinking the set.** No per-mod hot reload, no
  dropping a file into a directory. That puts a requirement on M2's tooling —
  relinking has to be cheap enough to iterate on — rather than on the mod format.
- **A panicking mod would not end the game on the host-to-mod path.** The loader
  catches it and names it, so M2-W7's crash attribution reports a caught failure
  rather than reading a core dump. A mod that aborts still ends the process, and
  that is the residual cost recorded above. The measured mod-to-mod path has no
  such guard: `StaticMod::peer` calls through `host_peer`, an `extern "C" fn`, so a
  panic there unwinds out of an `extern "C"` frame and aborts exactly as the
  dynamic library does. Production would have to guard that path too, and this gate
  never measured what that costs.
- **The SDK would own the whole boundary and it would be ordinary Rust.** No mod
  author writes `extern "C"`; `ravel-sdk-macros` generates registration, and a
  contract change that breaks a mod breaks its build.

## Limits of this evidence

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
- The linked trial links one mod. Nothing here measures link time, binary growth,
  or compile time for a pack-sized set, and those are exactly the numbers the
  distribution question turns on. They are M2's to measure, and they are the reason
  this record does not also choose who links.
- Rust has no stable ABI and rlib metadata is compiler-version locked, so every
  mod in a set must be built by the same toolchain as the loader. Inside one link
  that is automatic. It is only a constraint on whoever performs the link, which is
  the open question above rather than a property of the mod call.
- The single-call and mod-to-mod figures are not guarded the same way. The linked
  single call at 2.50 ns runs inside `catch_unwind`; the 2.34 ns mod-to-mod figure
  does not, because `StaticMod::peer` calls straight through. That is why a path
  crossing the boundary twice measures *less* than one crossing it once, and it
  means the mod-to-mod row compares an unguarded linked call against the dynamic
  library. Guarding it would cost something this gate did not measure.
