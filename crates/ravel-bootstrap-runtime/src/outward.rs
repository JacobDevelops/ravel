//! Queues mod-requested actions for the Java adapter to drain.
//!
//! Each verb carries an argument count so readers can skip unknown verbs.

use jni::objects::{JClass, JString};
use jni::{Env, EnvUnowned};

use crate::guard::HookError;
use crate::wire::{VERSION, count, field};

/// Adapter verb for a declared sound.
const PLAY: &str = "audio.play";

/// One thing the game should do, in the shape the adapter performs it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub(crate) struct Action {
    verb: &'static str,
    arguments: Vec<String>,
}

/// Asks for a declared sound to be played on the client.
pub(crate) fn play(id: &ravel_core::ContentId) -> Action {
    Action {
        verb: PLAY,
        arguments: vec![
            id.namespace().to_string(),
            id.path().to_owned(),
            "1".to_owned(),
        ],
    }
}

/// Encodes and clears queued actions; an empty queue returns an empty string.
pub(crate) fn drain(actions: &mut Vec<Action>) -> String {
    let taken = std::mem::take(actions);
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
            let payload = crate::runtime::with_selected(|runtime| runtime.drain_outward())
                .unwrap_or_default();
            Ok(env.new_string(payload)?)
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[cfg(test)]
mod tests {

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
        let id = "audio:door_opens".parse().expect("valid audio id");
        let mut queued = vec![play(&id)];

        assert_eq!(
            drain(&mut queued),
            "1:11:110:audio.play1:35:audio10:door_opens1:1"
        );
        assert_eq!(drain(&mut queued), "");
    }

    #[test]
    fn an_argument_carrying_the_separator_is_read_back_whole() {
        let payload = encode(&[action(PLAY, &["a:b", "3:xy"])]);

        assert_eq!(payload, "1:11:110:audio.play1:23:a:b4:3:xy");
    }
}
