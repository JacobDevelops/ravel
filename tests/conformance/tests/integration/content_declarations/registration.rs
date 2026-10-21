//! Registration ownership and namespace conflicts at the real barrier.

use std::collections::BTreeSet;

use ravel_core::{ContentId, ContentKind, LifecycleState, ModId, RegistryKey, Side, SideSet};
use ravel_example_content_mod::ContentMod;
use ravel_host_api::{HostError, Mod, Registrar};
use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement};
use ravel_sdk::Declared;

use super::id;

fn entry(name: &str) -> LifecycleEntry {
    LifecycleEntry {
        mod_id: id(name),
        sides: SideSet::new([Side::Client, Side::DedicatedServer]).expect("a non-empty side set"),
        requirement: Requirement::Required,
        saved_data_versions: BTreeSet::new(),
    }
}

struct Rival {
    id: ModId,
    claims: Vec<RegistryKey>,
}

impl Mod for Rival {
    fn id(&self) -> &ModId {
        &self.id
    }

    fn enter(&mut self, _state: LifecycleState) -> Result<(), ravel_host_api::ModError> {
        Ok(())
    }

    fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), ravel_host_api::ModError> {
        for key in &self.claims {
            registrar
                .claim(key.clone())
                .map_err(ravel_host_api::ModError::from_display)?;
        }
        Ok(())
    }
}

type Outcome = (ModId, Result<(), HostError>);

fn register_all(
    entries: Vec<LifecycleEntry>,
    mods: &mut [&mut dyn Mod],
) -> (LifecycleMachine, Vec<Outcome>) {
    let mut machine = LifecycleMachine::new(Side::DedicatedServer, entries).expect("a legal plan");
    for state in [
        LifecycleState::Validated,
        LifecycleState::Resolved,
        LifecycleState::Loaded,
        LifecycleState::Constructed,
    ] {
        machine.advance_all(state).expect("a legal step");
    }

    let order: Vec<ModId> = machine.order().cloned().collect();
    let mut outcomes = Vec::new();
    for mod_id in order {
        let mut window = machine
            .registration_window(&mod_id)
            .expect("the plan names it");
        if let Some(host) = mods.iter_mut().find(|host| host.id() == &mod_id) {
            outcomes.push((
                mod_id.clone(),
                host.register(&mut window).map_err(|error| {
                    HostError::from_operation_refusal(
                        mod_id.clone(),
                        LifecycleState::Constructed,
                        error,
                    )
                }),
            ));
        }
        assert!(window.unexpected().is_none(), "the window was open");
    }
    (machine, outcomes)
}

#[test]
fn every_entry_the_example_declares_is_owned_by_it_after_the_real_barrier_closes() {
    let mut content = ContentMod::new();
    let declared = content
        .declaration()
        .expect("valid declaration")
        .entries()
        .to_vec();
    assert!(
        !declared.is_empty(),
        "an example that declares nothing would make every check below vacuous"
    );

    let (mut machine, outcomes) = register_all(vec![entry("content")], &mut [&mut content]);
    for (mod_id, outcome) in outcomes {
        outcome.unwrap_or_else(|error| panic!("`{mod_id}` was refused: {error}"));
    }

    for key in &declared {
        assert_eq!(
            machine.registry_owner(key),
            Some(ContentMod::declared_id()),
            "`{key}` was declared and claimed, so the barrier owns it for this mod"
        );
    }
    assert_eq!(
        machine.registry_claim_count(ContentMod::declared_id()),
        declared.len()
    );

    machine
        .advance_all(LifecycleState::Registered)
        .expect("a legal step");
    let late: RegistryKey = "block:content:late".parse().expect("valid key");
    assert_eq!(
        machine
            .registration_window(ContentMod::declared_id())
            .expect("tracked owner")
            .claim(late.clone()),
        Err(ravel_host_api::RegistrationRefused::Closed { key: late })
    );
}

#[test]
fn a_declared_entry_the_barrier_never_saw_has_no_owner() {
    let never = RegistryKey::new(
        ContentKind::Block,
        ContentId::new(id("content"), "unclaimed").expect("a legal content path"),
    );
    let mut content = ContentMod::new();
    let (machine, _) = register_all(vec![entry("content")], &mut [&mut content]);

    assert_eq!(machine.registry_owner(&never), None);
}

#[test]
fn a_second_mod_claiming_the_examples_block_is_refused_and_told_who_owns_it() {
    let mut content = ContentMod::new();
    let contested = content
        .declaration()
        .expect("valid declaration")
        .entries()
        .iter()
        .find(|key| key.kind() == ContentKind::Block)
        .expect("the example declares a block")
        .clone();
    let mut rival = Rival {
        id: id("rival"),
        claims: vec![contested.clone()],
    };

    let (machine, outcomes) = register_all(
        vec![entry("content"), entry("rival")],
        &mut [&mut content, &mut rival],
    );

    let refusal = outcomes
        .iter()
        .find(|(mod_id, _)| mod_id == &id("rival"))
        .and_then(|(_, outcome)| outcome.as_ref().err())
        .expect("the rival's claim was refused");
    let detail = refusal.to_string();
    assert!(detail.contains("content"), "{detail}");
    assert!(detail.contains(&contested.to_string()), "{detail}");
    assert_eq!(refusal.state(), LifecycleState::Constructed);

    assert_eq!(
        machine.registry_owner(&contested),
        Some(ContentMod::declared_id())
    );
    assert_eq!(machine.registry_claim_count(&id("rival")), 0);
}

#[test]
fn a_foreign_claim_cannot_reserve_the_examples_namespace_before_its_owner() {
    let mut content = ContentMod::new();
    let contested = content
        .declaration()
        .expect("valid declaration")
        .entries()
        .first()
        .expect("the example declares content")
        .clone();
    let mut rival = Rival {
        id: id("rival"),
        claims: vec![contested.clone()],
    };

    let (machine, outcomes) = register_all(
        vec![entry("rival"), entry("content")],
        &mut [&mut rival, &mut content],
    );

    let refusal = outcomes
        .iter()
        .find(|(mod_id, _)| mod_id == &id("rival"))
        .and_then(|(_, outcome)| outcome.as_ref().err())
        .expect("the foreign claim was refused before acquiring ownership");
    assert_eq!(refusal.mod_id(), &id("rival"));
    assert_eq!(refusal.state(), LifecycleState::Constructed);
    let detail = refusal.to_string();
    assert!(detail.contains("rival"), "{detail}");
    assert!(detail.contains(&contested.to_string()), "{detail}");

    assert_eq!(
        machine.registry_owner(&contested),
        Some(ContentMod::declared_id())
    );
    assert_eq!(machine.registry_claim_count(&id("rival")), 0);
    assert_eq!(
        machine.registry_claim_count(ContentMod::declared_id()),
        content
            .declaration()
            .expect("valid declaration")
            .entries()
            .len()
    );
}

#[test]
fn a_second_mod_claiming_a_name_the_example_did_not_declare_is_accepted() {
    let mut content = ContentMod::new();
    let mine = RegistryKey::new(
        ContentKind::Block,
        ContentId::new(id("rival"), "tin_lantern").expect("a legal content path"),
    );
    let mut rival = Rival {
        id: id("rival"),
        claims: vec![mine.clone()],
    };

    let (machine, outcomes) = register_all(
        vec![entry("content"), entry("rival")],
        &mut [&mut content, &mut rival],
    );
    for (mod_id, outcome) in outcomes {
        outcome.unwrap_or_else(|error| panic!("`{mod_id}` was refused: {error}"));
    }
    assert_eq!(machine.registry_owner(&mine), Some(&id("rival")));
}
