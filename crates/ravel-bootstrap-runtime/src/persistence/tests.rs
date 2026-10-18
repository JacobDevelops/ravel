#[test]
fn a_world_path_that_names_no_absolute_directory_is_refused() {
    for named in ["null", "", "world", "./world", "../world"] {
        let refusal =
            super::admissible_world(std::path::Path::new(named), std::path::Path::new("/game"))
                .expect_err("a relative path is not a world directory");
        assert!(
            refusal.contains("not an absolute path"),
            "{named} was refused for the wrong reason: {refusal}"
        );
    }
}

fn scratch(name: &str) -> std::path::PathBuf {
    crate::tests::fresh_directory("world-guard", name)
}

#[test]
fn a_world_path_that_is_absent_or_is_a_file_is_refused() {
    let directory = scratch("absent");
    let absent = directory.join("no-such-world");
    let refusal = super::admissible_world(&absent, &directory)
        .expect_err("a path that is not there is not a world directory");
    assert!(refusal.contains("not an existing directory"), "{refusal}");

    let file = directory.join("world");
    std::fs::write(&file, b"not a directory").expect("the file should be written");
    let refusal =
        super::admissible_world(&file, &directory).expect_err("a file is not a world directory");
    assert!(refusal.contains("not an existing directory"), "{refusal}");
}

#[test]
fn an_existing_absolute_world_directory_is_admitted() {
    let world = scratch("admitted").join("Driven");
    std::fs::create_dir_all(&world).expect("the world should be created");

    super::admissible_world(&world, world.parent().expect("world parent"))
        .expect("an ordinary world directory is writable without an options.txt marker");
}

#[test]
fn the_launch_directory_and_its_ancestors_are_not_world_directories() {
    let root = scratch("contains-game");
    let game = root.join("game");
    std::fs::create_dir_all(&game).expect("launch directory");
    for world in [&root, &game] {
        let refused = super::admissible_world(world, &game).expect_err("contains launch directory");
        assert!(refused.contains("holds the game directory"), "{refused}");
    }
}

#[test]
fn an_unresolvable_launch_directory_never_disables_containment() {
    let root = scratch("missing-game");
    let refused = super::admissible_world(&root, &root.join("missing"))
        .expect_err("missing launch directory fails closed");
    assert!(
        refused.contains("launch directory cannot be resolved"),
        "{refused}"
    );
}

#[cfg(unix)]
#[test]
fn a_world_symlink_cannot_hide_the_launch_directory() {
    let root = scratch("symlink-game");
    let game = root.join("game");
    let world = root.join("world");
    std::fs::create_dir_all(&game).expect("launch directory");
    std::os::unix::fs::symlink(&root, &world).expect("world symlink");
    let refused = super::admissible_world(&world, &game).expect_err("hidden launch directory");
    assert!(refused.contains("holds the game directory"), "{refused}");
}

#[cfg(unix)]
#[test]
fn a_symlinked_pack_directory_cannot_redirect_the_write() {
    let root = scratch("symlink-datapacks");
    let game = root.join("game");
    let world = root.join("world");
    let outside = root.join("outside");
    for directory in [&game, &world, &outside] {
        std::fs::create_dir_all(directory).expect("scratch directory");
    }
    std::os::unix::fs::symlink(&outside, world.join(crate::packs::WORLD_DATA_PACKS))
        .expect("datapacks symlink");
    let refused = super::admissible_world(&world, &game).expect_err("escaping pack target");
    assert!(refused.contains("is a symbolic link"), "{refused}");
    assert!(
        refused.contains(crate::packs::WORLD_DATA_PACKS),
        "{refused}"
    );
}

#[cfg(unix)]
#[test]
fn every_pack_root_and_pending_tree_rejects_symlink_redirection() {
    let root = scratch("symlink-pack-roots");
    let game = root.join("game");
    let world = root.join("world");
    let outside = root.join("outside");
    for directory in [&game, &world, &outside] {
        std::fs::create_dir_all(directory).expect("scratch directory");
    }
    for pack in crate::packs::world_pack_roots(&world) {
        for target in ravel_loader::replaced_paths(&pack) {
            std::fs::create_dir_all(target.parent().expect("pack parent")).expect("pack holder");
            std::os::unix::fs::symlink(&outside, &target).expect("redirected target");
            let refusal = super::admissible_world(&world, &game).expect_err("redirected pack");
            assert!(refusal.contains("is a symbolic link"), "{refusal}");
            std::fs::remove_file(target).expect("remove symlink");
        }
    }
}

#[cfg(unix)]
#[test]
fn world_pack_refusals_leave_files_untouched_and_publish_notices() {
    let root = scratch("pack-refusal-notices");
    let game = root.join("game");
    let outside = root.join("outside");
    let redirected_world = game.join("redirected-world");
    let blocked_world = game.join("blocked-world");
    for directory in [&game, &outside, &redirected_world, &blocked_world] {
        std::fs::create_dir_all(directory).expect("scratch directory");
    }
    std::os::unix::fs::symlink(
        &outside,
        redirected_world.join(crate::packs::WORLD_DATA_PACKS),
    )
    .expect("redirected datapacks");
    let blocked_pack = blocked_world.join(crate::packs::WORLD_DATA_PACKS);
    std::fs::create_dir_all(&blocked_pack).expect("datapacks directory");
    std::fs::write(
        blocked_pack.join(crate::packs::GAME_PACK_DIRECTORY),
        b"do not replace",
    )
    .expect("blocking file");
    let (mut runtime, _) = running();
    runtime.launch.game_directory = game;
    for world in [&redirected_world, &blocked_world] {
        runtime.write_world_pack(world);
    }
    assert_eq!(
        std::fs::read_dir(&outside)
            .expect("outside directory")
            .count(),
        0
    );
    assert_eq!(
        std::fs::read(blocked_pack.join(crate::packs::GAME_PACK_DIRECTORY)).expect("blocking file"),
        b"do not replace",
    );
    let notices = notices(&runtime);
    assert!(
        notices.iter().any(|notice| {
            notice.starts_with("no data pack was written into")
                && notice.contains("redirected-world")
        }),
        "{notices:?}"
    );
    assert!(
        notices.iter().any(|notice| {
            notice.contains("could not be given the mods' data pack")
                && notice.contains("blocked-world")
        }),
        "{notices:?}"
    );
}
use std::collections::BTreeSet;
use std::num::NonZero;
use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::sync::{Arc, Mutex};

use ravel_core::{LifecycleState, ModId, Side, SideSet};
use ravel_host_api::{InterceptionDeclaration, Mod, ModError, Occasion, Outcome, Point};
use ravel_loader::{LifecycleEntry, LifecycleMachine, LoadedMods, Requirement};
use ravel_save::{
    MAX_AGGREGATE_BYTES, SavedDataAggregate, SavedDataDeclaration, SavedDataEnvelope,
    SavedDataVersion,
};
use serde_json::json;

use super::{OpenOutcome, SaveOutcome, StagedPack, WorldIdentity};
use crate::runtime::{LoaderRuntime, advance_through};

const OWNER: &str = "saved_data";
const OTHER_OWNER: &str = "other_saved_data";
const FIRST: &str = "worlds/first";
const SECOND: &str = "worlds/second";
struct FreshWorldPackSupplier(ModId);

impl Mod for FreshWorldPackSupplier {
    fn id(&self) -> &ModId {
        &self.0
    }

    fn enter(&mut self, _: LifecycleState) -> Result<(), ModError> {
        Ok(())
    }

    fn content_declaration(
        &self,
    ) -> Result<Option<std::borrow::Cow<'_, ravel_data::ContentDeclaration>>, ModError> {
        let mut declaration = ravel_data::ContentDeclaration::new(self.0.clone());
        declaration
            .resource(
                ravel_core::RelativePath::try_from(format!("data/{OWNER}/tags/items/fresh.json"))
                    .expect("tag path"),
                br#"{"values":[]}"#.to_vec(),
            )
            .expect("unique tag resource");
        Ok(Some(std::borrow::Cow::Owned(declaration)))
    }
}

fn notices(runtime: &LoaderRuntime) -> Vec<String> {
    runtime
        .inner
        .lock()
        .expect("test lock")
        .report
        .notices()
        .iter()
        .map(|notice| notice.text().to_owned())
        .collect()
}

fn id(name: &str) -> ModId {
    name.parse().expect("a legal mod id")
}

fn attribution() -> ravel_core::FailureAttribution {
    ravel_core::FailureAttribution {
        mod_id: Some(id(OWNER)),
        version: None,
        dependency_chain: Vec::new(),
        state: LifecycleState::Running,
        kind: None,
        surface: None,
        environment: crate::runtime::pre_world_report().environment().clone(),
        message: ravel_core::DiagnosticMessage::raw("this mod stopped working"),
    }
}

