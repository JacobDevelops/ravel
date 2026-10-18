#![forbid(unsafe_code)]

//! Optional service ordering, absence, provider refusal, and required-service controls.

use std::collections::BTreeSet;

use ravel_core::{ModId, Side, SideSet};
use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement, ServiceRefused, open_services};
use ravel_sdk::{Declared, Mod};
use ravel_test_mod_inter_mod_service::{ShopMod, prices};
use ravel_test_mod_optional_service::{Bearings, CompassMod, SurveyMod, landmarks};

const SIDE: Side = Side::DedicatedServer;

fn environment() -> ravel_core::FailureEnvironment {
    ravel_core::FailureEnvironment {
        ravel: "0.1.0".parse().expect("valid version"),
        minecraft: "Synthetic26.1.5".to_owned(),
        platform: "test".to_owned(),
        architecture: "test".to_owned(),
    }
}

fn entry(mod_id: &ModId, sides: &[Side]) -> LifecycleEntry {
    LifecycleEntry {
        mod_id: mod_id.clone(),
        sides: SideSet::new(sides.iter().copied()).expect("a non-empty side set"),
        requirement: Requirement::Required,
        saved_data_versions: BTreeSet::new(),
    }
}

fn machine(entries: Vec<LifecycleEntry>) -> LifecycleMachine {
    LifecycleMachine::new(SIDE, entries).expect("a legal plan")
}

#[test]
fn an_installed_optional_provider_is_composed_before_the_mod_that_may_use_it() {
    let mut compass = CompassMod::new();
    let mut survey = SurveyMod::new();
    let mut machine = machine(vec![
        entry(CompassMod::declared_id(), &[SIDE]),
        entry(SurveyMod::declared_id(), &[SIDE]),
    ]);

    // Supply the consumer first so slice order cannot satisfy the dependency.
    let opened = open_services(
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(
            &mut machine,
            &mut [&mut compass, &mut survey],
        )
        .expect("all active mods have hosts"),
        &environment(),
    )
    .expect("the graph resolves");

    assert_eq!(
        opened.order(),
        [
            SurveyMod::declared_id().clone(),
            CompassMod::declared_id().clone()
        ],
        "an optional dependency bought no ordering"
    );
    assert_eq!(
        compass.bearings().named(),
        Some(&["spire".to_owned(), "hollow".to_owned()][..]),
        "the compass was composed before the survey it could have reached"
    );
    assert_eq!(survey.answered(), 1);
}

#[test]
fn the_same_compass_alone_loads_and_records_doing_without() {
    let mut compass = CompassMod::new();
    let mut machine = machine(vec![entry(CompassMod::declared_id(), &[SIDE])]);

    let opened = open_services(
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut compass])
            .expect("all active mods have hosts"),
        &environment(),
    )
    .expect("an absent provider refuses nothing");

    assert_eq!(opened.order(), [CompassMod::declared_id().clone()]);
    assert_eq!(
        compass.bearings(),
        &Bearings::Alone,
        "the compass never asked, so absence was never reached"
    );
}

#[test]
fn an_optional_provider_this_side_parks_is_absence_rather_than_a_refusal() {
    let mut compass = CompassMod::new();
    let mut survey = SurveyMod::new();
    let mut machine = machine(vec![
        entry(CompassMod::declared_id(), &[SIDE]),
        entry(SurveyMod::declared_id(), &[Side::Client]),
    ]);

    let opened = open_services(
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(
            &mut machine,
            &mut [&mut compass, &mut survey],
        )
        .expect("all active mods have hosts"),
        &environment(),
    )
    .expect("a parked provider refuses no optional consumer");

    assert_eq!(opened.order(), [CompassMod::declared_id().clone()]);
    assert_eq!(compass.bearings(), &Bearings::Alone);
    assert_eq!(survey.answered(), 0, "a parked mod answered a call");
}

#[test]
fn a_survey_that_was_reached_and_refused_refuses_the_set() {
    let mut compass = CompassMod::new();
    let mut survey = SurveyMod::refusing();
    let mut machine = machine(vec![
        entry(CompassMod::declared_id(), &[SIDE]),
        entry(SurveyMod::declared_id(), &[SIDE]),
    ]);

    let refusal = open_services(
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(
            &mut machine,
            &mut [&mut compass, &mut survey],
        )
        .expect("all active mods have hosts"),
        &environment(),
    )
    .expect_err("the public path reports the provider's refusal");

    assert!(matches!(
        refusal,
        ServiceRefused::Mod { ref mod_id, .. } if mod_id == SurveyMod::declared_id()
    ));
    let rendered = refusal.to_string();
    assert!(rendered.contains("no chart loaded"), "{rendered}");
    assert_eq!(compass.bearings(), &Bearings::Unwired);
    assert_eq!(
        machine.state(SurveyMod::declared_id()),
        Ok(ravel_core::LifecycleState::Failed)
    );
    let failure = machine
        .failure(SurveyMod::declared_id())
        .expect("known survey")
        .expect("recorded provider failure");
    assert_eq!(failure.mod_id.as_ref(), Some(SurveyMod::declared_id()));
    assert_eq!(failure.environment, environment());
}

#[test]
fn a_required_service_nothing_answers_still_refuses_the_whole_set() {
    let mut shop = ShopMod::new();
    let mut machine = machine(vec![entry(ShopMod::declared_id(), &[SIDE])]);

    assert_eq!(
        open_services(
            &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut shop],)
                .expect("all active mods have hosts"),
            &environment(),
        ),
        Err(ServiceRefused::NoProvider {
            consumer: ShopMod::declared_id().clone(),
            service: prices(),
        })
    );
    assert!(shop.prices().is_none());
}

#[test]
fn a_required_and_an_optional_consumer_of_the_same_absent_provider_disagree() {
    let mut compass = CompassMod::new();
    let mut shop = ShopMod::new();
    let mut machine = machine(vec![
        entry(CompassMod::declared_id(), &[SIDE]),
        entry(ShopMod::declared_id(), &[SIDE]),
    ]);

    let refusal = open_services(
        &mut ravel_loader::LoadedMods::<&mut dyn Mod>::new(
            &mut machine,
            &mut [&mut compass, &mut shop],
        )
        .expect("all active mods have hosts"),
        &environment(),
    )
    .expect_err("the shop's provider is missing");

    assert_eq!(
        refusal,
        ServiceRefused::NoProvider {
            consumer: ShopMod::declared_id().clone(),
            service: prices(),
        }
    );
    assert_eq!(
        compass.bearings(),
        &Bearings::Unwired,
        "a refused set wired an optional consumer anyway"
    );
}

#[test]
fn the_compass_calls_a_service_it_declared_optional_rather_than_one_it_did_not_declare() {
    let declaration = CompassMod::new()
        .service_declaration()
        .expect("the service declaration succeeds")
        .expect("the compass declares one service");
    assert!(declaration.calls(&landmarks()));
    assert!(declaration.may_do_without(&landmarks()));
    assert_eq!(declaration.requires().count(), 0);
}
