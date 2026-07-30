# Safety boundary

Ravel-maintained Rust and code written against the public SDK use
`#![forbid(unsafe_code)]`. The public mod API is safe Rust.

Unavoidable native loading, JNI, JVMTI, operating-system, or equivalent unsafe
operations may be encapsulated only by audited dependencies behind narrow safe
wrappers. The dependency audit records the owner, purpose, licence, advisory
status, and exposed invariants of each boundary. Ravel code does not weaken the
unsafe prohibition or add escape hatches to make a prototype pass.

## There is no boundary crate

| Operation | Crate | Dependency |
| --- | --- | --- |
| *(none)* | — | — |

The list is empty, and that is a finding rather than an aspiration. M1 spent most
of its length believing otherwise: an exported symbol carrying the JNI-mangled
callback name looked like a declaration the runtime resolves rather than a call a
wrapper can hide, so `SPEC.md` §3.3 was drafted to admit exactly one such operation
in `ravel-bootstrap`.

The M1-W8 review refuted the premise. The pinned `jni` 0.22.4 ships
`#[jni::jni_mangle("class", "method")]`, which emits the export itself, and a crate
that keeps `#![forbid(unsafe_code)]` verbatim — no `allow`, no `expect`, no
relaxation of any kind — compiles clean and exports the mangled symbol. That was
verified by building such a crate and reading its symbol table, not by reading
documentation. Because the attribute takes class and method as string literals, the
binding generator emits it, so the declaration is generated rather than written.

Two consequences worth stating:

- **`ravel-bootstrap` is an ordinary crate.** It keeps `#![forbid(unsafe_code)]`
  like every other, needs no lint relaxation, and no crate has to keep it out of
  its dependency graph on safety grounds. The layering rules in
  [`dependency-rules.md`](dependency-rules.md) still apply for their own reasons.
- **`RegisterNatives` still cannot reach zero.** `Env::register_native_methods` and
  `NativeMethod::from_raw_parts` are `unsafe fn`, and an unsafe call is not
  something an attribute can hide. ADG-1's rejection of the hosted VM therefore
  holds more strongly than the spike measured: zero against two, not one against
  two.

Adding a row needs an accepted ADR and an independent review, and the bar is the one
M1 failed to clear — showing that no audited dependency can hold the operation,
rather than that none was found. Measurement alternatives live in `experiments/`,
which is fenced out of the production graph by `ravel:verify-scaffold` and may hold
whatever unsafe a candidate needs to be measured at all.

The spike code still contains hand-written `#[unsafe(no_mangle)]` exports, and those
are not what the recorded counts describe. `maintainedUnsafeBlocks` is what a
production implementation of a candidate would need in maintained crates, so the
five hand-written exports in `experiments/adg-4-state` are counted by nothing. Where
a result records a non-zero count — the hosted VM at two, JVMTI at four, the dynamic
library at two — the operation is an `unsafe fn` no attribute can generate away, and
that is a floor rather than an artefact of how the spike was written.

Installed mods are trusted within the capabilities of the execution model.
Initial Ravel does not promise filesystem, network, process, or native-access
sandboxing. Trust does not remove package integrity checks, pre-execution
validation, side checks, structured failure attribution, or diagnostic
redaction.

If safe wrappers make a viable loader impossible or prevent measured targets
from being met, implementation pauses for an explicit product decision.
