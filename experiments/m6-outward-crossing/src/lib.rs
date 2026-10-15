//! Native side of the three ways a mod's game action could reach the game.
//!
//! Every crossing this repository ships runs inward: the game calls Rust and
//! Rust answers. A mod that wants a sound played needs the opposite, and there
//! are two shapes for it. Either Rust calls the game method itself through JNI,
//! or Rust answers with an instruction and the Java adapter makes the call —
//! which is the direction ADG-7's persistence adapter already runs in.
//!
//! The direct shape is measured twice, because resolving a method identifier is
//! not the same cost as invoking one and a real loader would cache it. Pricing
//! only the resolving form would overstate the cost of the candidate this gate
//! is most likely to reject, which is the way to get a gate wrong that nobody
//! reading the number afterwards can see.

use std::cell::Cell;

use jni::ids::JStaticMethodID;
use jni::objects::{JByteArray, JClass, JString, JValue};
use jni::signature::{JavaType, Primitive};
use jni::sys::{jdouble, jint, jvalue};
use jni::{Env, EnvUnowned, jni_sig, jni_str};

/// The identifier the game action refuses.
const FAILING_ACTION: jint = -1;

/// Bytes one deferred instruction occupies, matching `Action.INSTRUCTION_BYTES`.
const INSTRUCTION_BYTES: usize = 4 + 8;

thread_local! {
    /// The resolved identifier of the game method, in the candidate that caches.
    ///
    /// A cached identifier is only sound for as long as something holds the
    /// class it came from, which here is the class declaring the native method
    /// being executed — the JVM cannot unload it while its own native frame is
    /// on the stack. A loader caching across calls would have to hold that
    /// reference itself, and that obligation is part of what this candidate
    /// costs rather than an accident of the measurement.
    static CACHED: Cell<Option<JStaticMethodID>> = const { Cell::new(None) };
}

/// The argument one action carries, fixed so no candidate can vary the work.
fn argument(index: jint) -> jdouble {
    jdouble::from(index) * 0.5
}

/// Rust performs each action itself, resolving the method on every call.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_ravel_outward_Action_directLookup<'local>(
    mut unowned: EnvUnowned<'local>,
    class: JClass<'local>,
    actions: jint,
) -> jdouble {
    unowned
        .with_env(|env: &mut Env<'local>| -> jni::errors::Result<jdouble> {
            let mut last = 0.0;
            for index in 0..actions {
                last = env
                    .call_static_method(
                        &class,
                        jni_str!("perform"),
                        jni_sig!("(ID)D"),
                        &[JValue::Int(index), JValue::Double(argument(index))],
                    )?
                    .d()?;
            }
            Ok(last)
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// Rust performs each action itself through an identifier resolved once.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_ravel_outward_Action_directCached<'local>(
    mut unowned: EnvUnowned<'local>,
    class: JClass<'local>,
    actions: jint,
) -> jdouble {
    unowned
        .with_env(|env: &mut Env<'local>| -> jni::errors::Result<jdouble> {
            let method = if let Some(method) = CACHED.get() {
                method
            } else {
                let resolved =
                    env.get_static_method_id(&class, jni_str!("perform"), jni_sig!("(ID)D"))?;
                CACHED.set(Some(resolved));
                resolved
            };
            let mut last = 0.0;
            for index in 0..actions {
                // Safe because the identifier was resolved from this class for
                // this signature, and the class is the one declaring the frame
                // currently executing.
                let value = unsafe {
                    env.call_static_method_unchecked(
                        &class,
                        method,
                        JavaType::Primitive(Primitive::Double),
                        &[jvalue { i: index }, jvalue { d: argument(index) }],
                    )
                }?;
                last = value.d()?;
            }
            Ok(last)
        })
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// Rust answers with what should happen and calls nothing.
///
/// This is the shape `AudioDeclaration::play` already has — a request
/// describing what would be played — carried one step further, to the side that
/// can act on it.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_ravel_outward_Action_deferred<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
    actions: jint,
) -> JByteArray<'local> {
    unowned
        .with_env(
            |env: &mut Env<'local>| -> jni::errors::Result<JByteArray<'local>> {
                env.byte_array_from_slice(&instructions(actions))
            },
        )
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// The instruction array for `actions` actions, big-endian as Java reads it.
fn instructions(actions: jint) -> Vec<u8> {
    let count = usize::try_from(actions.max(0)).unwrap_or(0);
    let mut encoded = Vec::with_capacity(count * INSTRUCTION_BYTES);
    for index in 0..actions {
        encoded.extend_from_slice(&index.to_be_bytes());
        encoded.extend_from_slice(&argument(index).to_be_bytes());
    }
    encoded
}

/// Calls the action the game refuses, and reports what the native side is left
/// holding.
///
/// The answer is the axis a nanosecond cannot report: a thrown Java exception
/// stays pending on the calling thread, and every JNI call the native side
/// makes before clearing it is undefined. That obligation exists only in the
/// candidates that call outward.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_ravel_outward_Action_directFailure<'local>(
    mut unowned: EnvUnowned<'local>,
    class: JClass<'local>,
) -> JString<'local> {
    unowned
        .with_env(
            |env: &mut Env<'local>| -> jni::errors::Result<JString<'local>> {
                let outcome = env.call_static_method(
                    &class,
                    jni_str!("perform"),
                    jni_sig!("(ID)D"),
                    &[JValue::Int(FAILING_ACTION), JValue::Double(0.0)],
                );
                let described = match outcome {
                    Ok(_) => "the refused action returned a value".to_owned(),
                    Err(error) => {
                        // Nothing else may cross until this is cleared, including
                        // the call that builds the string describing it.
                        env.exception_clear();
                        format!("a pending exception ({error}), now cleared")
                    }
                };
                env.new_string(&described)
            },
        )
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

/// Answers with one instruction naming the action the game refuses.
#[unsafe(no_mangle)]
pub extern "system" fn Java_dev_ravel_outward_Action_deferredFailure<'local>(
    mut unowned: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JByteArray<'local> {
    unowned
        .with_env(
            |env: &mut Env<'local>| -> jni::errors::Result<JByteArray<'local>> {
                let mut encoded = Vec::with_capacity(INSTRUCTION_BYTES);
                encoded.extend_from_slice(&FAILING_ACTION.to_be_bytes());
                encoded.extend_from_slice(&0.0_f64.to_be_bytes());
                env.byte_array_from_slice(&encoded)
            },
        )
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[cfg(test)]
mod tests {
    use super::{INSTRUCTION_BYTES, argument, instructions};

    #[test]
    fn an_instruction_array_is_one_fixed_width_record_per_action() {
        assert_eq!(instructions(4).len(), 4 * INSTRUCTION_BYTES);
        assert_eq!(instructions(0), [] as [u8; 0]);
    }

    #[test]
    fn an_instruction_encodes_the_argument_the_direct_candidates_pass() {
        let encoded = instructions(2);
        let second = &encoded[INSTRUCTION_BYTES..];
        assert_eq!(&second[..4], &1_i32.to_be_bytes());
        assert_eq!(&second[4..], &argument(1).to_be_bytes());
    }
}
