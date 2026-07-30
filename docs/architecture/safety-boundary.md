# Safety boundary

Ravel-maintained Rust and code written against the public SDK use
`#![forbid(unsafe_code)]`. The public mod API is safe Rust.

Unavoidable native loading, JNI, JVMTI, operating-system, or equivalent unsafe
operations may be encapsulated only by audited dependencies behind narrow safe
wrappers. The dependency audit records the owner, purpose, licence, advisory
status, and exposed invariants of each boundary. Ravel code does not weaken the
unsafe prohibition or add escape hatches to make a prototype pass.

Installed mods are trusted within the capabilities of the execution model.
Initial Ravel does not promise filesystem, network, process, or native-access
sandboxing. Trust does not remove package integrity checks, pre-execution
validation, side checks, structured failure attribution, or diagnostic
redaction.

If safe wrappers make a viable loader impossible or prevent measured targets
from being met, implementation pauses for an explicit product decision.
