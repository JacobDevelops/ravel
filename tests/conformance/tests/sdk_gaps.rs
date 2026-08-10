#![forbid(unsafe_code)]

//! The six general SDK capabilities M5-W7 proposed, checked against the original
//! test mods that use them.
//!
//! `ROADMAP.md`'s M6-W2 row asks for public SDK changes plus original test mods,
//! verified by the M4-class gates. Those gates are unit, registration, save,
//! side and provenance; this file is the cross-crate half of them, and each
//! capability's own refusals are unit-tested in the crate that implements it.
//! M6-W2 shipped five of the six; the sixth, library provision, is the last
//! section here and landed in M7 for the reason `ROADMAP.md`'s open-questions
//! row gives.
//!
//! Two rules hold every check here, and both exist because a suite that agrees
//! with the implementation proves nothing:
//!
//! - **the lifecycle is the real one.** Every mod is driven through
//!   `LifecycleMachine`, which owns the plan and the registration barrier a
//!   running game uses, rather than through a stub written in this file;
//! - **the claim is reached by a different route than the implementation takes
//!   it.** An order is verified positionally against the edges that were
//!   declared rather than compared to a list this file also wrote; the bytes a
//!   world stores are read back by `serde_json` rather than by the reader that
//!   matches the writer; a derived path is spelled out here rather than asked
//!   for.
//!
//! What none of this proves is that anything happens in a game. Nothing carries
//! a sound, draws at a stage, asks at a point, or builds a provision from the
//! packages a launch composed — the same disclosure
//! `docs/guide/limits.md` already makes about packets — and each SDK module says
//! so where an author reads it. What is shipped and what is checked here is the
//! declaration, the order, and the refusals.

use std::collections::BTreeSet;

use ravel_core::{LifecycleState, ModId, Side, SideSet};
use ravel_host_api::HostMod;
use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement};
use ravel_sdk::audio::AudioError;
use ravel_sdk::interception::{Outcome, Points};
use ravel_sdk::provision::{Provided, Provision};
use ravel_sdk::render_stages::{RenderStage, StageDeclaration};
use ravel_sdk::server_data::{ServerDataDeclaration, ServerDataKind};
use ravel_sdk::world_shaping::{ShapingRecord, WorldShaping};
use ravel_sdk::{Declared, Version, VersionRange};
use ravel_test_mod_audio::AudioMod;
use ravel_test_mod_interception::InterceptionMod;
use ravel_test_mod_provision::{LIBRARY, ProvisionMod, VERSION};
use ravel_test_mod_render_stages::RenderStagesMod;
use ravel_test_mod_server_data::ServerDataMod;
use ravel_test_mod_world_shaping::WorldShapingMod;

fn id(text: &str) -> ModId {
    text.parse().expect("a legal mod id")
}

fn range(grammar: &str) -> VersionRange {
    VersionRange::parse(grammar).expect("a well formed range")
}

fn entry(mod_id: &ModId) -> LifecycleEntry {
    LifecycleEntry {
        mod_id: mod_id.clone(),
        sides: SideSet::new([Side::Client, Side::DedicatedServer]).expect("a non-empty side set"),
        requirement: Requirement::Required,
        saved_data_versions: BTreeSet::new(),
    }
}

/// Drives every mod through the loader's own plan, to the registration window
/// and out the far side.
///
/// The six mods declare no content, so nothing is claimed here and nothing can
/// be refused. What this checks is that a mod written against the new modules is
/// an ordinary mod: the plan admits it, the machine takes it through every
/// state, and the registration barrier opens and closes for it like any other.
fn drive(mods: &mut [&mut dyn HostMod]) -> LifecycleMachine {
    let entries: Vec<LifecycleEntry> = mods.iter().map(|host| entry(host.id())).collect();
    let mut machine = LifecycleMachine::new(Side::DedicatedServer, entries).expect("a legal plan");

    for state in [
        LifecycleState::Validated,
        LifecycleState::Resolved,
        LifecycleState::Loaded,
        LifecycleState::Constructed,
    ] {
        machine.advance_all(state).expect("a legal step");
        for host in mods.iter_mut() {
            host.enter(state).expect("the mod accepts every step");
        }
    }

    let order: Vec<ModId> = machine.order().cloned().collect();
    for mod_id in &order {
        let mut window = machine
            .registration_window(mod_id)
            .expect("the plan names it");
        if let Some(host) = mods.iter_mut().find(|host| host.id() == mod_id) {
            host.register(&mut window)
                .expect("a mod that claims nothing is refused nothing");
        }
    }
    machine
}

