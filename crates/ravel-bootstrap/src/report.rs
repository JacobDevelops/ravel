//! What the agent's shutdown line reports.
//!
//! The M2 launch spike proved the JVM took Ravel's two arguments and the client ran.
//! It could not prove the class rewrite happened, because nothing in the spike
//! agent said whether it had transformed anything. Counting on both sides and
//! printing one line at shutdown is what turns an ordinary in-game launch into
//! direct evidence, and it is cheap enough to leave on.

use std::sync::atomic::{AtomicU64, Ordering};

use jni::EnvUnowned;
use jni::objects::JClass;
use jni::sys::jlong;

/// Hook invocations that reached native code since the library was loaded.
static INVOCATIONS: AtomicU64 = AtomicU64::new(0);

/// Counts one hook invocation.
///
/// This is the only work a hook does outside the panic guard, and it is the only
/// work that may be: an atomic add allocates nothing, indexes nothing, calls
/// nothing fallible, and therefore cannot unwind. Everything that can panic
/// happens inside [`crate::guard::enter`]'s guarded closure.
#[inline]
pub(crate) fn record_invocation() {
    INVOCATIONS.fetch_add(1, Ordering::Relaxed);
}

/// How many hook invocations have reached native code.
pub(crate) fn invocations() -> u64 {
    INVOCATIONS.load(Ordering::Relaxed)
}

/// Reports the native invocation count to the agent's shutdown line.
///
/// The declaration is written rather than generated because it belongs to the
/// bootstrap rather than to any version's hook surface, but it is the same
/// mechanism: `jni_mangle` emits the export, so this crate keeps
/// `#![forbid(unsafe_code)]` verbatim.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "nativeInvocations")]
pub fn native_invocations<'local>(_env: EnvUnowned<'local>, _class: JClass<'local>) -> jlong {
    // The count is reported for a log line, so saturating is the right answer:
    // a count this large is already a bug and clamping it says so.
    jlong::try_from(invocations()).unwrap_or(jlong::MAX)
}

#[cfg(test)]
mod tests {
    use super::{invocations, record_invocation};

    #[test]
    fn every_invocation_is_counted_once() {
        let before = invocations();

        record_invocation();
        record_invocation();

        assert_eq!(invocations(), before + 2);
    }
}