fn version(raw: u32) -> SavedDataVersion {
    NonZero::new(raw).expect("a non-zero version")
}

#[derive(Debug, Default)]
struct Held {
    visits: AtomicU32,
    callbacks: AtomicU32,
    refuses_production: AtomicBool,
    panics_production: AtomicBool,
    refuses_open: AtomicBool,
    refuses_interception: AtomicBool,
    refuses_interception_declaration: AtomicBool,
    opened_with: Mutex<Option<SavedDataEnvelope>>,
    declaration_owner: Mutex<Option<ModId>>,
    envelope_owner: Mutex<Option<ModId>>,
    refuses_declaration: AtomicBool,
}

impl Held {
    fn opened_with(&self) -> SavedDataEnvelope {
        self.opened_with
            .lock()
            .expect("no test poisons this lock")
            .clone()
            .expect("this mod was never opened")
    }
}

struct Owning {
    id: ModId,
    held: Arc<Held>,
}

impl Mod for Owning {
    fn id(&self) -> &ModId {
        &self.id
    }

    fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
        Ok(())
    }

    fn interception_declaration(&self) -> Result<Option<InterceptionDeclaration>, ModError> {
        self.held.callbacks.fetch_add(1, Ordering::SeqCst);
        if self
            .held
            .refuses_interception_declaration
            .load(Ordering::SeqCst)
        {
            return Err(ModError::Unsupported);
        }
        let mut declaration = InterceptionDeclaration::new(self.id.clone());
        declaration.answer_at(Point::WorldIsSaved);
        Ok(Some(declaration))
    }

    fn intercept(&mut self, _occasion: Occasion) -> Result<Outcome, ModError> {
        self.held.callbacks.fetch_add(1, Ordering::SeqCst);
        if self.held.refuses_interception.load(Ordering::SeqCst) {
            return Err(ModError::Unsupported);
        }
        Ok(Outcome::Proceed)
    }

    fn saved_data_declaration(&self) -> Result<Option<SavedDataDeclaration>, ModError> {
        self.held.callbacks.fetch_add(1, Ordering::SeqCst);
        if self.held.refuses_declaration.load(Ordering::SeqCst) {
            return Err(ModError::Unsupported);
        }
        let owner = self
            .held
            .declaration_owner
            .lock()
            .expect("test lock")
            .clone()
            .unwrap_or_else(|| self.id.clone());
        Ok(Some(
            SavedDataDeclaration::new(owner, [version(1), version(2)]).expect("two versions"),
        ))
    }

    fn saved_data_migrations(&self) -> Result<Option<ravel_save::MigrationChain>, ModError> {
        self.held.callbacks.fetch_add(1, Ordering::SeqCst);
        Ok(Some(
            ravel_save::MigrationChain::new(self.id.clone()).reaching(version(2), |payload| {
                let visits = payload["visits"]
                    .as_u64()
                    .ok_or_else(|| ravel_save::MigrationFailed::new("version 1 lacks visits"))?;
                Ok(json!({ "visits": visits + 1 }))
            }),
        ))
    }

    fn open_saved_data(&mut self, envelope: Option<&SavedDataEnvelope>) -> Result<(), ModError> {
        self.held.callbacks.fetch_add(1, Ordering::SeqCst);
        if self.held.refuses_open.load(Ordering::SeqCst) {
            return Err(ModError::refused(
                "this mod cannot read what the world stored",
            ));
        }
        let visits = envelope.map_or(0, |stored| {
            u32::try_from(stored.payload()["visits"].as_u64().unwrap_or(0)).unwrap_or(u32::MAX)
        });
        self.held.visits.store(visits, Ordering::SeqCst);
        *self
            .held
            .opened_with
            .lock()
            .expect("no test poisons this lock") = envelope.cloned();
        Ok(())
    }

    fn save_saved_data(&self) -> Result<Option<SavedDataEnvelope>, ModError> {
        self.held.callbacks.fetch_add(1, Ordering::SeqCst);
        assert!(
            !self.held.panics_production.load(Ordering::SeqCst),
            "{}",
            concat!(
                "the owner panicked with private data: token=",
                "ghp_",
                "private"
            )
        );
        if self.held.refuses_production.load(Ordering::SeqCst) {
            return Err(ModError::refused("this mod cannot produce an envelope"));
        }
        Ok(Some(SavedDataEnvelope::new(
            self.held
                .envelope_owner
                .lock()
                .expect("test lock")
                .clone()
                .unwrap_or_else(|| self.id.clone()),
            version(2),
            Vec::new(),
            None,
            json!({ "visits": self.held.visits.load(Ordering::SeqCst) }),
        )))
    }
}

#[derive(Debug)]
struct Unresolvable;

impl std::fmt::Display for Unresolvable {
    fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        formatter.write_str("`example_mod` requires `example_library`, which is not installed")
    }
}

impl std::error::Error for Unresolvable {}

impl ravel_core::StructuredFailure for Unresolvable {
    fn failure_kind(&self) -> Option<ravel_core::FailureKind> {
        Some(ravel_core::FailureKind::MissingRequiredDependency)
    }

    fn lifecycle_state(&self) -> LifecycleState {
        LifecycleState::Validated
    }

    fn quotes_untrusted_text(&self) -> bool {
        false
    }
}

fn running() -> (LoaderRuntime, Arc<Held>) {
    running_with(crate::runtime::pre_world_report())
}

fn running_with(report: ravel_observability::PreWorldReport) -> (LoaderRuntime, Arc<Held>) {
    let (runtime, mut held) = owners_running(&[OWNER], report);
    (runtime, held.remove(0))
}

fn running_pair() -> (LoaderRuntime, Arc<Held>, Arc<Held>) {
    let (runtime, mut held) =
        owners_running(&[OWNER, OTHER_OWNER], crate::runtime::pre_world_report());
    let second = held.remove(1);
    (runtime, held.remove(0), second)
}

fn owners_running(
    owners: &[&str],
    report: ravel_observability::PreWorldReport,
) -> (LoaderRuntime, Vec<Arc<Held>>) {
    owners_running_with_requirement(owners, report, Requirement::Required)
}

fn owners_running_with_requirement(
    owners: &[&str],
    report: ravel_observability::PreWorldReport,
    requirement: Requirement,
) -> (LoaderRuntime, Vec<Arc<Held>>) {
    let (machine, hosts, held) = owner_parts(owners, requirement);
    (
        LoaderRuntime::with_observers(
            machine,
            hosts,
            ravel_observability::Profiler::off(),
            report,
            crate::runtime::test_launch(),
            std::collections::BTreeMap::new(),
        ),
        held,
    )
}

fn owner_parts(
    owners: &[&str],
    requirement: Requirement,
) -> (LifecycleMachine, Vec<Box<dyn Mod>>, Vec<Arc<Held>>) {
    let held: Vec<Arc<Held>> = owners.iter().map(|_| Arc::new(Held::default())).collect();
    let hosts: Vec<Box<dyn Mod>> = owners
        .iter()
        .zip(&held)
        .map(|(owner, held)| -> Box<dyn Mod> {
            Box::new(Owning {
                id: id(owner),
                held: Arc::clone(held),
            })
        })
        .collect();
    let entries: Vec<LifecycleEntry> = owners
        .iter()
        .map(|owner| LifecycleEntry {
            mod_id: id(owner),
            sides: SideSet::new([Side::Client, Side::IntegratedServer])
                .expect("a non-empty side set"),
            requirement,
            saved_data_versions: BTreeSet::from([version(1), version(2)]),
        })
        .collect();
    let mut machine =
        LifecycleMachine::new(Side::Client, entries).expect("independent mods are a legal plan");
    advance_through(&mut machine, LifecycleState::Running);
    (machine, hosts, held)
}

const GUARD: &str = "zephyr_guard";

struct Guarding {
    id: ModId,
    refusing: Arc<AtomicBool>,
    asked: Arc<AtomicU32>,
}

impl Mod for Guarding {
    fn id(&self) -> &ModId {
        &self.id
    }

    fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
        Ok(())
    }

    fn interception_declaration(&self) -> Result<Option<InterceptionDeclaration>, ModError> {
        let mut declared = InterceptionDeclaration::new(self.id.clone());
        declared.answer_at(Point::WorldIsSaved);
        Ok(Some(declared))
    }

    fn intercept(&mut self, occasion: Occasion) -> Result<Outcome, ModError> {
        assert_eq!(occasion.point(), Point::WorldIsSaved);
        assert_eq!(occasion.side(), Side::IntegratedServer);
        self.asked.fetch_add(1, Ordering::SeqCst);
        Ok(if self.refusing.load(Ordering::SeqCst) {
            Outcome::Refuse
        } else {
            Outcome::Proceed
        })
    }
}

