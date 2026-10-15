//! The admitted-content crossing.
//!
//! `ravel-loader`'s registration barrier already decides *that* a mod owns
//! `block:example_mod:copper`: two mods cannot claim one identifier, and a claim
//! outside the window is refused. What it cannot do is make the block exist. A
//! registry in the running game is where that happens, and Java can call Rust
//! without a reverse call into the JVM — so this is the read the Java
//! adapter performs at the last moment the built-in registries still accept
//! entries, and the registration itself is Java's.
//!
//! # Why a read rather than a hook
//!
//! A hook observes or vetoes, and nothing else: it cannot
//! change an argument, substitute a return value, or call back into the JVM.
//! Registering content is an *action*, so the hook fires and a
//! hand-written Java adapter performs the action using its own crossing; see
//! `docs/architecture/overview.md#mechanisms`. This is that
//! crossing, on the framing `mods.rs` and `settings.rs` already use.
//!
//! # Only what a registry can take
//!
//! [`ContentKind`] names four kinds and two of them are registry entries. A
//! recipe is data the recipe manager reloads and a command is registered against
//! a dispatcher that does not exist yet at this seam, so neither crosses here.
//! They are filtered on this side rather than skipped on the other, because
//! which claims are registrable is an admission question:
//! Rust decides, Java performs.

use std::sync::{Arc, Mutex, PoisonError};

use jni::objects::{JClass, JString};
use jni::{Env, EnvUnowned};
use ravel_core::{ContentBehaviour, ContentKind};

use crate::guard::HookError;
use crate::runtime::LoaderRuntime;
use crate::wire::{VERSION, count, field, flag};

/// The installed runtime, or nothing before one is installed.
static RUNTIME: Mutex<Option<Arc<LoaderRuntime>>> = Mutex::new(None);

/// Makes the admitted content readable by the registration adapter.
///
/// Called wherever the runtime is installed rather than by a second caller, so
/// there is no launch in which the barrier admits a claim and the adapter cannot
/// see it.
pub(crate) fn install(runtime: Arc<LoaderRuntime>) {
    *lock() = Some(runtime);
}

/// Takes the store, recovering it from a previous panic.
///
/// Nothing here mutates the runtime, so a poisoned lock cannot mean a half-built
/// one.
fn lock() -> std::sync::MutexGuard<'static, Option<Arc<LoaderRuntime>>> {
    RUNTIME.lock().unwrap_or_else(PoisonError::into_inner)
}

/// Whether a kind names a registry this crossing can commit to.
const fn is_registrable(kind: ContentKind) -> bool {
    matches!(kind, ContentKind::Block | ContentKind::Item)
}

/// Everything admitted, as one payload.
///
/// Returns nothing before a set is installed, which is a launch that has not got
/// far enough to have claimed anything rather than a failure.
fn document() -> Option<String> {
    // The runtime leaves the store's lock behind before the payload is built,
    // for the reason `mods.rs` gives: nothing is held while Java works.
    document_of(lock().clone().as_deref())
}

/// The payload one runtime answers with, or nothing without one.
///
/// Separate from [`document`] so the absent case is a value a caller passes
/// rather than the state of the process's own store.
fn document_of(runtime: Option<&LoaderRuntime>) -> Option<String> {
    Some(runtime?.content_document())
}

impl LoaderRuntime {
    /// Every admitted registry claim this runtime holds, as one payload.
    ///
    /// The order is the claim table's own, which is sorted by identifier. That
    /// is deliberate: what a game registers must not depend on the order mods
    /// happened to claim in, because two launches of the same set would then
    /// produce two registries.
    pub(crate) fn content_document(&self) -> String {
        let claimed: Vec<_> = self
            .inner
            .lock()
            .unwrap_or_else(PoisonError::into_inner)
            .machine
            .claimed_content()
            .filter(|(key, _, _)| is_registrable(key.kind()))
            .map(|(key, owner, behaviour)| (key, owner.clone(), behaviour))
            .collect();

        let mut out = String::new();
        field(&mut out, VERSION);
        count(&mut out, claimed.len());
        for (key, owner, behaviour) in claimed {
            field(&mut out, key.kind().as_str());
            field(&mut out, key.id().namespace().as_str());
            field(&mut out, key.id().path());
            // The owner as well as the namespace. They are the same identity in
            // every claim the barrier admits today, and carrying only one would
            // make a future where they differ unreadable rather than wrong.
            field(&mut out, owner.as_str());
            describe(&mut out, behaviour);
        }
        out
    }
}

/// Writes how one entry behaves, as the fields its kind declares.
///
/// The field count is fixed per kind rather than tagged, because the kind is
/// already the field before it: a reader that knows it is looking at a block
/// knows how many fields follow. A number crosses as text for the reason every
/// other field does — the framing has one field type and adding a second would
/// be a second parser on the other side.
fn describe(out: &mut String, behaviour: ContentBehaviour) {
    match behaviour {
        ContentBehaviour::Block(block) => {
            field(out, &block.hardness.to_string());
            field(out, &block.explosion_resistance.to_string());
            flag(out, block.requires_correct_tool);
            flag(out, block.random_ticks);
        }
        ContentBehaviour::Item(item) => {
            field(out, &item.stack_size.to_string());
            flag(out, item.fire_resistant);
        }
    }
}

