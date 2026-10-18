#![forbid(unsafe_code)]

//! Cross-crate SDK capability contracts exercised by the original test mods.
//! Uses the loader lifecycle and independent format/order assertions, not in-game execution.

use std::collections::BTreeSet;

use ravel_core::{LifecycleState, ModId, Side, SideSet};
use ravel_host_api::{Mod, Points, Round};
use ravel_loader::{LifecycleEntry, LifecycleMachine, Requirement};
use ravel_resolve::Provision;
use ravel_sdk::audio::AudioError;
use ravel_sdk::interception::{Outcome, Point};
use ravel_sdk::provision::Provided;
use ravel_sdk::render_stages::{RenderStage, StageDeclaration};
use ravel_sdk::server_data::{ServerDataDeclaration, ServerDataKind};
use ravel_sdk::world_shaping::{ShapingRecord, WorldShaping};
use ravel_sdk::{Declared, Version, VersionRange};
use ravel_test_mod_audio::AudioMod;
use ravel_test_mod_interception::{InterceptionMod, POINT};
use ravel_test_mod_provision::{LIBRARY, ProvisionMod};
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

// Exercise the real registration barrier even though these mods declare no content.
fn drive(mods: &mut [&mut dyn Mod]) -> LifecycleMachine {
    let entries: Vec<LifecycleEntry> = mods.iter().map(|host| entry(host.id())).collect();
    let mut machine =
        LifecycleMachine::new(Side::DedicatedServer, entries).expect("a legal lifecycle plan");
    ravel_loader::drive_loaded(
        &mut ravel_loader::LoadedMods::new(&mut machine, mods).expect("all active mods have hosts"),
        &ravel_bootstrap_runtime::environment(),
    )
    .expect("the production driver starts the set");
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

    let mut mods: [&mut dyn Mod; 6] = [
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
            LifecycleState::Loaded,
            LifecycleState::Constructed,
            LifecycleState::Registered,
            LifecycleState::Running,
        ]
    );
    assert_eq!(interception.entered().len(), 4);
    assert_eq!(provision.entered().len(), 4);
}

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

#[test]
fn what_a_world_stores_is_readable_without_the_reader_that_wrote_it() {
    let written = WorldShapingMod::record();
    let document: serde_json::Value =
        serde_json::from_slice(&written.to_bytes()).expect("the record is a JSON document");

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
    assert_eq!(refusal.arrived(), []);
}

#[test]
fn bytes_that_are_not_a_shaping_record_are_refused_rather_than_guessed_at() {
    let refusal = ShapingRecord::read(b"not a shaping record")
        .expect_err("bytes from nowhere are refused rather than guessed at");
    assert!(refusal.is_unreadable());

    let mut shaping = WorldShaping::default();
    shaping.declare(id("terrain_probe"));
    ShapingRecord::read(&ShapingRecord::at_creation(&shaping).to_bytes())
        .expect("what this capability wrote reads back");
}

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
    assert_eq!(refusal.unknown_entry(), Some("a_sound_nobody_declared"));
}

#[test]
fn a_mod_that_declared_it_follows_another_is_entered_after_it_at_every_stage() {
    let partner = id("partner_probe");
    let order = RenderStagesMod::behind(&partner).expect("the pair resolves");

    for stage in RenderStagesMod::stages() {
        let at = order.at(stage);
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
    stages.contribute(&first, RenderStage::AfterTerrain);
    stages.contribute(&second, RenderStage::AfterTerrain);
    stages.after(&first, &second);
    stages.after(&second, &first);

    let refusal = stages
        .resolve()
        .expect_err("a cycle has no order and is not given one");
    assert_eq!(refusal.cycle(), [first, second]);
}

#[test]
fn a_contribution_to_a_stage_nothing_publishes_is_refused_at_declaration() {
    assert!(RenderStage::published("a_stage_nobody_publishes").is_none());
}

#[test]
fn a_refusal_at_a_point_ends_the_round_where_it_was_given() {
    let guarded = round(true);
    let open = round(false);

    assert_eq!(guarded.outcome(), Outcome::Refuse);
    assert_eq!(guarded.asked(), [InterceptionMod::declared_id().clone()]);

    assert_eq!(open.outcome(), Outcome::Proceed);
    assert_eq!(open.asked().len(), 2);
    assert!(open.asked().contains(InterceptionMod::declared_id()));
}

// The follower sorts first by id, so only the declared edge puts it after the guard.
fn round(guarding: bool) -> Round {
    struct Follower {
        id: ModId,
        after: ModId,
        asked: bool,
    }

    impl Mod for Follower {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _: LifecycleState) -> Result<(), ravel_sdk::ModError> {
            Ok(())
        }

        fn interception_declaration(
            &self,
        ) -> Result<Option<ravel_sdk::interception::InterceptionDeclaration>, ravel_sdk::ModError>
        {
            let mut declared =
                ravel_sdk::interception::InterceptionDeclaration::new(self.id.clone());
            declared.answer_at(POINT);
            declared.after(&self.after);
            Ok(Some(declared))
        }

        fn intercept(
            &mut self,
            _: ravel_sdk::interception::Occasion,
        ) -> Result<Outcome, ravel_sdk::ModError> {
            self.asked = true;
            Ok(Outcome::Proceed)
        }
    }

    let mut guard = if guarding {
        InterceptionMod::guarding()
    } else {
        InterceptionMod::new()
    };
    let mut follower = Follower {
        id: id("alpha_follower"),
        after: InterceptionMod::declared_id().clone(),
        asked: false,
    };
    let mut mods: Vec<&mut dyn Mod> = vec![&mut follower, &mut guard];
    let mut machine = interception_machine(&mut mods);

    let environment = ravel_bootstrap_runtime::environment();
    let mut loaded = ravel_loader::LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut mods)
        .expect("all active mods have hosts");
    let admitted = ravel_loader::open_interceptions(&mut loaded, &environment)
        .expect("declarations are admitted before dispatch");
    let round =
        ravel_loader::intercept(&mut loaded, &admitted, POINT, &environment).expect("a legal set");
    assert_eq!(
        follower.asked,
        round.asked().contains(&id("alpha_follower")),
        "the round and the mod disagree about whether it was asked"
    );
    round
}

