//! The crossing an action a mod asked for leaves Ravel by.
//!
//! An action crosses outward as data and the Java adapter makes the call; see
//! `docs/architecture/overview.md#mechanisms`. `content.rs` applies this to one
//! verb, with the verb's shape written into the payload:
//! a row is a kind and the behaviour fields that kind declares, so the reader
//! must know every kind to read any row. This module is the same crossing with
//! the verb left open, which is the whole of what it adds — an action states how
//! many arguments it carries, so the adapter can step over a verb it does not
//! know instead of losing the rest of the payload with it.
//!
//! That is the only reason this exists rather than a second `content.rs`. Audio
//! is the first verb and packets are meant to be the second, and a mechanism
//! that has to be re-derived per verb is the shape `ROADMAP.md`'s M8-W0 prose
//! refuses: "the mechanism is built once and reused".
//!
//! # What reaches this queue
//!
//! [`HostMod::take_play_requests`] is the near end. `runtime.rs` asks every mod
//! for what it asked the game for after each delivery and queues the answers
//! here, so a request is attributable to the mod that made it and no mod holds
//! a channel into this module. What is still unobserved is the far end in a
//! real game: no launched Minecraft has performed one of these, and
//! `docs/guide/limits.md` says so where an author reads it.
//!
//! [`HostMod::take_play_requests`]: ravel_host_api::HostMod::take_play_requests

use std::sync::{Mutex, PoisonError};

use jni::objects::{JClass, JString};
use jni::{Env, EnvUnowned};

use crate::guard::HookError;
use crate::wire::{VERSION, count, field};

/// Actions queued since the adapter last drained them.
///
/// A `Vec` rather than a bounded queue because a drain empties it whole and the
/// only thing that fills it is a mod asking for something between two drains.
/// The ceiling is therefore one client tick of requests; if a verb ever arrives
/// that a mod can ask for in an unbounded loop, this is where the bound goes.
static PENDING: Mutex<Vec<Action>> = Mutex::new(Vec::new());

/// Held by every test that reads or writes [`PENDING`].
///
/// The queue is process-wide and the test harness runs threads, so two tests
/// that both drain would otherwise take each other's actions. Not a lock the
/// running loader needs: one drain per client tick is already serialised by the
/// tick.
#[cfg(test)]
pub(crate) static SERIAL: Mutex<()> = Mutex::new(());

/// The verb that plays a sound a mod declared.
///
/// A constant rather than a literal at the call site, because the adapter
/// switches on the same word and the two have to agree exactly.
const PLAY: &str = "audio.play";

/// One thing the game should do, in the shape the adapter performs it.
#[derive(Clone, Debug, Eq, PartialEq)]
struct Action {
    verb: &'static str,
    arguments: Vec<String>,
}

/// Asks for a declared sound to be played on the client.
///
/// The pitch is stated here rather than defaulted on the other side, because
/// `PlayRequest` carries no pitch and something has to decide one. Rust decides
/// and Java performs, including for a value a mod did not give.
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

/// Encodes everything queued and clears it.
///
/// Public because it is what the crossing answers, and a test that cannot ask
/// what an instruction encodes to can only prove the queue accepted it.
///
/// Answers an empty payload when nothing is queued rather than a payload
/// declaring nothing. The adapter runs once per client tick and a mod asks for
/// something on almost none of them, so the common case is a string the other
/// side can reject on its length.
pub fn drain() -> String {
    let taken = std::mem::take(&mut *PENDING.lock().unwrap_or_else(PoisonError::into_inner));
    if taken.is_empty() {
        return String::new();
    }
    encode(&taken)
}

/// Writes the payload the adapter reads.
///
/// The framing is `wire.rs`'s, for the reason that module gives: an argument is
/// free-form text a mod wrote, so a length is what says where one ends.
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
/// Does not panic. The guard around the body turns a panic into a thrown
/// `RuntimeException` rather than an unwind into the JVM.
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

    /// The payload `jvm/harness/template/driver/OutwardMain.java.in` decodes, byte for byte.
    ///
    /// Written out rather than computed, because the two sides of this crossing
    /// are two languages and a shared helper would only prove this side agrees
    /// with itself. Changing it means changing the literal in that harness too.
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