#[test]
fn every_mod_written_against_a_new_module_is_an_ordinary_mod_to_the_loader() {
    let mut server_data = ServerDataMod::new();
    let mut world_shaping = WorldShapingMod::new();
    let mut audio = AudioMod::new();
    let mut render_stages = RenderStagesMod::new();
    let mut interception = InterceptionMod::new();
    let mut provision = ProvisionMod::new();

    let mut mods: [&mut dyn HostMod; 6] = [
        &mut server_data,
        &mut world_shaping,
        &mut audio,
        &mut render_stages,
        &mut interception,
        &mut provision,
    ];
    let machine = drive(&mut mods);

    assert_eq!(machine.order().count(), 6);
    assert_eq!(
        server_data.entered(),
        [
            LifecycleState::Validated,
            LifecycleState::Resolved,
            LifecycleState::Loaded,
            LifecycleState::Constructed,
        ]
    );
    assert_eq!(interception.entered().len(), 4);
    assert_eq!(provision.entered().len(), 4);
}

// --- RVL-GAP-01, server-resolved data -------------------------------------

/// The path is spelled out here rather than read back from the declaration that
/// derived it. Asking the declaration where an entry went agrees with the
/// declaration whatever it decided, including a declaration that puts every
/// entry in one place.
#[test]
fn a_declared_server_entry_is_generated_at_the_path_this_file_spells() {
    let declared = ServerDataMod::declaration().expect("the declaration is well formed");
    let generated = declared.generate();

    let paths: Vec<String> = generated
        .files()
        .map(|file| file.path().to_string())
        .collect();
    assert_eq!(
        paths,
        [
            "data/server_data/advancement/first_plate.json",
            "data/server_data/function/grant_plate.mcfunction",
            "data/server_data/loot_table/plate_from_ingot.json",
            "data/server_data/recipe/plate_from_ingot.json",
        ]
    );
}