fn guarded_running() -> (LoaderRuntime, Arc<Held>, Arc<AtomicBool>, Arc<AtomicU32>) {
    let held = Arc::new(Held::default());
    let refusing = Arc::new(AtomicBool::new(false));
    let asked = Arc::new(AtomicU32::new(0));
    let hosts: Vec<Box<dyn Mod>> = vec![
        Box::new(Owning {
            id: id(OWNER),
            held: Arc::clone(&held),
        }),
        Box::new(Guarding {
            id: id(GUARD),
            refusing: Arc::clone(&refusing),
            asked: Arc::clone(&asked),
        }),
    ];
    let entries: Vec<LifecycleEntry> = [OWNER, GUARD]
        .iter()
        .map(|owner| LifecycleEntry {
            mod_id: id(owner),
            sides: SideSet::new([Side::Client, Side::IntegratedServer])
                .expect("a non-empty side set"),
            requirement: Requirement::Required,
            saved_data_versions: if *owner == OWNER {
                BTreeSet::from([version(1), version(2)])
            } else {
                BTreeSet::new()
            },
        })
        .collect();
    let mut machine =
        LifecycleMachine::new(Side::Client, entries).expect("independent mods are a legal plan");
    advance_through(&mut machine, LifecycleState::Running);
    (
        LoaderRuntime::with_observers(
            machine,
            hosts,
            ravel_observability::Profiler::off(),
            crate::runtime::pre_world_report(),
            crate::runtime::test_launch(),
            std::collections::BTreeMap::new(),
        ),
        held,
        refusing,
        asked,
    )
}

#[test]
fn a_mod_refusing_at_world_is_saved_withholds_ravels_write_and_a_later_save_offers_it() {
    let (runtime, held, refusing, asked) = guarded_running();
    runtime
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("a fresh world is staged");
    held.visits.store(7, Ordering::SeqCst);

    refusing.store(true, Ordering::SeqCst);
    let withheld = runtime.save_world(&WorldIdentity::of(FIRST));
    assert_eq!(withheld, SaveOutcome::Withheld);
    assert_eq!(asked.load(Ordering::SeqCst), 1, "the round was not asked");
    assert!(
        notices(&runtime)
            .iter()
            .any(|notice| notice.contains(GUARD) && notice.contains("world_is_saved")),
        "a withheld write is invisible to the player: {:?}",
        notices(&runtime)
    );

    refusing.store(false, Ordering::SeqCst);
    let bytes = replacement(runtime.save_world(&WorldIdentity::of(FIRST)));
    assert_eq!(asked.load(Ordering::SeqCst), 2);
    assert_eq!(visits(&bytes), 7);
}

#[test]
fn a_mod_that_fails_at_world_is_saved_fails_the_save_rather_than_writing() {
    struct Failing {
        id: ModId,
    }

    impl Mod for Failing {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
            Ok(())
        }

        fn interception_declaration(&self) -> Result<Option<InterceptionDeclaration>, ModError> {
            let mut declared = InterceptionDeclaration::new(self.id.clone());
            declared.answer_at(Point::WorldIsSaved);
            Ok(Some(declared))
        }

        fn intercept(&mut self, _occasion: Occasion) -> Result<Outcome, ModError> {
            Err(ModError::refused("the guard broke"))
        }
    }

    let hosts: Vec<Box<dyn Mod>> = vec![Box::new(Failing { id: id(GUARD) })];
    let entries = vec![LifecycleEntry {
        mod_id: id(GUARD),
        sides: SideSet::new([Side::Client, Side::IntegratedServer])
            .expect("client and integrated server"),
        requirement: Requirement::Required,
        saved_data_versions: BTreeSet::new(),
    }];
    let mut machine = LifecycleMachine::new(Side::Client, entries).expect("a legal plan");
    advance_through(&mut machine, LifecycleState::Running);
    let runtime = LoaderRuntime::with_observers(
        machine,
        hosts,
        ravel_observability::Profiler::off(),
        crate::runtime::pre_world_report(),
        crate::runtime::test_launch(),
        std::collections::BTreeMap::new(),
    );
    runtime
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("a fresh world is staged");

    let outcome = runtime.save_world(&WorldIdentity::of(FIRST));
    let SaveOutcome::Failed(detail) = &outcome else {
        panic!("a mod that failed while answering should fail the save: {outcome:?}");
    };
    assert!(detail.contains(GUARD), "{detail}");
    assert!(detail.contains("the guard broke"), "{detail}");
}

#[test]
fn an_optional_owner_failed_before_first_open_is_admitted_without_callbacks() {
    let (source, _) = running();
    source
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("fresh world");
    let durable = replacement(source.save_world(&WorldIdentity::of(FIRST)));
    let (runtime, held) = failed_optional();
    held.refuses_declaration.store(true, Ordering::SeqCst);
    held.refuses_open.store(true, Ordering::SeqCst);
    runtime
        .stage_world(WorldIdentity::of(FIRST), Some(&durable), false)
        .expect("terminal optional owners retain opaque envelopes without callbacks");
    assert_eq!(
        replacement(runtime.save_world(&WorldIdentity::of(FIRST))),
        durable
    );
}

#[test]
fn an_already_failed_owner_keeps_its_original_version_without_migration() {
    let stored = stored_at(version(99), 5);
    let (runtime, held) = failed_optional();
    held.refuses_declaration.store(true, Ordering::SeqCst);
    held.refuses_open.store(true, Ordering::SeqCst);
    runtime
        .stage_world(WorldIdentity::of(FIRST), Some(&stored), false)
        .expect("failed owner's envelope remains opaque even at an undeclared version");
    let written = replacement(runtime.save_world(&WorldIdentity::of(FIRST)));
    assert_eq!(
        written, stored,
        "failed owner's payload and data version are preserved"
    );
}

#[test]
fn failed_checkpoint_serialization_retains_dirty_state_for_every_retry() {
    let mut aggregate = SavedDataAggregate::fresh();
    aggregate.replace(SavedDataEnvelope::new(
        id(OWNER),
        version(2),
        Vec::new(),
        None,
        json!({ "oversized": "x".repeat(MAX_AGGREGATE_BYTES) }),
    ));
    let mut checkpoint = super::Checkpoint {
        identity: WorldIdentity::of(FIRST),
        aggregate,
        dirty: true,
    };
    for _ in 0..2 {
        assert!(matches!(
            LoaderRuntime::save_checkpoint(&mut checkpoint),
            SaveOutcome::Failed(_)
        ));
        assert!(
            checkpoint.dirty,
            "a failed serialization must be offered again"
        );
    }
    checkpoint.aggregate = SavedDataAggregate::fresh();
    assert!(matches!(
        LoaderRuntime::save_checkpoint(&mut checkpoint),
        SaveOutcome::Replacement(_)
    ));
    assert!(
        !checkpoint.dirty,
        "only successful serialization clears dirty"
    );
    assert_eq!(
        LoaderRuntime::save_checkpoint(&mut checkpoint),
        SaveOutcome::Unchanged
    );
}

#[test]
fn a_failed_migration_does_not_deliver_payload_or_arm_world_mutation() {
    let (runtime, held) = running();
    let mut aggregate = SavedDataAggregate::fresh();
    aggregate.replace(SavedDataEnvelope::new(
        id(OWNER),
        version(1),
        Vec::new(),
        None,
        json!({"not_visits": true}),
    ));
    let bytes = aggregate.to_bytes().expect("bounded fixture");
    assert!(
        runtime
            .stage_world(WorldIdentity::of(FIRST), Some(&bytes), false)
            .is_err()
    );
    assert!(held.opened_with.lock().expect("test lock").is_none());
    let inner = runtime.inner.lock().expect("test lock");
    assert!(inner.persistence.staged.is_none());
    assert_eq!(
        inner.loaded.machine().state(&id(OWNER)),
        Ok(LifecycleState::Failed)
    );
    assert!(
        inner.loaded.machine().world_access(&inner.report).is_err(),
        "a failed required owner blocks world admission"
    );
    assert!(
        inner.loaded.machine().save_access().is_ok(),
        "no mutation was admitted"
    );
    drop(inner);
}