fn interception_machine(mods: &mut [&mut dyn Mod]) -> LifecycleMachine {
    let entries = [InterceptionMod::declared_id().clone(), id("alpha_follower")]
        .into_iter()
        .map(|mod_id| LifecycleEntry {
            mod_id,
            sides: SideSet::new([Side::IntegratedServer]).expect("a non-empty side set"),
            requirement: Requirement::Required,
            saved_data_versions: BTreeSet::new(),
        })
        .collect();
    let mut machine =
        LifecycleMachine::new(Side::IntegratedServer, entries).expect("a legal lifecycle plan");
    ravel_loader::drive_loaded(
        &mut ravel_loader::LoadedMods::new(&mut machine, mods).expect("all active mods have hosts"),
        &ravel_bootstrap_runtime::environment(),
    )
    .expect("the production driver starts the interception mods");
    machine
}

#[test]
fn two_mods_replacing_one_outcome_are_refused_at_the_barrier_naming_both() {
    let first = id("first_probe");
    let second = id("second_probe");

    let mut points = Points::default();
    points.replace_at(&first, Point::BlockIsBroken);
    points.replace_at(&second, Point::BlockIsBroken);

    let refusal = points
        .admit()
        .expect_err("two replacements at one point are a conflict");
    assert_eq!(refusal.point(), Some(Point::BlockIsBroken));
    assert_eq!(refusal.claimants(), [first, second]);
}

#[test]
fn a_consumer_binds_to_the_one_provider_at_a_version_inside_the_range_it_asked_for() {
    let consumer = id("consumer_probe");
    let wanted = range(">=2, <3");

    let mut provision = Provision::default();
    let (library, version) = ProvisionMod::provided().into_parts();
    provision.provides(ProvisionMod::declared_id(), library, version);
    provision.requires(&consumer, LIBRARY, wanted.clone());
    let resolved = provision
        .resolve()
        .expect("one provider answers one requirement");

    assert_eq!(
        resolved.provider_of(LIBRARY),
        Some(ProvisionMod::declared_id())
    );
    let bound = resolved
        .version_of(LIBRARY)
        .expect("the provider resolved at a version");
    assert!(wanted.matches(bound), "{bound}");
    assert_eq!(resolved.libraries().collect::<Vec<&str>>(), [LIBRARY]);
}

#[test]
fn two_mods_providing_one_library_are_refused_at_resolution_naming_both() {
    let rival = id("rival_probe");

    let mut provision = Provision::default();
    let (library, version) = ProvisionMod::provided().into_parts();
    provision.provides(ProvisionMod::declared_id(), library, version);
    let (library, version) = Provided::new(LIBRARY, Version::new(2, 1, 0)).into_parts();
    provision.provides(&rival, library, version);
    let refusal = provision
        .resolve()
        .expect_err("one launch takes one provider");

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
    let (library, version) = ProvisionMod::provided().into_parts();
    provision.provides(ProvisionMod::declared_id(), library, version);
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
    provision.provides(&consumer, "shared_words".to_owned(), Version::new(1, 0, 0));
    provision.requires(&consumer, "shared_numbers", range("^2"));

    let refusal = provision
        .resolve()
        .expect_err("a library nothing provides is refused");

    assert_eq!(refusal.library(), "shared_numbers");
    assert!(refusal.has_no_provider());
    assert_eq!(refusal.unsatisfied(), [consumer]);
}
