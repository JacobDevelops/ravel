//! JNI input admission and wire encoding for world persistence.

use jni::objects::{JByteArray, JClass, JString};
use jni::sys::{JNI_FALSE, jboolean};
use jni::{Env, EnvUnowned};
use ravel_save::{MAX_AGGREGATE_BYTES, SavedDataError};

use super::{OpenOutcome, SaveOutcome, open_world_data, save_world_data};
use crate::guard::{HookError, hand_document, text};

enum Incoming {
    Absent,
    /// Empty arrays are present but unreadable, never fresh state.
    Present(Vec<u8>),
    Oversized,
}

// Check the JNI array length before allocating native storage.
fn incoming<'local>(
    env: &Env<'local>,
    aggregate: &JByteArray<'local>,
) -> Result<Incoming, HookError> {
    if aggregate.is_null() {
        return Ok(Incoming::Absent);
    }
    let length = aggregate.len(env)?;
    if length > MAX_AGGREGATE_BYTES {
        return Ok(Incoming::Oversized);
    }
    Ok(Incoming::Present(env.convert_byte_array(aggregate)?))
}

/// Returns empty text on admission or a diagnostic on refusal; null also refuses.
///
/// # Panics
/// Panics are translated into a JVM exception by the JNI guard.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelOpenWorldPersistence")]
pub fn ravel_open_world_persistence<'local>(
    env: EnvUnowned<'local>,
    _class: JClass<'local>,
    aggregate: JByteArray<'local>,
    identity: JString<'local>,
    fresh_world: jboolean,
) -> JString<'local> {
    hand_document(env, |env| {
        let identity = text(env, &identity)?;
        let outcome = match incoming(env, &aggregate)? {
            Incoming::Absent => open_world_data(&identity, None, fresh_world != JNI_FALSE),
            Incoming::Present(bytes) => {
                open_world_data(&identity, Some(&bytes), fresh_world != JNI_FALSE)
            }
            Incoming::Oversized => OpenOutcome::from(SavedDataError::AggregateTooLarge {
                limit: MAX_AGGREGATE_BYTES,
            }),
        };
        Ok(Some(match outcome {
            OpenOutcome::Staged => String::new(),
            OpenOutcome::Refused(diagnostic) => publishable(&diagnostic),
        }))
    })
}

const UNCHANGED: u8 = 0x00;
const REPLACEMENT: u8 = 0x01;
const FAILED: u8 = 0x02;
const IDENTITY_MISMATCH: u8 = 0x03;

fn tagged(tag: u8, payload: &[u8]) -> Vec<u8> {
    let mut answer = Vec::with_capacity(payload.len() + 1);
    answer.push(tag);
    answer.extend_from_slice(payload);
    answer
}

fn publishable(diagnostic: &str) -> String {
    ravel_core::DiagnosticMessage::redacted(diagnostic)
        .text()
        .to_owned()
}

fn save_answer(outcome: SaveOutcome) -> Vec<u8> {
    match outcome {
        // Java writes nothing for both vetoed and unchanged saves.
        SaveOutcome::Unchanged | SaveOutcome::Withheld => vec![UNCHANGED],
        SaveOutcome::Replacement(bytes) => tagged(REPLACEMENT, &bytes),
        SaveOutcome::Failed(detail) => tagged(FAILED, publishable(&detail).as_bytes()),
        SaveOutcome::IdentityMismatch(detail) => {
            tagged(IDENTITY_MISMATCH, publishable(&detail).as_bytes())
        }
    }
}

/// Returns a tagged save result; null or failure leaves vanilla saving untouched.
///
/// # Panics
/// Panics are translated into a JVM exception by the JNI guard.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelSaveWorldPersistence")]
pub fn ravel_save_world_persistence<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
    identity: JString<'local>,
) -> JByteArray<'local> {
    env.with_env(
        |env: &mut Env<'local>| -> Result<JByteArray<'local>, HookError> {
            let identity = text(env, &identity)?;
            let answer = save_answer(save_world_data(&identity));
            Ok(env.byte_array_from_slice(&answer)?)
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use super::{
        FAILED, IDENTITY_MISMATCH, REPLACEMENT, UNCHANGED, publishable, save_answer, tagged,
    };
    use crate::persistence::SaveOutcome;

    #[test]
    fn the_save_answer_tags_what_java_should_do_and_only_one_tag_writes() {
        assert_eq!(vec![UNCHANGED], tagged(UNCHANGED, &[]));
        assert_eq!(tagged(REPLACEMENT, b"stored"), b"\x01stored".to_vec());
        assert_eq!(tagged(FAILED, b"why"), b"\x02why".to_vec());
        assert_eq!(tagged(IDENTITY_MISMATCH, b"whose"), b"\x03whose".to_vec());
        assert_eq!(
            BTreeSet::from([UNCHANGED, REPLACEMENT, FAILED, IDENTITY_MISMATCH]).len(),
            4
        );
    }

    #[test]
    fn persistence_diagnostics_are_redacted_before_the_jni_output() {
        let detail = concat!(
            "could not read /home/player/.minecraft/world, token=",
            "ghp_",
            "secretvalue"
        );
        let published = publishable(detail);
        assert!(
            !published.contains(concat!("ghp_", "secretvalue")),
            "{published}"
        );
        assert!(!published.contains("/home/player"), "{published}");
        assert!(published.contains("redacted"), "{published}");
        for (tag, outcome) in [
            (FAILED, SaveOutcome::Failed(detail.to_owned())),
            (
                IDENTITY_MISMATCH,
                SaveOutcome::IdentityMismatch(detail.to_owned()),
            ),
        ] {
            let answer = save_answer(outcome);
            assert_eq!(answer[0], tag);
            let published = String::from_utf8_lossy(&answer[1..]);
            assert!(
                !published.contains(concat!("ghp_", "secretvalue")),
                "{published}"
            );
            assert!(!published.contains("/home/player"), "{published}");
        }
    }
}