#[test]
fn an_optional_failed_migration_is_isolated_without_delivering_its_payload() {
    let (runtime, held) = optional_running();
    let mut aggregate = SavedDataAggregate::fresh();
    aggregate.replace(SavedDataEnvelope::new(
        id(OWNER),
        version(1),
        Vec::new(),
        None,
        json!({"not_visits": true}),
    ));
    let bytes = aggregate.to_bytes().expect("bounded fixture");
    runtime
        .stage_world(WorldIdentity::of(FIRST), Some(&bytes), false)
        .expect("an optional migration failure does not withhold the world");
    assert!(held.opened_with.lock().expect("test lock").is_none());
    let inner = runtime.inner.lock().expect("test lock");
    let staged = inner.persistence.staged.as_ref().expect("world is staged");
    assert_eq!(staged.identity, WorldIdentity::of(FIRST));
    assert_eq!(staged.aggregate, aggregate);
    assert_eq!(
        inner.loaded.machine().state(&id(OWNER)),
        Ok(LifecycleState::Failed)
    );
    assert!(
        inner.loaded.machine().world_access(&inner.report).is_ok(),
        "the failed optional owner does not block world admission"
    );
    drop(inner);
    assert_eq!(
        replacement(runtime.save_world(&WorldIdentity::of(FIRST))),
        bytes,
        "the failed optional owner's opaque envelope is retained"
    );
}

#[test]
fn a_foreign_saved_data_declaration_fails_its_producer_without_opening_another_owner() {
    for foreign in ["absent_owner", OTHER_OWNER] {
        let (runtime, held, other) = running_pair();
        *held.declaration_owner.lock().expect("test lock") = Some(id(foreign));
        assert!(
            runtime
                .stage_world(WorldIdentity::of(FIRST), None, true)
                .is_err()
        );
        let inner = runtime
            .inner
            .lock()
            .expect("declaration rejection does not panic");
        assert_eq!(
            inner.loaded.machine().state(&id(OWNER)),
            Ok(LifecycleState::Failed)
        );
        assert_eq!(
            inner.loaded.machine().state(&id(OTHER_OWNER)),
            Ok(LifecycleState::Running)
        );
        drop(inner);
        assert!(other.opened_with.lock().expect("test lock").is_none());
    }
}

#[test]
fn foreign_envelopes_cannot_overwrite_live_or_failed_owners() {
    for (victim_failed, next_world) in [(false, false), (true, false), (false, true), (true, true)]
    {
        let (mut machine, hosts, held) = owner_parts(&[OWNER, OTHER_OWNER], Requirement::Optional);
        let [_, writer] = held.as_slice() else {
            panic!("the fixture has two owners");
        };
        if victim_failed {
            machine
                .fail(&id(OWNER), attribution())
                .expect("optional owner failed before mutation");
        }
        let runtime = LoaderRuntime::new(machine, hosts, crate::runtime::test_launch());
        let stored = stored_owning(&[(OWNER, version(2), 7), (OTHER_OWNER, version(2), 11)]);
        runtime
            .stage_world(WorldIdentity::of(FIRST), Some(&stored), false)
            .expect("existing world");
        let SaveOutcome::Replacement(baseline) = runtime.save_world(&WorldIdentity::of(FIRST))
        else {
            panic!("the honest snapshot can be saved");
        };
        *writer.envelope_owner.lock().expect("test lock") = Some(id(OWNER));
        writer.visits.store(99, Ordering::SeqCst);

        if next_world {
            assert_eq!(
                runtime.stage_world(WorldIdentity::of(SECOND), None, true),
                Err(super::PersistenceError::Admission(
                    ravel_loader::SaveRefused::ForeignEnvelope {
                        mod_id: id(OTHER_OWNER),
                        owner: id(OWNER),
                    },
                )),
            );
        } else {
            let SaveOutcome::Failed(detail) = runtime.save_world(&WorldIdentity::of(FIRST)) else {
                panic!("a foreign envelope must never become a durable replacement");
            };
            assert!(detail.contains(OTHER_OWNER), "{detail}");
            assert!(detail.contains(&format!("`{OWNER}`")), "{detail}");
        }
        let inner = runtime.inner.lock().expect("test lock");
        let live = inner.persistence.live.as_ref().expect("live checkpoint");
        assert_eq!(live.aggregate.to_bytes().expect("bounded"), baseline);
        assert!(!live.dirty);
        assert_eq!(
            inner.loaded.machine().state(&id(OTHER_OWNER)),
            Ok(LifecycleState::Failed)
        );
        assert_eq!(
            inner
                .loaded
                .machine()
                .failure(&id(OTHER_OWNER))
                .expect("owner")
                .expect("failure")
                .mod_id,
            Some(id(OTHER_OWNER)),
        );
        drop(inner);
    }
}

#[test]
fn interception_callback_failure_is_attributed_to_the_asked_owner() {
    let (runtime, held) = running();
    runtime
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("fresh world");
    held.refuses_interception.store(true, Ordering::SeqCst);
    assert!(matches!(
        runtime.save_world(&WorldIdentity::of(FIRST)),
        SaveOutcome::Failed(_)
    ));
    let inner = runtime.inner.lock().expect("no test poisons the lock");
    assert_eq!(
        inner.loaded.machine().state(&id(OWNER)),
        Ok(LifecycleState::Failed)
    );
    assert_eq!(
        inner
            .loaded
            .machine()
            .failure(&id(OWNER))
            .expect("owner")
            .expect("failure")
            .mod_id,
        Some(id(OWNER))
    );
    drop(inner);
}

fn optional_running() -> (LoaderRuntime, Arc<Held>) {
    let (runtime, mut held) = owners_running_with_requirement(
        &[OWNER],
        crate::runtime::pre_world_report(),
        Requirement::Optional,
    );
    (runtime, held.remove(0))
}

fn failed_optional() -> (LoaderRuntime, Arc<Held>) {
    let (mut machine, hosts, mut held) = owner_parts(&[OWNER], Requirement::Optional);
    machine
        .fail(&id(OWNER), attribution())
        .expect("optional owner fails before opening");
    (
        LoaderRuntime::new(machine, hosts, crate::runtime::test_launch()),
        held.remove(0),
    )
}

#[test]
fn a_panicking_save_callback_fails_the_owner_without_poisoning_runtime() {
    let (runtime, held) = running();
    runtime
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("fresh world");
    held.panics_production.store(true, Ordering::SeqCst);
    let SaveOutcome::Failed(detail) = runtime.save_world(&WorldIdentity::of(FIRST)) else {
        panic!("a panicking callback cannot produce durable data");
    };
    assert!(!detail.contains(concat!("ghp_", "private")), "{detail}");
    let inner = runtime
        .inner
        .lock()
        .expect("caught panic must not poison runtime");
    assert_eq!(
        inner.loaded.machine().state(&id(OWNER)),
        Ok(LifecycleState::Failed)
    );
    assert!(inner.loaded.machine().world_access(&inner.report).is_err());
    drop(inner);
}

#[test]
fn a_save_failure_after_world_mutation_halts_even_an_optional_owner() {
    let (runtime, held) = optional_running();
    runtime
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("fresh world");
    held.visits.store(7, Ordering::SeqCst);
    let durable = replacement(runtime.save_world(&WorldIdentity::of(FIRST)));
    held.refuses_production.store(true, Ordering::SeqCst);
    assert!(matches!(
        runtime.save_world(&WorldIdentity::of(FIRST)),
        SaveOutcome::Failed(_)
    ));
    for _ in 0..2 {
        assert!(matches!(
            runtime.save_world(&WorldIdentity::of(FIRST)),
            SaveOutcome::Failed(_)
        ));
    }
    let inner = runtime.inner.lock().expect("test lock");
    let live = inner.persistence.live.as_ref().expect("durable checkpoint");
    assert_eq!(live.aggregate.to_bytes().expect("bounded"), durable);
    drop(inner);
}

#[test]
fn an_optional_failure_after_open_withholds_another_world() {
    let (runtime, held) = optional_running();
    runtime
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("fresh world");
    held.visits.store(7, Ordering::SeqCst);
    let durable = replacement(runtime.save_world(&WorldIdentity::of(FIRST)));
    held.refuses_production.store(true, Ordering::SeqCst);
    assert!(matches!(
        runtime.save_world(&WorldIdentity::of(FIRST)),
        SaveOutcome::Failed(_)
    ));
    held.refuses_open.store(true, Ordering::SeqCst);
    assert!(
        runtime
            .stage_world(WorldIdentity::of(SECOND), Some(&durable), false)
            .is_err(),
        "world mutation prevents continuing after even an optional owner fails"
    );
    let inner = runtime.inner.lock().expect("test lock");
    assert_eq!(
        inner
            .persistence
            .live
            .as_ref()
            .expect("durable checkpoint")
            .aggregate
            .to_bytes()
            .expect("bounded"),
        durable
    );
}

