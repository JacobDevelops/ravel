#![forbid(unsafe_code)]

//! Whether a mod can say it uses another mod *if* that mod is installed.
//!
//! M7-W0's gate. Every property here is checked against the code a running host
//! uses — `ravel_loader::open_services` — over mods written against `ravel-sdk`
//! alone, so what passes is the loader's behaviour and not a harness agreeing with
//! itself. `ravel-test-mod-optional-service` supplies both halves: a survey that is
//! an ordinary provider, and a compass that declares the survey optional.
//!
//! # Why each property needs more than "the set loaded"
//!
//! **Loading is the easy half and the wrong half to stop at.** A set whose optional
//! provider is installed must also *order* it first, and a set that composed the
//! consumer first would load, pass every "no refusal" assertion, and be wrong only
//! in a running game. So each test reads what the compass ended up holding, and
//! every test hands the consumer to the loader first so a slice order would fail.
//!
//! **Absence has to be a branch taken.** `Bearings` distinguishes a compass that
//! asked and found nobody from a compass nothing ever asked, so a host that
//! silently skipped an optional-only consumer cannot pass as a working absent
//! branch.
//!
//! **Absence and refusal are different events.** A refusing survey is installed and
//! running. If the loader reported that as absence the compass would carry on and
//! the set would open with a broken survey nobody named, so the refusing row is
//! checked to refuse.
//!
//! **A requirement must still refuse.** The last test here is the control: the same
//! shape of set, with `inter-mod-service`'s required consumer in place of the
//! optional one, is refused. Without it "optional loads" would be consistent with
//! nothing failing closed at all.

use std::collections::BTreeSet;

use ravel_core::{ModId, Side, SideSet};
use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement, ServiceRefused, open_services};
use ravel_sdk::{Declared, Mod};
use ravel_test_mod_inter_mod_service::{ShopMod, prices};
use ravel_test_mod_optional_service::{Bearings, CompassMod, SurveyMod, landmarks};

/// The side every test here runs on.
///
/// A dedicated server, because both halves run there and only the parked-provider
/// test is about a side. That one says so.
const SIDE: Side = Side::DedicatedServer;

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
    let machine = machine(vec![
        entry(CompassMod::declared_id(), &[SIDE]),
        entry(SurveyMod::declared_id(), &[SIDE]),
    ]);

    // The consumer is supplied first on purpose: an order taken from the slice
    // would call it before anything could answer.
    let opened =
        open_services(&machine, &mut [&mut compass, &mut survey]).expect("the graph resolves");

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
    // The survey is simply not installed, which is the ordinary case: a player
    // installed one mod of a pair. A required declaration refuses this set; this
    // one opens it.
    let mut compass = CompassMod::new();
    let machine = machine(vec![entry(CompassMod::declared_id(), &[SIDE])]);

    let opened =
        open_services(&machine, &mut [&mut compass]).expect("an absent provider refuses nothing");

    assert_eq!(opened.order(), [CompassMod::declared_id().clone()]);
    assert_eq!(
        compass.bearings(),
        &Bearings::Alone,
        "the compass never asked, so absence was never reached"
    );
}

#[test]
fn an_optional_provider_this_side_parks_is_absence_rather_than_a_refusal() {
    // The survey is installed and declares the client alone, so it never runs here.
    // For a required service that refuses the set; for an optional one it is the
    // same fact the consumer branches on, and the only test here about a side.
    let mut compass = CompassMod::new();
    let mut survey = SurveyMod::new();
    let machine = machine(vec![
        entry(CompassMod::declared_id(), &[SIDE]),
        entry(SurveyMod::declared_id(), &[Side::Client]),
    ]);

    let opened = open_services(&machine, &mut [&mut compass, &mut survey])
        .expect("a parked provider refuses no optional consumer");

    assert_eq!(opened.order(), [CompassMod::declared_id().clone()]);
    assert_eq!(compass.bearings(), &Bearings::Alone);
    assert_eq!(survey.answered(), 0, "a parked mod answered a call");
}

#[test]
fn a_survey_that_was_reached_and_refused_refuses_the_set() {
    // The distinction the whole design turns on. This survey is installed and
    // running, so its "no" is an answer about the call. Reporting it as absence
    // would open a world with a broken survey nothing named.
    let mut compass = CompassMod::new();
    let mut survey = SurveyMod::refusing();
    let machine = machine(vec![
        entry(CompassMod::declared_id(), &[SIDE]),
        entry(SurveyMod::declared_id(), &[SIDE]),
    ]);

    let refusal = open_services(&machine, &mut [&mut compass, &mut survey])
        .expect_err("the survey said no and the compass did not declare it could ignore that");

    assert!(matches!(
        refusal,
        ServiceRefused::Consumer { ref mod_id, .. } if mod_id == CompassMod::declared_id()
    ));
    let rendered = refusal.to_string();
    assert!(rendered.contains("no chart loaded"), "{rendered}");
    assert_ne!(
        compass.bearings(),
        &Bearings::Alone,
        "a refusal from a running provider was recorded as absence"
    );
}

#[test]
fn a_required_service_nothing_answers_still_refuses_the_whole_set() {
    // The control. `ShopMod` requires what `CompassMod` may use, and the same
    // absence that opens the set above closes this one — so "optional loads" is a
    // property of the declaration and not of the loader having stopped refusing.
    let mut shop = ShopMod::new();
    let machine = machine(vec![entry(ShopMod::declared_id(), &[SIDE])]);

    assert_eq!(
        open_services(&machine, &mut [&mut shop]),
        Err(ServiceRefused::NoProvider {
            consumer: ShopMod::declared_id().clone(),
            service: prices(),
        })
    );
    assert!(shop.prices().is_none());
}

#[test]
fn a_required_and_an_optional_consumer_of_the_same_absent_provider_disagree() {
    // Both mods want a service nothing answers, in one set. The required one
    // decides: the set is refused, and the optional one is left holding nothing
    // because nothing was wired at all.
    let mut compass = CompassMod::new();
    let mut shop = ShopMod::new();
    let machine = machine(vec![
        entry(CompassMod::declared_id(), &[SIDE]),
        entry(ShopMod::declared_id(), &[SIDE]),
    ]);

    let refusal = open_services(&machine, &mut [&mut compass, &mut shop])
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
    // `Undeclared` must not fire for an optional name. The proof is negative — the
    // set above resolved — so this states what it rests on: the loader's directory
    // reads both calling sides of the declaration, and the compass names the survey
    // on the optional one.
    let declaration = CompassMod::new()
        .service_declaration()
        .expect("the compass declares one service");
    assert!(declaration.calls(&landmarks()));
    assert!(declaration.may_do_without(&landmarks()));
    assert_eq!(declaration.requires().count(), 0);
}
