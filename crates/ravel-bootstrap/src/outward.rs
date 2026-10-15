//! Queues mod-requested actions for the Java adapter to drain.
//!
//! Each verb carries an argument count so readers can skip unknown verbs.

use std::sync::{Mutex, PoisonError};

use jni::objects::{JClass, JString};
use jni::{Env, EnvUnowned};

use crate::guard::HookError;
use crate::wire::{VERSION, count, field};

/// Actions queued since the adapter last drained them.
static PENDING: Mutex<Vec<Action>> = Mutex::new(Vec::new());

/// Serializes tests sharing the process-wide action queue.
#[cfg(test)]
pub(crate) static SERIAL: Mutex<()> = Mutex::new(());

/// Adapter verb for a declared sound.
const PLAY: &str = "audio.play";

/// One thing the game should do, in the shape the adapter performs it.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Action {
    verb: &'static str,
    arguments: Vec<String>,
}

/// Asks for a declared sound to be played on the client.
pub fn play(owner: &str, entry: &str) {
    queue(Action {
        verb: PLAY,
        arguments: vec![owner.to_owned(), entry.to_owned(), "1".to_owned()],
    });
}

fn queue(action: Action) {
    PENDING
        .lock()
        .unwrap_or_else(PoisonError::into_inner)
        .push(action);
}

/// Encodes and clears queued actions; an empty queue returns an empty string.
pub fn drain() -> String {
    let taken = std::mem::take(&mut *PENDING.lock().unwrap_or_else(PoisonError::into_inner));
    if taken.is_empty() {
        return String::new();
    }
    encode(&taken)
}

/// Writes the payload the adapter reads.
fn encode(actions: &[Action]) -> String {
    let mut out = String::new();
    field(&mut out, VERSION);
    count(&mut out, actions.len());
    for action in actions {
        field(&mut out, action.verb);
        count(&mut out, action.arguments.len());
        for argument in &action.arguments {
            field(&mut out, argument);
        }
    }
    out
}

/// Hands the adapter every action queued since it last asked.
///
/// # Panics
///
/// Panics become Java `RuntimeException`s rather than unwinding into the JVM.
#[must_use]
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelOutwardActions")]
pub fn ravel_outward_actions<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    env.with_env(
        |env: &mut Env<'local>| -> Result<JString<'local>, HookError> {
            Ok(env.new_string(drain())?)
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[cfg(test)]
mod tests {
    use std::sync::PoisonError;

    use super::{Action, PLAY, drain, encode, play};

    fn action(verb: &'static str, arguments: &[&str]) -> Action {
        Action {
            verb,
            arguments: arguments
                .iter()
                .map(|argument| (*argument).to_owned())
                .collect(),
        }
    }

    // Keep this literal in sync with jvm/harness/template/driver/OutwardMain.java.in.
    #[test]
    fn one_action_encodes_to_the_payload_the_adapter_is_written_against() {
        let payload = encode(&[action(PLAY, &["audio", "door_opens", "1"])]);

        assert_eq!(payload, "1:11:110:audio.play1:35:audio10:door_opens1:1");
    }

    /// The two-action payload the same harness decodes, byte for byte.
    #[test]
    fn actions_are_carried_by_one_payload_however_many_there_are() {
        let payload = encode(&[
            action(PLAY, &["audio", "door_opens", "1"]),
            action(PLAY, &["audio", "door_closes", "2"]),
        ]);

        assert_eq!(
            payload,
            "1:11:210:audio.play1:35:audio10:door_opens1:110:audio.play1:35:audio11:door_closes1:2"
        );
    }

    #[test]
    fn draining_answers_what_was_queued_and_empties_it() {
        let _serial = super::SERIAL.lock().unwrap_or_else(PoisonError::into_inner);
        play("audio", "door_opens");

        assert_eq!(drain(), "1:11:110:audio.play1:35:audio10:door_opens1:1");
        assert_eq!(drain(), "");
    }

    #[test]
    fn an_argument_carrying_the_separator_is_read_back_whole() {
        let payload = encode(&[action(PLAY, &["a:b", "3:xy"])]);

        assert_eq!(payload, "1:11:110:audio.play1:23:a:b4:3:xy");
    }
}