#[test]
fn save_errors_fail_the_callback_owner_without_panicking() {
    let (runtime, held, _) = running_pair();
    runtime
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("fresh world");
    held.refuses_production.store(true, Ordering::SeqCst);
    assert!(matches!(
        runtime.save_world(&WorldIdentity::of(FIRST)),
        SaveOutcome::Failed(_)
    ));
    let inner = runtime.inner.lock().expect("callback errors do not panic");
    assert_eq!(
        inner.loaded.machine().state(&id(OWNER)).expect("owner"),
        LifecycleState::Failed
    );
    assert_eq!(
        inner
            .loaded
            .machine()
            .state(&id(OTHER_OWNER))
            .expect("other owner"),
        LifecycleState::Running
    );
    assert_eq!(
        inner
            .loaded
            .machine()
            .failure(&id(OWNER))
            .expect("owner")
            .expect("failure recorded")
            .mod_id,
        Some(id(OWNER))
    );
    drop(inner);
}

#[test]
fn save_panics_are_caught_and_attributed_for_every_requirement() {
    for requirement in [Requirement::Required, Requirement::Optional] {
        let (runtime, held) = owners_running_with_requirement(
            &[OWNER],
            crate::runtime::pre_world_report(),
            requirement,
        );
        runtime
            .stage_world(WorldIdentity::of(FIRST), None, true)
            .expect("a new world stages");
        held[0].panics_production.store(true, Ordering::SeqCst);
        assert!(matches!(
            runtime.save_world(&WorldIdentity::of(FIRST)),
            SaveOutcome::Failed(_)
        ));
        let inner = runtime.inner.lock().expect("the save panic is contained");
        assert_eq!(
            inner.loaded.machine().state(&id(OWNER)),
            Ok(LifecycleState::Failed)
        );
        let failure = inner
            .loaded
            .machine()
            .failure(&id(OWNER))
            .expect("owner")
            .expect("failure");
        assert_eq!(failure.mod_id, Some(id(OWNER)));
        assert_eq!(failure.state, LifecycleState::Running);
        assert_eq!(failure.environment, crate::runtime::environment());
        assert_eq!(failure.surface, None);
        assert!(failure.message.text().contains("panicked"));
        assert!(!failure.message.text().contains(concat!("ghp_", "private")));
        assert!(inner.loaded.machine().world_access(&inner.report).is_err());
        drop(inner);
        assert!(matches!(
            runtime.save_world(&WorldIdentity::of(FIRST)),
            SaveOutcome::Failed(_)
        ));
    }
}

#[test]
fn admitted_interception_declarations_are_not_requeried_during_world_saves() {
    for requirement in [Requirement::Required, Requirement::Optional] {
        let (runtime, held) = owners_running_with_requirement(
            &[OWNER],
            crate::runtime::pre_world_report(),
            requirement,
        );
        runtime
            .stage_world(WorldIdentity::of(FIRST), None, true)
            .expect("a new world stages");
        held[0]
            .refuses_interception_declaration
            .store(true, Ordering::SeqCst);
        assert!(matches!(
            runtime.save_world(&WorldIdentity::of(FIRST)),
            SaveOutcome::Replacement(_)
        ));
        let inner = runtime.inner.lock().expect("the callback returned");
        assert_eq!(
            inner
                .loaded
                .machine()
                .state(&id(OWNER))
                .expect("known owner"),
            LifecycleState::Running
        );
        assert!(
            inner
                .loaded
                .machine()
                .failure(&id(OWNER))
                .expect("known owner")
                .is_none()
        );
        drop(inner);
    }
}

#[test]
fn save_callback_errors_after_open_fail_the_owner_and_halt_every_requirement() {
    for requirement in [Requirement::Required, Requirement::Optional] {
        let held = Arc::new(Held::default());
        let mut machine = LifecycleMachine::new(
            Side::Client,
            vec![LifecycleEntry {
                mod_id: id(OWNER),
                sides: SideSet::new([Side::Client, Side::IntegratedServer])
                    .expect("client and integrated world owner"),
                requirement,
                saved_data_versions: BTreeSet::from([version(1), version(2)]),
            }],
        )
        .expect("one owner is a legal plan");
        advance_through(&mut machine, LifecycleState::Running);
        let runtime = LoaderRuntime::new(
            machine,
            vec![Box::new(Owning {
                id: id(OWNER),
                held: Arc::clone(&held),
            })],
            crate::runtime::test_launch(),
        );
        runtime
            .stage_world(WorldIdentity::of(FIRST), None, true)
            .expect("a new world stages");
        held.refuses_production.store(true, Ordering::SeqCst);
        assert!(matches!(
            runtime.save_world(&WorldIdentity::of(FIRST)),
            SaveOutcome::Failed(_)
        ));
        let inner = runtime
            .inner
            .lock()
            .expect("the callback returned an error");
        assert_eq!(
            inner
                .loaded
                .machine()
                .state(&id(OWNER))
                .expect("known owner"),
            LifecycleState::Failed
        );
        let failure = inner
            .loaded
            .machine()
            .failure(&id(OWNER))
            .expect("known owner")
            .expect("structured callback failure");
        assert_eq!(failure.mod_id, Some(id(OWNER)));
        assert_eq!(failure.state, LifecycleState::Running);
        assert!(
            failure
                .message
                .text()
                .contains("cannot produce an envelope")
        );
        assert!(
            inner.loaded.machine().world_access(&inner.report).is_err(),
            "a save failure after opening must halt every requirement class"
        );
        drop(inner);
        let retry = runtime.save_world(&WorldIdentity::of(FIRST));
        assert!(matches!(retry, SaveOutcome::Failed(_)));
    }
}

fn replacement(outcome: SaveOutcome) -> Vec<u8> {
    match outcome {
        SaveOutcome::Replacement(bytes) => bytes,
        other @ (SaveOutcome::Unchanged
        | SaveOutcome::Withheld
        | SaveOutcome::Failed(_)
        | SaveOutcome::IdentityMismatch(_)) => {
            panic!("this save should have replaced the aggregate: {other:?}")
        }
    }
}

fn visits(bytes: &[u8]) -> u64 {
    visits_for(bytes, &[OWNER], OWNER)
}

fn visits_for(bytes: &[u8], owners: &[&str], owner: &str) -> u64 {
    let declarations: Vec<SavedDataDeclaration> = owners
        .iter()
        .map(|name| {
            SavedDataDeclaration::new(id(name), [version(1), version(2)]).expect("two versions")
        })
        .collect();
    SavedDataAggregate::open(
        Some(bytes),
        &declarations,
        &std::collections::BTreeMap::new(),
    )
    .expect("a produced aggregate is admissible")
    .envelope(&id(owner))
    .expect("the owner is in the aggregate")
    .payload()["visits"]
        .as_u64()
        .expect("the payload records a visit count")
}

fn stored_visits(count: u32) -> Vec<u8> {
    stored_at(version(2), count)
}

fn stored_at(at: SavedDataVersion, count: u32) -> Vec<u8> {
    stored_owning(&[(OWNER, at, count)])
}

fn stored_owning(entries: &[(&str, SavedDataVersion, u32)]) -> Vec<u8> {
    let mut aggregate = SavedDataAggregate::fresh();
    for (owner, at, count) in entries {
        aggregate.replace(SavedDataEnvelope::new(
            id(owner),
            *at,
            Vec::new(),
            None,
            json!({ "visits": count }),
        ));
    }
    aggregate.to_bytes().expect("bounded")
}

#[test]
fn a_stored_aggregate_reaches_the_mod_that_owns_it() {
    let (runtime, held) = running();

    runtime
        .stage_world(WorldIdentity::of(FIRST), Some(&stored_visits(5)), false)
        .expect("a readable aggregate opens its world");

    assert_eq!(
        held.visits.load(Ordering::SeqCst),
        5,
        "the world's stored state never reached the mod that owns it"
    );
}

#[test]
fn a_world_written_at_an_older_version_is_migrated_before_delivery() {
    let (runtime, held) = running();

    runtime
        .stage_world(
            WorldIdentity::of(FIRST),
            Some(&stored_at(version(1), 5)),
            false,
        )
        .expect("a version this build declares a migration for opens its world");

    let delivered = held.opened_with();
    assert_eq!(delivered.owner(), &id(OWNER));
    assert_eq!(
        delivered.data_version(),
        version(2),
        "delivery receives the migrated version"
    );
    assert_eq!(held.visits.load(Ordering::SeqCst), 6);
    let bytes = replacement(runtime.save_world(&WorldIdentity::of(FIRST)));
    assert_eq!(visits(&bytes), 6, "the migrated payload survives saving");

    let (current, current_held) = running();
    current
        .stage_world(
            WorldIdentity::of(FIRST),
            Some(&stored_at(version(2), 5)),
            false,
        )
        .expect("a current world opens");
    assert_eq!(current_held.opened_with().data_version(), version(2));
}