#[test]
fn the_same_entries_declared_in_two_orders_generate_the_same_bytes() {
    let owner = id("determinism_probe");
    let names = ["third", "first", "second"];

    let render = |order: &[usize]| -> Vec<(String, Vec<u8>)> {
        let mut declared = ServerDataDeclaration::new(owner.clone());
        for index in order {
            declared
                .crafting(names[*index], br#"{"result": "determinism_probe:plate"}"#)
                .expect("each name is claimed once");
        }
        declared
            .generate()
            .files()
            .map(|file| (file.path().to_string(), file.bytes().to_vec()))
            .collect()
    };

    let forward = render(&[0, 1, 2]);
    let backward = render(&[2, 1, 0]);
    assert_eq!(forward, backward);
    assert_eq!(forward, render(&[0, 1, 2]), "and the same twice over");
    assert_eq!(forward.len(), 3);
}

#[test]
fn a_second_claim_on_one_server_entry_is_refused_by_the_declaration() {
    let mut declared = ServerDataMod::declaration().expect("the declaration is well formed");
    declared
        .entry(ServerDataKind::Crafting, "plate_from_ingot", b"{}")
        .expect_err("the mod already declares that entry");
}

// --- RVL-GAP-02, world shaping --------------------------------------------

/// The bytes are taken apart by `serde_json` rather than by
/// [`ShapingRecord::read`]. A round trip checked only through the reader that
/// matches the writer passes whatever the format is, including one that stores
/// nothing at all.
#[test]
fn what_a_world_stores_is_readable_without_the_reader_that_wrote_it() {
    let written = WorldShapingMod::record();
    let document: serde_json::Value =
        serde_json::from_slice(&written.bytes()).expect("the record is a JSON document");

    assert_eq!(document["format"], 1);
    assert_eq!(document["shaping"][0], "world_shaping");
    assert_eq!(
        document["shaping"]
            .as_array()
            .expect("the shaping set is an array")
            .len(),
        1
    );
}

#[test]
fn a_world_is_refused_before_it_is_read_when_a_shaping_mod_is_gone() {
    let refusal = WorldShapingMod::admit_without()
        .expect_err("a world is refused when what shaped it is gone");

    assert_eq!(refusal.missing(), [WorldShapingMod::declared_id().clone()]);
    assert!(refusal.arrived().is_empty());
}

#[test]
fn bytes_that_are_not_a_shaping_record_are_refused_rather_than_guessed_at() {
    let refusal = ShapingRecord::read(b"not a shaping record")
        .expect_err("bytes from nowhere are refused rather than guessed at");
    assert!(refusal.is_unreadable());

    // And a record from this capability is not refused, so the check above is
    // about these bytes rather than about every byte.
    let mut shaping = WorldShaping::default();
    shaping.declare(id("terrain_probe"));
    ShapingRecord::read(&ShapingRecord::at_creation(&shaping).bytes())
        .expect("what this capability wrote reads back");
}

// --- RVL-GAP-03, audio ----------------------------------------------------

#[test]
fn playing_is_refused_on_every_server_side_and_taken_on_the_client() {
    for side in Side::ALL {
        let asked = AudioMod::play(side);
        if side.is_server() {
            let refusal = asked.expect_err("a server has nothing to play with");
            assert_eq!(refusal.wrong_side(), Some(side), "{side:?}");
            assert!(matches!(refusal, AudioError::WrongSide { .. }));
        } else {
            assert_eq!(
                asked.expect("a client plays what the mod declared").side(),
                side
            );
        }
    }
}

#[test]
fn an_event_for_a_sound_nothing_declares_is_refused_where_it_is_written() {
    let mut declared = AudioMod::declaration().expect("the declaration is well formed");
    let refusal = declared
        .event("a_sound_nobody_declared")
        .expect_err("an event with nothing to play is refused at declaration");
    assert_eq!(refusal.unknown_entry(), "a_sound_nobody_declared");
}

// --- RVL-GAP-04, render stages --------------------------------------------

/// The order is checked positionally against the edge that was declared, which
/// is a different route from the one that produced it. Comparing it to a list
/// this file also wrote would agree with any order the algorithm chose.
#[test]
fn a_mod_that_declared_it_follows_another_is_entered_after_it_at_every_stage() {
    let partner = id("partner_probe");
    let order = RenderStagesMod::behind(&partner).expect("the pair resolves");

    for stage in RenderStagesMod::stages() {
        let at = order.at(stage.as_str());
        let theirs = at
            .iter()
            .position(|entered| entered == &partner)
            .expect("the partner contributes here");
        let mine = at
            .iter()
            .position(|entered| entered == RenderStagesMod::declared_id())
            .expect("the test mod contributes here");
        assert!(theirs < mine, "{stage}");
    }
}

#[test]
fn contributors_that_each_follow_the_other_are_refused_rather_than_ordered() {
    let first = id("first_probe");
    let second = id("second_probe");

    let mut stages = StageDeclaration::default();
    stages.contribute(&first, RenderStage::AfterTerrain.as_str());
    stages.contribute(&second, RenderStage::AfterTerrain.as_str());
    stages.after(&first, &second);
    stages.after(&second, &first);

    let refusal = stages
        .resolve()
        .expect_err("a cycle has no order and is not given one");
    assert_eq!(refusal.cycle(), [first, second]);
}

#[test]
fn a_contribution_to_a_stage_nothing_publishes_is_refused_at_declaration() {
    let mut stages = StageDeclaration::default();
    let refusal = stages
        .contribute_checked(&id("probe"), "a_stage_nobody_publishes")
        .expect_err("a stage nothing publishes cannot be contributed to");
    assert_eq!(refusal.unknown_stage(), "a_stage_nobody_publishes");
}

// --- RVL-GAP-05, behaviour interception -----------------------------------

#[test]
fn a_refusal_at_a_point_ends_the_round_where_it_was_given() {
    let guarded = InterceptionMod::round(true).expect("a published point is asked");
    let open = InterceptionMod::round(false).expect("a published point is asked");

    assert_eq!(guarded.outcome(), Outcome::Refuse);
    assert_eq!(guarded.asked(), [InterceptionMod::declared_id().clone()]);

    assert_eq!(open.outcome(), Outcome::Proceed);
    assert_eq!(open.asked().len(), 2);
    // The mod that refused is asked in both rounds, so what differs is what it
    // answered rather than whether it was there.
    assert!(open.asked().contains(InterceptionMod::declared_id()));
}

#[test]
fn two_mods_replacing_one_outcome_are_refused_at_the_barrier_naming_both() {
    let first = id("first_probe");
    let second = id("second_probe");

    let mut points = Points::default();
    points.replace_at(&first, "block_is_broken", |_| Outcome::Proceed);
    points.replace_at(&second, "block_is_broken", |_| Outcome::Proceed);

    let refusal = points
        .admit()
        .expect_err("two replacements at one point are a conflict");
    assert_eq!(refusal.point(), "block_is_broken");
    assert_eq!(refusal.claimants(), [first, second]);
}

#[test]
fn a_point_no_adapter_publishes_is_refused_rather_than_answered_empty() {
    let mut points = Points::default();
    points.answer_at(&id("probe"), "a_point_nobody_publishes", |_| {
        Outcome::Proceed
    });

    let refusal = points
        .admit()
        .expect_err("a name nothing publishes names nothing");
    assert_eq!(refusal.point(), "a_point_nobody_publishes");
}

// --- RVL-GAP-06, library provision ----------------------------------------

/// The version is reached through the range that accepted it rather than
/// compared to the constant the test mod also publishes. Asserting the two
/// constants are equal would agree with any resolution, including one that
/// returns what it was handed without deciding anything.
#[test]
fn a_consumer_binds_to_the_one_provider_at_a_version_inside_the_range_it_asked_for() {
    let consumer = id("consumer_probe");
    let wanted = range(">=2, <3");

    let resolved = ProvisionMod::supplying(&consumer, wanted.clone())
        .expect("one provider answers one requirement");

    assert_eq!(
        resolved.provider_of(LIBRARY),
        Some(ProvisionMod::declared_id())
    );
    let bound: Version = resolved
        .version_of(LIBRARY)
        .expect("the provider resolved at a version")
        .parse()
        .expect("the resolved version is a version");
    assert!(wanted.matches(&bound), "{bound}");
    assert_eq!(resolved.libraries().collect::<Vec<&str>>(), [LIBRARY]);
}

#[test]
fn two_mods_providing_one_library_are_refused_at_resolution_naming_both() {
    let rival = id("rival_probe");

    let refusal = ProvisionMod::beside(&rival, VERSION).expect_err("one launch takes one provider");

    assert_eq!(refusal.library(), LIBRARY);
    assert_eq!(
        refusal.providers(),
        [ProvisionMod::declared_id().clone(), rival]
    );
    assert!(!refusal.has_no_provider());
}

#[test]
fn no_version_that_answers_every_requirement_is_refused_naming_the_mod_left_out() {
    let older = id("older_probe");
    let newer = id("newer_probe");

    let mut provision = Provision::default();
    ProvisionMod::provide(&mut provision);
    provision.requires(&older, LIBRARY, range("^1"));
    provision.requires(&newer, LIBRARY, range("^2"));

    let refusal = provision
        .resolve()
        .expect_err("no single version answers both, so the set is refused");

    assert_eq!(refusal.library(), LIBRARY);
    assert_eq!(refusal.unsatisfied(), [older]);
}

#[test]
fn a_required_library_nothing_provides_is_refused_before_anything_runs() {
    let consumer = id("consumer_probe");

    let mut provision = Provision::default();
    provision.provides(&consumer, Provided::new("shared_words", "1.0.0"));
    provision.requires(&consumer, "shared_numbers", range("^2"));

    let refusal = provision
        .resolve()
        .expect_err("a library nothing provides is refused");

    assert_eq!(refusal.library(), "shared_numbers");
    assert!(refusal.has_no_provider());
    assert_eq!(refusal.unsatisfied(), [consumer]);
}
