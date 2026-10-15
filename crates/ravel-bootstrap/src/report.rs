//! Counts native hook invocations for the agent's shutdown report.

use std::sync::atomic::{AtomicU64, Ordering};

use jni::EnvUnowned;
use jni::objects::JClass;
use jni::sys::jlong;

static INVOCATIONS: AtomicU64 = AtomicU64::new(0);

// Counting occurs outside the panic guard and must remain infallible.
#[inline]
pub(crate) fn record_invocation() {
    INVOCATIONS.fetch_add(1, Ordering::Relaxed);
}

pub(crate) fn invocations() -> u64 {
    INVOCATIONS.load(Ordering::Relaxed)
}

/// Reports the native invocation count to the agent's shutdown line.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "nativeInvocations")]
pub fn native_invocations<'local>(_env: EnvUnowned<'local>, _class: JClass<'local>) -> jlong {
    // Saturate the diagnostic count rather than wrap into a negative JVM value.
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