#[test]
fn each_owner_in_one_world_receives_its_own_envelope_and_no_other() {
    let owners = [OWNER, OTHER_OWNER];
    let stored = stored_owning(&[(OWNER, version(2), 5), (OTHER_OWNER, version(1), 9)]);

    let (runtime, first, second) = running_pair();
    runtime
        .stage_world(WorldIdentity::of(FIRST), Some(&stored), false)
        .expect("a world holding both owners' data opens");

    assert_eq!(
        first.visits.load(Ordering::SeqCst),
        5,
        "`{OWNER}` did not receive its own state"
    );
    assert_eq!(
        second.visits.load(Ordering::SeqCst),
        10,
        "`{OTHER_OWNER}` did not receive its own state"
    );
    assert_eq!(first.opened_with().owner(), &id(OWNER));
    assert_eq!(second.opened_with().owner(), &id(OTHER_OWNER));
    assert_eq!(first.opened_with().data_version(), version(2));
    assert_eq!(second.opened_with().data_version(), version(2));

    let written = replacement(runtime.save_world(&WorldIdentity::of(FIRST)));
    assert_eq!(visits_for(&written, &owners, OWNER), 5);
    assert_eq!(visits_for(&written, &owners, OTHER_OWNER), 10);
}

#[test]
fn a_world_reopened_from_what_its_save_emitted_holds_the_same_state() {
    let (first, held) = running();
    first
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("a new world stages");
    held.visits.store(11, Ordering::SeqCst);
    let stored = replacement(first.save_world(&WorldIdentity::of(FIRST)));

    let (second, reopened) = running();
    second
        .stage_world(WorldIdentity::of(FIRST), Some(&stored), false)
        .expect("the bytes a save emitted open the world they came from");

    assert_eq!(
        reopened.visits.load(Ordering::SeqCst),
        11,
        "the round trip lost the state the save emitted"
    );
    assert_eq!(
        visits(&replacement(second.save_world(&WorldIdentity::of(FIRST)))),
        11
    );
}

#[test]
fn a_world_reopened_in_one_session_writes_what_the_second_visit_held() {
    let (runtime, held) = running();
    runtime
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("a new world stages");
    held.visits.store(11, Ordering::SeqCst);
    let first_visit = replacement(runtime.save_world(&WorldIdentity::of(FIRST)));
    assert_eq!(visits(&first_visit), 11);

    runtime
        .stage_world(WorldIdentity::of(FIRST), Some(&first_visit), false)
        .expect("the world its own save wrote reopens");
    assert_eq!(held.visits.load(Ordering::SeqCst), 11);
    held.visits.store(12, Ordering::SeqCst);

    let second_visit = replacement(runtime.save_world(&WorldIdentity::of(FIRST)));
    assert_eq!(
        visits(&second_visit),
        12,
        "the second visit's state never reached the world's file"
    );

    assert_eq!(
        runtime.save_world(&WorldIdentity::of(FIRST)),
        SaveOutcome::Unchanged
    );
    held.visits.store(13, Ordering::SeqCst);
    assert_eq!(
        visits(&replacement(runtime.save_world(&WorldIdentity::of(FIRST)))),
        13
    );
}

fn shipped_running() -> LoaderRuntime {
    let shipped = ravel_test_mod_saved_data::SavedDataMod::new();
    let entries = vec![LifecycleEntry {
        mod_id: shipped.id().clone(),
        sides: SideSet::new([Side::Client, Side::IntegratedServer])
            .expect("client and integrated world owner"),
        requirement: Requirement::Required,
        saved_data_versions: shipped
            .declaration()
            .expect("valid declaration")
            .versions()
            .collect(),
    }];
    let mut machine =
        LifecycleMachine::new(Side::Client, entries).expect("one mod is a legal plan");
    advance_through(&mut machine, LifecycleState::Running);
    LoaderRuntime::with_observers(
        machine,
        vec![Box::new(shipped)],
        ravel_observability::Profiler::off(),
        crate::runtime::pre_world_report(),
        crate::runtime::test_launch(),
        std::collections::BTreeMap::new(),
    )
}

#[test]
fn a_launch_with_no_declared_resources_still_stages_pack_metadata() {
    let staging = std::env::temp_dir().join(format!("ravel-unstaged-{}", std::process::id()));
    if staging.exists() {
        std::fs::remove_dir_all(&staging).expect("remove the previous staging directory");
    }
    std::fs::create_dir_all(&staging).expect("a writable temporary directory");

    let staged = shipped_running().write_staged_pack(&staging);

    assert_eq!(staged, StagedPack::Written);
    assert!(staging.join("ravel/pack.mcmeta").is_file());
    std::fs::remove_dir_all(staging).expect("remove the staging directory");
}

fn shipped_visits(bytes: &[u8]) -> u64 {
    let declaration = ravel_test_mod_saved_data::SavedDataMod::new()
        .declaration()
        .expect("valid declaration");
    let owner = declaration.owner().clone();
    SavedDataAggregate::open(
        Some(bytes),
        std::slice::from_ref(&declaration),
        &std::collections::BTreeMap::new(),
    )
    .expect("a produced aggregate is admissible")
    .envelope(&owner)
    .expect("the shipped owner is in the aggregate")
    .payload()["visits"]
        .as_u64()
        .expect("the payload records a visit count")
}

#[test]
fn the_shipped_world_owner_stores_a_different_world_after_a_second_visit() {
    let runtime = shipped_running();
    runtime
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("a new world stages");
    let first = replacement(runtime.save_world(&WorldIdentity::of(FIRST)));
    assert_eq!(
        shipped_visits(&first),
        1,
        "opening a world left the shipped owner with nothing to store"
    );

    runtime
        .stage_world(WorldIdentity::of(FIRST), Some(&first), false)
        .expect("the world its own save wrote reopens");
    let second = replacement(runtime.save_world(&WorldIdentity::of(FIRST)));

    assert_eq!(
        shipped_visits(&second),
        2,
        "the reopen did not hand the shipped owner back what the world stored"
    );
    assert_ne!(
        first, second,
        "two sessions of one world stored byte-identical payloads"
    );
}

#[test]
fn a_mod_refusing_its_stored_state_withholds_the_world_and_stages_nothing() {
    let (accepting, _held) = running();
    accepting
        .stage_world(WorldIdentity::of(FIRST), Some(&stored_visits(5)), false)
        .expect("a mod that accepts delivery opens its world");

    let (runtime, held) = running();
    held.refuses_open.store(true, Ordering::SeqCst);

    let refused = runtime
        .stage_world(WorldIdentity::of(FIRST), Some(&stored_visits(5)), false)
        .expect_err("a mod that cannot read its stored state opens no world")
        .to_string();
    assert!(refused.contains(OWNER), "{refused}");
    assert!(
        refused.contains("cannot read what the world stored"),
        "{refused}"
    );

    {
        let inner = runtime
            .inner
            .lock()
            .expect("refusal leaves the lock healthy");
        assert!(
            inner.persistence.staged.is_none(),
            "the refused world was staged"
        );
        assert!(
            inner.persistence.live.is_none(),
            "the refused world became live"
        );
        drop(inner);
    }
    let SaveOutcome::Failed(detail) = runtime.save_world(&WorldIdentity::of(FIRST)) else {
        panic!("the failed required owner must block saving");
    };
    assert!(detail.contains(OWNER), "{detail}");
}

#[test]
fn an_absent_aggregate_stages_fresh_state_and_the_first_save_promotes_it() {
    let (runtime, held) = running();

    assert_eq!(
        runtime.stage_world(WorldIdentity::of(FIRST), None, true),
        Ok(())
    );
    held.visits.store(4, Ordering::SeqCst);

    assert_eq!(
        visits(&replacement(runtime.save_world(&WorldIdentity::of(FIRST)))),
        4
    );
    assert_eq!(
        runtime.save_world(&WorldIdentity::of(FIRST)),
        SaveOutcome::Unchanged
    );
}

#[test]
fn an_unmatched_save_receiver_never_replaces_the_aggregate() {
    let (runtime, _held) = running();
    runtime
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("absence stages");

    let outcome = runtime.save_world(&WorldIdentity::of(SECOND));

    assert!(
        matches!(outcome, SaveOutcome::IdentityMismatch(_)),
        "a save from a world Ravel never opened wrote to it anyway: {outcome:?}"
    );
    assert_eq!(
        visits(&replacement(runtime.save_world(&WorldIdentity::of(FIRST)))),
        0
    );
}

#[test]
fn an_idle_autosave_leaves_the_stored_aggregate_alone() {
    let (runtime, held) = running();
    runtime
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("absence stages");
    replacement(runtime.save_world(&WorldIdentity::of(FIRST)));

    assert_eq!(
        runtime.save_world(&WorldIdentity::of(FIRST)),
        SaveOutcome::Unchanged
    );

    held.visits.store(1, Ordering::SeqCst);
    assert_eq!(
        visits(&replacement(runtime.save_world(&WorldIdentity::of(FIRST)))),
        1
    );
}