/// Hands the registration adapter everything the barrier admitted.
///
/// An empty answer is a launch with nothing to register, and it is not the same
/// as null: null is what the JVM receives when the native failed, and the
/// adapter registers nothing on it rather than guessing at a set. Registering
/// less than a mod declared is the safe side here — the missing block is
/// visible to the player, where a block registered from an unchecked set is not.
///
/// # Panics
///
/// Does not panic. The guard around the body turns a panic into a thrown
/// `RuntimeException` rather than an unwind into the JVM.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelAdmittedContent")]
pub fn ravel_admitted_content<'local>(
    mut env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    env.with_env(
        |env: &mut Env<'local>| -> Result<JString<'local>, HookError> {
            match document() {
                Some(document) => Ok(env.new_string(document)?),
                None => Ok(JString::default()),
            }
        },
    )
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>()
}

#[cfg(test)]
mod tests {
    use ravel_core::{BlockBehaviour, ContentBehaviour, LifecycleState, ModId, Side, SideSet};
    use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement};

    use super::document_of;
    use crate::runtime::LoaderRuntime;

    /// Reads a payload back into its fields, the way the adapter does.
    fn fields(payload: &str) -> Vec<String> {
        let units: Vec<u16> = payload.encode_utf16().collect();
        let mut read = Vec::new();
        let mut at = 0;
        while at < units.len() {
            let separator = at
                + units[at..]
                    .iter()
                    .position(|unit| *unit == u16::from(b':'))
                    .expect("every field states a length");
            let length: usize = String::from_utf16(&units[at..separator])
                .expect("a length is text")
                .parse()
                .expect("a length is a number");
            let start = separator + 1;
            read.push(String::from_utf16(&units[start..start + length]).expect("a field is text"));
            at = start + length;
        }
        read
    }

    fn id(name: &str) -> ModId {
        name.parse().expect("a valid mod id")
    }

    /// A runtime holding one mod, driven far enough to claim.
    fn runtime(mod_ids: &[&str]) -> LoaderRuntime {
        let entries = mod_ids
            .iter()
            .map(|name| LifecycleEntry {
                mod_id: id(name),
                sides: SideSet::new([Side::Client]).expect("one side is a side"),
                requirement: Requirement::Required,
                saved_data_versions: std::collections::BTreeSet::new(),
            })
            .collect();
        let mut machine =
            LifecycleMachine::new(Side::Client, entries).expect("a valid plan for one side");
        for state in [
            LifecycleState::Validated,
            LifecycleState::Resolved,
            LifecycleState::Loaded,
            LifecycleState::Constructed,
        ] {
            machine.advance_all(state).expect("a legal advance");
        }
        LoaderRuntime::new(machine, Vec::new())
    }

    #[test]
    fn nothing_is_readable_before_a_set_is_installed() {
        assert_eq!(document_of(None), None);
    }

    #[test]
    fn an_installed_set_that_claimed_nothing_answers_with_an_empty_table() {
        let runtime = runtime(&["example_mod"]);

        let payload = document_of(Some(&runtime)).expect("an installed set answers");

        assert_eq!(fields(&payload), vec!["1", "0"]);
    }

    #[test]
    fn every_admitted_block_and_item_crosses_with_its_namespace_and_owner() {
        let runtime = runtime(&["first_mod", "second_mod"]);
        runtime
            .claim(&id("second_mod"), "item:second_mod:ingot")
            .expect("accepted");
        runtime
            .claim(&id("first_mod"), "block:first_mod:copper")
            .expect("accepted");

        let payload = document_of(Some(&runtime)).expect("an installed set answers");

        assert_eq!(
            fields(&payload),
            vec![
                "1",
                "2",
                "block",
                "first_mod",
                "copper",
                "first_mod",
                "1.5",
                "6",
                "false",
                "false",
                "item",
                "second_mod",
                "ingot",
                "second_mod",
                "64",
                "false",
            ],
            "the order is the claim table's own, so what a launch registers \
             cannot depend on the order mods happened to claim in"
        );
    }

    #[test]
    fn a_described_claim_crosses_with_the_values_the_mod_chose() {
        let runtime = runtime(&["example_mod"]);
        runtime
            .describe(
                &id("example_mod"),
                "block:example_mod:obsidian",
                ContentBehaviour::Block(BlockBehaviour {
                    hardness: 50.0,
                    explosion_resistance: 1200.0,
                    requires_correct_tool: true,
                    random_ticks: false,
                }),
            )
            .expect("accepted");

        let payload = document_of(Some(&runtime)).expect("an installed set answers");

        assert_eq!(
            fields(&payload),
            vec![
                "1",
                "1",
                "block",
                "example_mod",
                "obsidian",
                "example_mod",
                "50",
                "1200",
                "true",
                "false",
            ],
            "what crosses is what the mod described, not the default it did not ask for"
        );
    }

    #[test]
    fn a_claim_naming_a_registry_this_seam_cannot_commit_to_does_not_cross() {
        let runtime = runtime(&["example_mod"]);
        runtime
            .claim(&id("example_mod"), "command:example_mod:home")
            .expect("accepted");
        runtime
            .claim(&id("example_mod"), "recipe:example_mod:alloy")
            .expect("accepted");
        runtime
            .claim(&id("example_mod"), "block:example_mod:copper")
            .expect("accepted");

        let payload = document_of(Some(&runtime)).expect("an installed set answers");

        assert_eq!(
            fields(&payload),
            vec![
                "1",
                "1",
                "block",
                "example_mod",
                "copper",
                "example_mod",
                "1.5",
                "6",
                "false",
                "false",
            ],
            "a recipe is reloaded data and a command is registered against a \
             dispatcher that does not exist at this seam"
        );
    }
}
