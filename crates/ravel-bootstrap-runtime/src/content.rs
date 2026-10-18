//! Admitted registry content encoded for the Java registration adapter.

use std::sync::PoisonError;

use jni::EnvUnowned;
use jni::objects::{JClass, JString};
use ravel_core::ContentKind;
use ravel_data::ContentBehaviour;

use crate::guard::hand_document;
use crate::runtime::LoaderRuntime;
use crate::wire::{VERSION, count, field, flag};

/// Whether a kind names a registry this crossing can commit to.
const fn is_registrable(kind: ContentKind) -> bool {
    matches!(kind, ContentKind::Block | ContentKind::Item)
}

/// Encodes admitted content, or returns nothing before runtime installation.
fn document() -> Option<String> {
    document_of(crate::runtime::installed().map(AsRef::as_ref))
}

/// The payload one runtime answers with, or nothing without one.
fn document_of(runtime: Option<&LoaderRuntime>) -> Option<String> {
    Some(runtime?.content_document())
}

impl LoaderRuntime {
    /// Encodes admitted registry claims in identifier order.
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
            field(&mut out, owner.as_str());
            describe(&mut out, behaviour);
        }
        out
    }
}

/// Writes the fixed behavior fields declared by the entry's kind.
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

/// Hands the registration adapter the admitted set, or null before installation.
///
/// # Panics
///
/// Panics become Java `RuntimeException`s rather than unwinding into the JVM.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelAdmittedContent")]
pub fn ravel_admitted_content<'local>(
    env: EnvUnowned<'local>,
    _class: JClass<'local>,
) -> JString<'local> {
    hand_document(env, |_| Ok(document()))
}

#[cfg(test)]
mod tests {
    use ravel_core::{LifecycleState, ModId, Side, SideSet};
    use ravel_data::{BlockBehaviour, ContentBehaviour};
    use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement};

    use super::document_of;
    use crate::runtime::{LoaderRuntime, advance_through};
    use crate::wire::fields;

    fn id(name: &str) -> ModId {
        name.parse().expect("a valid mod id")
    }

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
        advance_through(&mut machine, LifecycleState::Constructed);
        LoaderRuntime::new(machine, Vec::new(), crate::runtime::test_launch())
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
            .claim(
                &id("second_mod"),
                &"item:second_mod:ingot".parse().expect("valid key"),
            )
            .expect("accepted");
        runtime
            .claim(
                &id("first_mod"),
                &"block:first_mod:copper".parse().expect("valid key"),
            )
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
            .claim_describing(
                &id("example_mod"),
                &"block:example_mod:obsidian".parse().expect("valid key"),
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
            .claim(
                &id("example_mod"),
                &"command:example_mod:home".parse().expect("valid key"),
            )
            .expect("accepted");
        runtime
            .claim(
                &id("example_mod"),
                &"recipe:example_mod:alloy".parse().expect("valid key"),
            )
            .expect("accepted");
        runtime
            .claim(
                &id("example_mod"),
                &"block:example_mod:copper".parse().expect("valid key"),
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