#[test]
fn a_production_failure_preserves_the_previous_aggregate() {
    let (runtime, held) = running();
    runtime
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("absence stages");
    held.visits.store(7, Ordering::SeqCst);
    let durable = replacement(runtime.save_world(&WorldIdentity::of(FIRST)));

    held.visits.store(9, Ordering::SeqCst);
    held.refuses_production.store(true, Ordering::SeqCst);
    let outcome = runtime.save_world(&WorldIdentity::of(FIRST));
    assert!(
        matches!(outcome, SaveOutcome::Failed(_)),
        "a refused envelope was reported as something else: {outcome:?}"
    );

    assert_eq!(visits(&durable), 7);
    held.refuses_production.store(false, Ordering::SeqCst);
    assert!(matches!(
        runtime.save_world(&WorldIdentity::of(FIRST)),
        SaveOutcome::Failed(_)
    ));
}

#[test]
fn the_prior_world_saves_the_state_it_held_when_the_next_world_was_staged() {
    let (runtime, held) = running();
    runtime
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("absence stages");
    held.visits.store(3, Ordering::SeqCst);
    replacement(runtime.save_world(&WorldIdentity::of(FIRST)));

    held.visits.store(5, Ordering::SeqCst);
    runtime
        .stage_world(WorldIdentity::of(SECOND), None, true)
        .expect("the second world stages");
    assert_eq!(
        held.visits.load(Ordering::SeqCst),
        0,
        "staging the second world left the first world's state in the mod"
    );

    assert_eq!(
        visits(&replacement(runtime.save_world(&WorldIdentity::of(FIRST)))),
        5,
        "the prior world's teardown save wrote the incoming world's state"
    );
    assert_eq!(
        visits(&replacement(runtime.save_world(&WorldIdentity::of(SECOND)))),
        0
    );
}

#[test]
fn each_open_replaces_the_prior_stage() {
    let (runtime, _held) = running();
    runtime
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("absence stages");
    runtime
        .stage_world(WorldIdentity::of(SECOND), None, true)
        .expect("the second world stages");

    let outcome = runtime.save_world(&WorldIdentity::of(FIRST));
    assert!(
        matches!(outcome, SaveOutcome::IdentityMismatch(_)),
        "a stage the next open replaced still answered a save: {outcome:?}"
    );
    replacement(runtime.save_world(&WorldIdentity::of(SECOND)));
}

#[test]
fn an_oversized_aggregate_is_refused_before_it_is_parsed() {
    let (runtime, _held) = running();
    let oversized = vec![b'\0'; MAX_AGGREGATE_BYTES + 1];

    let refused = runtime
        .stage_world(WorldIdentity::of(FIRST), Some(&oversized), true)
        .expect_err("an aggregate over the ceiling does not open a world")
        .to_string();

    assert!(refused.contains("limit"), "{refused}");
    assert_eq!(
        runtime.save_world(&WorldIdentity::of(FIRST)),
        SaveOutcome::IdentityMismatch(
            "save receiver `worlds/first` matches neither live nor staged world state".to_owned()
        )
    );
}

#[test]
fn a_world_this_build_cannot_read_refuses_rather_than_staging() {
    let mut future = SavedDataAggregate::fresh();
    future.replace(SavedDataEnvelope::new(
        id(OWNER),
        version(9),
        Vec::new(),
        None,
        json!({ "visits": 1 }),
    ));
    let mut foreign = SavedDataAggregate::fresh();
    foreign.replace(SavedDataEnvelope::new(
        id("uninstalled"),
        version(1),
        Vec::new(),
        None,
        json!(null),
    ));

    for (name, bytes) in [
        ("future", future.to_bytes().expect("bounded")),
        ("foreign", foreign.to_bytes().expect("bounded")),
        ("torn", b"{\"formatVersion\":".to_vec()),
    ] {
        let (runtime, _held) = running();
        let refused = runtime
            .stage_world(WorldIdentity::of(FIRST), Some(&bytes), true)
            .expect_err("this aggregate does not open a world")
            .to_string();
        assert!(!refused.is_empty(), "`{name}` refused without saying why");
        assert!(
            matches!(
                runtime.save_world(&WorldIdentity::of(FIRST)),
                SaveOutcome::IdentityMismatch(_)
            ),
            "`{name}` was refused and staged anyway"
        );
    }
}

#[test]
fn the_two_shapes_one_world_arrives_as_are_the_same_world() {
    let world = WorldIdentity::of("saves/world");
    assert_eq!(WorldIdentity::of("saves/world/."), world);
    assert_eq!(WorldIdentity::of("saves/world/"), world);
    assert_eq!(WorldIdentity::of("saves//world//."), world);
    assert_eq!(
        WorldIdentity::of("/saves/world/."),
        WorldIdentity::of("/saves/world")
    );
    assert_eq!(
        WorldIdentity::of("/saves/world/.").to_string(),
        "/saves/world",
        "a rooted path stays rooted, so two worlds under different roots stay apart"
    );
}

#[test]
fn a_save_addressed_in_the_other_seams_shape_still_finds_its_own_world() {
    let (runtime, held) = running();
    runtime
        .stage_world(WorldIdentity::of("saves/world/."), None, true)
        .expect("the level path stages a world");
    held.visits.store(6, Ordering::SeqCst);

    let outcome = runtime.save_world(&WorldIdentity::of("saves/world"));

    assert!(
        !matches!(outcome, SaveOutcome::IdentityMismatch(_)),
        "the two shapes of one world stopped selecting one checkpoint: {outcome:?}"
    );
    assert_eq!(visits(&replacement(outcome)), 6);
}

#[test]
fn two_different_worlds_are_never_the_same_world() {
    assert_ne!(
        WorldIdentity::of("saves/first"),
        WorldIdentity::of("saves/second")
    );
    assert_ne!(
        WorldIdentity::of("saves/world"),
        WorldIdentity::of("other/world")
    );
    assert_ne!(
        WorldIdentity::of("saves/world"),
        WorldIdentity::of("/saves/world"),
        "one is under the game directory and one is not"
    );
    assert_ne!(
        WorldIdentity::of("saves/world"),
        WorldIdentity::of("saves/world/nether"),
        "collapsing separators must not collapse a path into its parent"
    );
}

#[test]
fn every_missing_data_notice_reaches_the_player_report() {
    let (runtime, _held) = running();
    for index in 0..3 {
        runtime
            .stage_world(WorldIdentity::of(&format!("worlds/w{index}")), None, false)
            .expect("an existing world with no Ravel data still stages");
    }
    let observed = notices(&runtime);
    assert_eq!(observed.len(), 3);
    for (index, notice) in observed.iter().enumerate() {
        assert!(notice.contains(&format!("worlds/w{index}")), "{notice}");
    }
}

#[test]
fn a_world_requiring_a_package_is_refused_and_the_same_world_without_one_is_not() {
    let requiring = stored_requiring(vec![ravel_save::RequiredPackage {
        id: id("base"),
        version: ravel_core::Version::new(2, 0, 0),
    }]);
    let requiring_none = stored_requiring(Vec::new());

    let (runtime, _held) = running();
    let refused = runtime
        .stage_world(WorldIdentity::of(FIRST), Some(&requiring), true)
        .expect_err("a requirement this process cannot check opens no world")
        .to_string();
    assert!(refused.contains("base"), "{refused}");
    assert!(
        matches!(
            runtime.save_world(&WorldIdentity::of(FIRST)),
            SaveOutcome::IdentityMismatch(_)
        ),
        "a refused world was staged anyway"
    );

    let (runtime, _held) = running();
    runtime
        .stage_world(WorldIdentity::of(FIRST), Some(&requiring_none), true)
        .expect("the same world declaring no requirement opens");

    let (runtime, _held) = running();
    runtime
        .inner
        .lock()
        .expect("healthy lock")
        .installed_packages
        .insert(id("base"), ravel_core::Version::new(2, 0, 0));
    runtime
        .stage_world(WorldIdentity::of(FIRST), Some(&requiring), true)
        .expect("an installed matching package satisfies the world's requirement");
}

fn stored_requiring(required: Vec<ravel_save::RequiredPackage>) -> Vec<u8> {
    let mut aggregate = SavedDataAggregate::fresh();
    aggregate.replace(SavedDataEnvelope::new(
        id(OWNER),
        version(1),
        required,
        None,
        json!({ "visits": 1 }),
    ));
    aggregate.to_bytes().expect("bounded")
}

