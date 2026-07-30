//! ADG-1 native callback shared by both in-process entry candidates.
//!
//! The same callback body is reached two ways: through an exported, JNI-mangled
//! symbol that `System.loadLibrary` resolves, and through a function pointer
//! registered with `RegisterNatives`. Only the binding differs, so the measured
//! difference is the binding rather than the work.

use std::process;
use std::sync::atomic::{AtomicI64, Ordering};

use jni::objects::JClass;
use jni::sys::jlong;
use jni::{Env, EnvUnowned, jni_str};

/// Payload the Java side sends to test invalid input.
pub const INVALID_PAYLOAD: jlong = -1;
/// Payload the Java side sends to make the callback itself fail.
pub const FAILING_PAYLOAD: jlong = jlong::MIN;
/// Payload the Java side sends to abort the process inside native code.
pub const CRASH_PAYLOAD: jlong = jlong::MIN + 1;

static ACCUMULATOR: AtomicI64 = AtomicI64::new(0);

/// Callback implementation reached by both bindings.
///
/// The hot path deliberately avoids upgrading the environment: a hook that only
/// needs its arguments should not pay for an attach guard and a `catch_unwind`
/// on every invocation. The failure paths, which must reach the JVM to throw,
/// do upgrade.
pub extern "system" fn callback(
    mut unowned: EnvUnowned<'_>,
    _class: JClass<'_>,
    payload: jlong,
) -> jlong {
    if payload > 0 {
        return ACCUMULATOR.fetch_add(payload, Ordering::Relaxed) + payload;
    }

    match payload {
        CRASH_PAYLOAD => {
            // Deliberate crash for the ADG-1 failure matrix.
            process::abort();
        }
        FAILING_PAYLOAD => {
            let outcome = unowned.with_env(|env: &mut Env<'_>| {
                env.throw_new(
                    jni_str!("java/lang/IllegalStateException"),
                    jni_str!("callback failed"),
                )
            });
            let () = outcome.resolve::<jni::errors::ThrowRuntimeExAndDefault>();
            -1
        }
        _ => INVALID_PAYLOAD,
    }
}

/// Exported entry point resolved by `System.loadLibrary` and JNI name mangling.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_ravel_spike_Hook_onNativeCallback(
    unowned: EnvUnowned<'_>,
    class: JClass<'_>,
    payload: jlong,
) -> jlong {
    callback(unowned, class, payload)
}