#[test]
fn an_existing_world_that_lost_its_data_is_noticed_and_admitted_anyway() {
    let (runtime, _held) = running();

    runtime
        .stage_world(WorldIdentity::of(FIRST), None, false)
        .expect("absence still initializes fresh state for any world");

    let observed = notices(&runtime);
    assert_eq!(observed.len(), 1, "{observed:?}");
    assert!(observed[0].contains(FIRST), "{observed:?}");
    assert!(observed[0].contains("holds no Ravel data"), "{observed:?}");
    replacement(runtime.save_world(&WorldIdentity::of(FIRST)));
}

#[test]
fn a_new_world_with_no_data_is_the_ordinary_case_and_says_nothing() {
    const CHILD: &str = "RAVEL_FRESH_WORLD_PACK_CHILD";
    if std::env::var_os(CHILD).is_none() {
        let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
            .args(["--exact", "persistence::tests::a_new_world_with_no_data_is_the_ordinary_case_and_says_nothing", "--nocapture"])
            .env(CHILD, "1").output().expect("isolated world crossing");
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
        return;
    }
    let game = scratch("fresh-world-pack");
    let world = game.join("world");
    std::fs::create_dir_all(&world).expect("new world directory");
    let (runtime, _held) = running();
    let inner = runtime.inner.into_inner().expect("runtime state");
    let mut machine = LifecycleMachine::new(
        Side::Client,
        vec![LifecycleEntry {
            mod_id: id(OWNER),
            sides: SideSet::new([Side::Client, Side::IntegratedServer])
                .expect("client and integrated world resource supplier"),
            requirement: Requirement::Required,
            saved_data_versions: BTreeSet::from([version(1), version(2)]),
        }],
    )
    .expect("supplier plan");
    advance_through(&mut machine, LifecycleState::Running);
    let mut suppliers: Vec<Box<dyn Mod>> = vec![Box::new(FreshWorldPackSupplier(id(OWNER)))];
    let resources = ravel_loader::open_resources(
        &mut LoadedMods::new(&mut machine, &mut suppliers).expect("paired supplier"),
        inner.report.environment(),
    )
    .expect("supplied world data");
    crate::runtime::install(
        crate::runtime::Composition {
            loaded: inner.loaded,
            configurations: Vec::new(),
            resources,
            interceptions: inner.interceptions,
            installed_packages: inner.installed_packages,
        },
        inner.report,
        game.clone(),
        ravel_observability::Profiler::from_switch(true),
    )
    .expect("install isolated runtime");
    assert_installed_process_context(&game);
    assert_eq!(
        super::open_world_data(world.to_str().expect("absolute world path"), None, true),
        OpenOutcome::Staged
    );
    assert!(
        !world.join(crate::packs::WORLD_DATA_PACKS).exists(),
        "world admission must not write a pack"
    );
    let selected = crate::runtime::selected().expect("installed runtime");
    super::write_world_datapack(&world);
    assert_fresh_world_pack(&world);
    assert!(notices(&selected).is_empty(), "{:?}", notices(&selected));
    assert_malformed_world_refuses_at_the_public_crossing(&world);
    std::fs::remove_dir_all(&selected.launch.game_directory).expect("remove scratch game");
}

fn assert_malformed_world_refuses_at_the_public_crossing(world: &std::path::Path) {
    let identity = world.to_str().expect("absolute world path");
    let OpenOutcome::Refused(diagnostic) =
        super::open_world_data(identity, Some(b"{\"formatVersion\":"), false)
    else {
        panic!("malformed saved data must refuse through the public crossing");
    };
    assert!(
        !diagnostic.is_empty(),
        "admission refusal needs a diagnostic"
    );
}

fn assert_installed_process_context(game: &std::path::Path) {
    let (directory, profiler) = crate::runtime::process_launch_context()
        .expect("installed process supplies launch context");
    assert_eq!(directory, game);
    assert_ne!(
        directory,
        std::env::current_dir().expect("test working directory")
    );
    assert!(
        profiler.is_on(),
        "the installed process profiling switch is retained"
    );
}

fn assert_fresh_world_pack(world: &std::path::Path) {
    let metadata = std::fs::read(
        world
            .join(crate::packs::WORLD_DATA_PACKS)
            .join(crate::packs::GAME_PACK_DIRECTORY)
            .join("pack.mcmeta"),
    )
    .expect("real world pack metadata");
    let generated: serde_json::Value =
        serde_json::from_slice(&metadata).expect("generated metadata JSON");
    assert!(generated.get("pack").is_some(), "{generated}");
    assert_eq!(
        std::fs::read(
            world
                .join(crate::packs::WORLD_DATA_PACKS)
                .join(crate::packs::GAME_PACK_DIRECTORY)
                .join(format!("data/{OWNER}/tags/items/fresh.json"))
        )
        .expect("supplied world tag"),
        br#"{"values":[]}"#
    );
}

#[test]
fn a_halted_set_cannot_open_a_world_at_the_crossing_the_game_calls() {
    let (runtime, held) = running();
    runtime
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect("a running set opens a world");

    held.refuses_interception.store(true, Ordering::SeqCst);
    assert!(matches!(
        runtime.save_world(&WorldIdentity::of(FIRST)),
        SaveOutcome::Failed(_)
    ));
    assert!(
        runtime
            .inner
            .lock()
            .expect("callback failure leaves the runtime lock healthy")
            .loaded
            .machine()
            .save_access()
            .is_err(),
        "the real callback failure must latch the post-mutation halt"
    );

    let refused = runtime
        .stage_world(WorldIdentity::of(SECOND), None, true)
        .expect_err("a halted set opens no world")
        .to_string();
    assert!(refused.starts_with("no world opens: "), "{refused}");
    assert!(refused.contains(OWNER), "{refused}");
}

#[test]
fn a_refusal_the_machine_never_saw_still_withholds_the_world() {
    let mut report = crate::runtime::pre_world_report();
    report.refuse(
        &ravel_observability::Subject::of(id("example_mod")),
        &Unresolvable,
    );
    let (runtime, _held) = running_with(report);

    let refused = runtime
        .stage_world(WorldIdentity::of(FIRST), None, true)
        .expect_err("a report holding a refusal withholds the world")
        .to_string();
    assert!(
        refused.contains("`example_mod` requires `example_library`"),
        "{refused}"
    );
    assert!(
        running()
            .0
            .stage_world(WorldIdentity::of(FIRST), None, true)
            .is_ok()
    );
}

#[test]
fn a_refusal_carries_the_diagnostic_to_the_java_adapter() {
    assert_eq!(
        OpenOutcome::from(ravel_save::SavedDataError::AggregateTooLarge {
            limit: MAX_AGGREGATE_BYTES
        }),
        OpenOutcome::Refused(
            "the canonical saved-data aggregate exceeds the 1048576-byte limit".to_owned()
        )
    );
}

fn java_int(source: &str, name: &str) -> i32 {
    let (_, rest) = source
        .split_once(&format!("static final int {name} = "))
        .unwrap_or_else(|| panic!("the Java side names `{name}`"));
    rest.split(';')
        .next()
        .expect("a declaration ends in a semicolon")
        .trim()
        .parse()
        .expect("a Java int constant is an integer")
}

fn documented_staged_answers(generator: &str) -> Vec<i32> {
    let (rendered, _) = generator
        .split_once("public static native int ravelWriteStagedDatapack")
        .expect("the generator renders the staging crossing");
    let (_, answers) = rendered
        .rsplit_once("@return")
        .expect("the staging crossing documents what it answers");
    answers
        .split("{@code ")
        .skip(1)
        .filter_map(|tail| tail.split('}').next()?.trim().parse().ok())
        .collect()
}

#[test]
fn every_java_spelling_of_a_staged_answer_is_the_number_rust_gives_it() {
    let root = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let read = |relative: &str| {
        std::fs::read_to_string(root.join(relative))
            .unwrap_or_else(|absent| panic!("{relative} is a tracked file: {absent}"))
    };
    let adapter = read("jvm/bootstrap/dev/ravel/jvm/Datapack.java");
    let standin = read("jvm/harness/initialization/dev/ravel/jvm/Hook.java");
    let generator = read("crates/ravel-minecraft-generate/src/render/hooks.rs");

    let nothing = i32::from(StagedPack::Nothing);
    let unchanged = i32::from(StagedPack::Unchanged);
    let written = i32::from(StagedPack::Written);

    assert_eq!(java_int(&adapter, "UNSTAGED"), nothing);
    assert_eq!(java_int(&adapter, "STAGED_ANEW"), written);
    assert_eq!(java_int(&standin, "UNSTAGED"), nothing);
    assert_eq!(java_int(&standin, "STAGED_UNCHANGED"), unchanged);
    assert_eq!(java_int(&standin, "STAGED_ANEW"), written);
    assert_eq!(
        documented_staged_answers(&generator),
        vec![nothing, unchanged, written]
    );
}
