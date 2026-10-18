#![forbid(unsafe_code)]

//! Saved-data versioning and migration test mod.

use std::num::NonZero;

use ravel_sdk::config::{ConfigurationDeclaration, LayeredConfiguration};
use ravel_sdk::save::{
    MigrationChain, MigrationFailed, SavedDataDeclaration, SavedDataEnvelope, SavedDataError,
    SavedDataPlan, SavedDataVersion,
};
use ravel_sdk::{Declared, LifecycleState, Mod, ModError, declare_mod};
use serde::{Deserialize, Serialize};

/// The canonical identity this mod is packaged and validated under.
pub const MOD_ID: &str = "saved_data";

/// The settings document packaged beside this mod.
///
/// Its one setting belongs to the integrated server, which is the side that owns
/// a world. A client or a dedicated server resolves nothing from it, so no
/// settings screen on either is offered for this mod.
pub const DECLARATION: &str = include_str!("../configuration.json");

/// The key this mod reads its one setting under.
const WAYPOINT_LIMIT: &str = "waypoints.limit";

/// Every version of its data this build can read, lowest first.
///
/// A packaged mod declares these in its manifest and the loader carries them into
/// the plan as `saved_data_versions`; `package.json` beside this file declares the
/// same three, which is what the composed host admits this mod under, and the
/// world fixtures under `tests/fixtures/public/worlds/` are written against them.
/// A test in `ravel-composed-host` fails if the two lists ever part company.
const VERSIONS: [u32; 3] = [1, 2, 3];

/// The data this mod owns in a world.
///
/// Ravel never looks inside this. `SPEC.md` §14 makes the owning mod the only
/// thing that interprets its own state, so what the loader decides is whether
/// this may be read at all, and the shape of it is this mod's business.
#[derive(Clone, Debug, Default, Deserialize, Eq, PartialEq, Serialize)]
pub struct SavedState {
    /// How many times the world has been opened.
    pub visits: u32,
    /// Named waypoints, which version 1 did not have.
    ///
    /// Absent rather than empty in a version-1 payload, so the field defaults on
    /// the way in. Without that, a version-1 world would fail to deserialize at
    /// the delivery step and never reach the migration that exists to fill this.
    #[serde(default)]
    pub waypoints: Vec<String>,
}

/// A mod that owns versioned saved data and steps an older world forward.
///
/// `ROADMAP.md`'s M3-W4 row asks for a save test mod and a world fixture over
/// the deciding half `ravel-save` already models. This mod is the owning side of
/// that decision: it declares which versions it reads, hands them to
/// [`SavedDataPlan`], and applies whatever steps the plan returns. It never
/// decides admissibility itself, because a mod that could would be able to open
/// a world `ravel-save` refused.
///
/// # Examples
///
/// ```
/// use std::num::NonZero;
///
/// use ravel_test_mod_saved_data::{SavedDataMod, SavedState};
///
/// let mut saved = SavedDataMod::new();
/// let version_one = NonZero::new(1).expect("a non-zero version");
///
/// // A version-1 world is stepped through every version above it.
/// let plan = saved
///     .open(Some((version_one, SavedState { visits: 4, waypoints: Vec::new() })))
///     .expect("this build reads version 1");
/// assert_eq!(plan.steps().len(), 2);
///
/// // Version 2 is what introduced the waypoint list, so the migration fills it.
/// assert_eq!(saved.state().visits, 4);
/// assert_eq!(saved.state().waypoints, ["spawn"]);
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct SavedDataMod {
    entered: Vec<LifecycleState>,
    state: SavedState,
    // Absent until the host states one, which is what an unconfigured mod knows.
    waypoint_limit: Option<usize>,
}

impl SavedDataMod {
    /// Builds the mod instance the loader drives, holding a world's worth of
    /// nothing until one is opened.
    #[must_use]
    pub fn new() -> Self {
        Self {
            entered: Vec::new(),
            state: SavedState::default(),
            waypoint_limit: None,
        }
    }

    /// Returns how many waypoints the settings in force let a world keep.
    #[must_use]
    pub const fn waypoint_limit(&self) -> Option<usize> {
        self.waypoint_limit
    }

    /// Returns what this mod declares about the data it owns.
    ///
    /// # Errors
    ///
    /// Returns an error if the declared versions are empty or invalid.
    pub fn declaration(&self) -> Result<SavedDataDeclaration, SavedDataError> {
        SavedDataDeclaration::new(
            Self::declared_id().clone(),
            VERSIONS.into_iter().map(Self::version),
        )
    }

    /// Returns the version this build writes.
    #[must_use]
    pub fn current(&self) -> SavedDataVersion {
        Self::version(3)
    }

    /// Opens what a world holds for this mod, stepping it to [`Self::current`].
    ///
    /// `stored` is `None` when the world holds nothing this mod owns. The plan
    /// decides whether that opens, rather than this mod deciding it.
    ///
    /// # Errors
    ///
    /// Returns whatever [`SavedDataPlan::open`] refuses: a world written by a
    /// newer build or one with no path across a gap in the declared versions.
    /// Absence initializes fresh state. Nothing is applied to
    /// [`Self::state`] unless the whole plan resolved, so a refused world leaves
    /// the mod holding what it held before.
    pub fn open(
        &mut self,
        stored: Option<(SavedDataVersion, SavedState)>,
    ) -> Result<SavedDataPlan, SavedDataError> {
        let declaration = self.declaration()?;
        let Some((version, mut state)) = stored else {
            let plan = SavedDataPlan::open(&declaration, None)?;
            self.state = SavedState::default();
            return Ok(plan);
        };

        let plan = SavedDataPlan::resolve(&declaration, version)?;
        for step in plan.steps() {
            Self::step(&mut state, *step);
        }
        self.state = state;
        Ok(plan)
    }

    /// Steps one version of the state forward into `to`.
    ///
    /// # Panics
    ///
    /// Panics on a version this mod does not declare. [`SavedDataPlan`] only
    /// ever names declared versions, so reaching that arm would mean the plan
    /// and [`VERSIONS`] had stopped agreeing.
    fn step(state: &mut SavedState, to: SavedDataVersion) {
        match to.get() {
            // Version 1 had no waypoint list and kept the spawn point implicit.
            2 => state.waypoints.push("spawn".to_owned()),
            // Version 2 stored waypoints in insertion order, so two worlds
            // holding the same waypoints did not compare equal.
            3 => {
                state.waypoints.sort();
                state.waypoints.dedup();
            }
            other => panic!("this mod declares no version {other} to step to"),
        }
    }

    /// Returns `raw` as a version this mod declares.
    ///
    /// # Panics
    ///
    /// Panics on a version outside [`VERSIONS`], which would mean this mod had
    /// stopped agreeing with its own declaration.
    fn version(raw: u32) -> SavedDataVersion {
        assert!(
            VERSIONS.contains(&raw),
            "this mod declares no version {raw}"
        );
        NonZero::new(raw).expect("every declared version is non-zero")
    }

    /// Runs [`Self::step`] over a stored payload, for one declared version.
    fn migrate(payload: &serde_json::Value, to: u32) -> Result<serde_json::Value, MigrationFailed> {
        let mut state: SavedState = serde_json::from_value(payload.clone())
            .map_err(|error| MigrationFailed::new(error.to_string()))?;
        Self::step(&mut state, Self::version(to));
        serde_json::to_value(&state).map_err(|error| MigrationFailed::new(error.to_string()))
    }

    fn migrate_to_two(payload: &serde_json::Value) -> Result<serde_json::Value, MigrationFailed> {
        Self::migrate(payload, 2)
    }

    fn migrate_to_three(payload: &serde_json::Value) -> Result<serde_json::Value, MigrationFailed> {
        Self::migrate(payload, 3)
    }

    /// Counts one opening of the world, which is what [`SavedState::visits`] is.
    ///
    /// Called from [`Mod::open_saved_data`] rather than only from a test,
    /// which is the whole difference between a counter and a field. Until it was
    /// called there, nothing in a running game ever changed this mod's state:
    /// every session produced the default payload, so the bytes a world stored
    /// were identical whatever had happened, and a save that restored nothing
    /// was indistinguishable from one that restored everything.
    pub const fn open_counted(&mut self) {
        self.state.visits = self.state.visits.saturating_add(1);
    }

    /// Records a visit to the world, which is what a reload has to preserve.
    pub fn visit(&mut self, waypoint: &str) {
        self.open_counted();
        self.state.waypoints.push(waypoint.to_owned());
        self.state.waypoints.sort();
        self.state.waypoints.dedup();
        if let Some(limit) = self.waypoint_limit {
            self.state.waypoints.truncate(limit);
        }
    }

    /// Returns what this mod would write, and the version it writes it under.
    #[must_use]
    pub fn save(&self) -> (SavedDataVersion, SavedState) {
        (self.current(), self.state.clone())
    }

    /// Returns the state this mod is holding.
    #[must_use]
    pub const fn state(&self) -> &SavedState {
        &self.state
    }

    /// Returns the lifecycle states the loader drove this mod through.
    #[must_use]
    pub fn entered(&self) -> &[LifecycleState] {
        &self.entered
    }
}

impl Default for SavedDataMod {
    fn default() -> Self {
        Self::new()
    }
}

#[declare_mod(id = "saved_data")]
impl Mod for SavedDataMod {
    /// Records the step and does nothing else.
    ///
    /// Opening saved data is not driven from here. `SPEC.md` §14 decides it
    /// before a world opens, which is earlier than any state a mod runs in, so
    /// [`SavedDataMod::open`] is called by whatever is opening the world.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        self.entered.push(state);
        Ok(())
    }

    fn saved_data_declaration(&self) -> Result<Option<SavedDataDeclaration>, ModError> {
        self.declaration().map(Some).map_err(ModError::from_display)
    }

    fn configuration_declaration(&self) -> Result<Option<ConfigurationDeclaration>, ModError> {
        ConfigurationDeclaration::parse(DECLARATION)
            .map(Some)
            .map_err(ModError::from_display)
    }

    /// Takes the waypoint limit the resolved settings put in force.
    ///
    /// # Errors
    ///
    /// Returns a [`ModError`] when this mod's own setting key stops
    /// being a legal one.
    fn configure(&mut self, resolved: &LayeredConfiguration) -> Result<(), ModError> {
        let key = WAYPOINT_LIMIT.parse().map_err(|_| {
            ModError::refused(format!("`{WAYPOINT_LIMIT}` is not a legal setting key"))
        })?;
        self.waypoint_limit = resolved
            .value(&key)
            .and_then(serde_json::Value::as_u64)
            .and_then(|limit| usize::try_from(limit).ok());
        Ok(())
    }

    /// Declares the step into every version above the oldest.
    ///
    /// Each step is `SavedDataMod::step` over the stored payload, so there is one
    /// implementation of the transformation and not a second one that could
    /// disagree with it. The chain exists because a host opening a world checks
    /// that every declared version has a way into it before it reads a payload,
    /// which is a check a typed `match` with a panicking arm cannot answer.
    fn saved_data_migrations(&self) -> Result<Option<MigrationChain>, ModError> {
        Ok(Some(
            MigrationChain::new(Self::declared_id().clone())
                .reaching(Self::version(2), Self::migrate_to_two)
                .reaching(Self::version(3), Self::migrate_to_three),
        ))
    }

    fn open_saved_data(
        &mut self,
        envelope: Option<&SavedDataEnvelope>,
        plan: &SavedDataPlan,
    ) -> Result<(), ModError> {
        let state = envelope
            .map(|stored| {
                serde_json::from_value::<SavedState>(stored.payload().clone())
                    .map(|state| (stored.data_version(), state))
            })
            .transpose()
            .map_err(ModError::from_display)?;
        let opened = self.open(state).map_err(ModError::from_display)?;
        if opened != *plan {
            return Err(ModError::refused(
                "the admitted saved-data plan changed during delivery",
            ));
        }
        // Last, so a delivery this mod refused above counts no visit and leaves
        // the state it was holding alone.
        self.open_counted();
        Ok(())
    }

    fn save_saved_data(&self) -> Result<Option<SavedDataEnvelope>, ModError> {
        let (version, state) = self.save();
        let payload = serde_json::to_value(state).map_err(ModError::from_display)?;
        Ok(Some(SavedDataEnvelope::new(
            Self::declared_id().clone(),
            version,
            Vec::new(),
            Some("urn:ravel:test:saved-state:1".to_owned()),
            payload,
        )))
    }
}

#[cfg(test)]
mod tests {
    use std::num::NonZero;

    use ravel_sdk::config::{ConfigurationDeclaration, LayeredConfiguration};
    use ravel_sdk::save::{SavedDataEnvelope, SavedDataPlan, SavedDataVersion};
    use ravel_sdk::{LifecycleState, Mod, ModId, Side};
    use serde_json::json;

    use super::{DECLARATION, MOD_ID, SavedDataMod, SavedState, VERSIONS};

    fn version(raw: u32) -> SavedDataVersion {
        NonZero::new(raw).expect("a non-zero version")
    }

    /// One envelope, as an admitted aggregate would hand it to this mod.
    fn envelope(at: u32, payload: serde_json::Value) -> SavedDataEnvelope {
        SavedDataEnvelope::new(
            MOD_ID.parse().expect("a legal mod id"),
            version(at),
            Vec::new(),
            Some("urn:ravel:test:saved-state:1".to_owned()),
            payload,
        )
    }

    /// The plan a world at `at` resolves to for this mod.
    fn plan_from(saved: &SavedDataMod, at: Option<u32>) -> SavedDataPlan {
        SavedDataPlan::open(
            &saved.declaration().expect("valid declaration"),
            at.map(version),
        )
        .expect("this build reads every version these tests name")
    }

    /// What a world at `raw` holds, as [`SavedDataMod::open`] takes it.
    fn at(raw: u32, state: SavedState) -> (SavedDataVersion, SavedState) {
        (version(raw), state)
    }

    #[test]
    fn the_mod_is_packaged_under_the_identity_it_reports() {
        let saved = SavedDataMod::new();

        assert_eq!(
            saved.id(),
            &MOD_ID.parse::<ModId>().expect("a legal mod id")
        );
        assert_eq!(SavedDataMod::default(), saved);
        assert_eq!(saved.entered(), []);
        assert_eq!(saved.state(), &SavedState::default());
    }

    #[test]
    fn the_declaration_holds_every_version_this_build_reads_and_writes_the_highest() {
        let saved = SavedDataMod::new();
        let declaration = saved.declaration().expect("valid declaration");

        assert_eq!(declaration.owner(), saved.id());
        assert_eq!(
            declaration.versions().collect::<Vec<_>>(),
            VERSIONS.map(version)
        );
        assert_eq!(saved.current(), version(3));
    }

    #[test]
    fn a_world_already_at_the_written_version_opens_with_no_step_and_keeps_its_state() {
        let mut saved = SavedDataMod::new();
        let held = SavedState {
            visits: 4,
            waypoints: vec!["mine".to_owned(), "spawn".to_owned()],
        };

        let plan = saved
            .open(Some(at(3, held.clone())))
            .expect("the current version always opens");

        assert!(plan.is_current());
        assert_eq!(saved.state(), &held);
    }

    #[test]
    fn an_older_world_is_stepped_through_every_version_between() {
        let mut saved = SavedDataMod::new();

        let plan = saved
            .open(Some(at(
                1,
                SavedState {
                    visits: 4,
                    waypoints: Vec::new(),
                },
            )))
            .expect("this build reads version 1");

        assert_eq!(plan.steps(), [version(2), version(3)]);
        assert_eq!(
            saved.state(),
            &SavedState {
                visits: 4,
                waypoints: vec!["spawn".to_owned()],
            }
        );
    }

    #[test]
    fn the_step_to_version_three_puts_waypoints_in_a_canonical_order() {
        // Reached through `open` rather than by calling `step` directly, so what
        // is tested is a migration the plan asked for.
        let mut saved = SavedDataMod::new();
        saved
            .open(Some(at(
                2,
                SavedState {
                    visits: 1,
                    waypoints: vec!["mine".to_owned(), "spawn".to_owned(), "mine".to_owned()],
                },
            )))
            .expect("this build reads version 2");

        assert_eq!(saved.state().waypoints, ["mine", "spawn"]);
    }

    #[test]
    fn a_world_holding_none_of_this_mods_data_initializes_fresh_state() {
        let mut saved = SavedDataMod::new();
        saved.visit("spawn");

        let plan = saved.open(None).expect("absence is a fresh world");

        assert!(plan.is_current());
        assert_eq!(saved.state(), &SavedState::default());
    }

    #[test]
    fn a_refused_world_leaves_the_mod_holding_what_it_held_before() {
        let mut saved = SavedDataMod::new();
        saved
            .open(Some(at(
                3,
                SavedState {
                    visits: 7,
                    waypoints: vec!["spawn".to_owned()],
                },
            )))
            .expect("the current version opens");
        let before = saved.state().clone();

        // A world from a build that writes a version this one has never seen.
        saved
            .open(Some(at(9, SavedState::default())))
            .expect_err("newer");
        assert_eq!(saved.state(), &before);
    }

    /// The delivery refusal this mod's own open step decides.
    ///
    /// Unreachable from the production crossing and asserted here anyway: the
    /// host admits the whole stored value before delivering any envelope, so a
    /// version this build cannot read is refused by `ravel-save` before this
    /// method is called. That makes this the defensive half — if the two ever
    /// stop agreeing, the mod is what refuses, and the refusal has to leave as an
    /// `Err` rather than as a mod holding whatever it held.
    #[test]
    fn a_stored_version_this_build_cannot_read_refuses_delivery() {
        let mut saved = SavedDataMod::new();
        let current = plan_from(&saved, None);

        let refused = saved
            .open_saved_data(Some(&envelope(9, json!({ "visits": 1 }))), &current)
            .expect_err("a world from a newer build is not delivered");

        assert!(refused.to_string().contains('9'), "{refused}");
        assert_eq!(saved.state(), &SavedState::default());
    }

    /// A plan that stopped describing the envelope beside it refuses delivery.
    ///
    /// The plan is what tells the mod which migrations to apply, so a plan and an
    /// envelope that disagree would migrate a world by the wrong number of steps
    /// and write the result back as current. This mod resolves the plan again from
    /// what it was actually handed and refuses when the two differ.
    ///
    /// Unreachable from the production crossing for the same reason as the
    /// refusal above, and defensive in the same way: `stage_world` builds the plan
    /// from the very envelope it then delivers, so this branch guards a host that
    /// paired them wrongly rather than a world that presented a disagreeing plan.
    /// What kills that host-side defect is `ravel-bootstrap`'s own suite, where
    /// building every plan from `None` fails a named test.
    #[test]
    fn a_plan_that_disagrees_with_the_envelope_beside_it_refuses_delivery() {
        let mut saved = SavedDataMod::new();
        // A world already at the current version, handed the plan a version-1
        // world would have resolved to.
        let stale = plan_from(&saved, Some(1));

        let refused = saved
            .open_saved_data(Some(&envelope(3, json!({ "visits": 1 }))), &stale)
            .expect_err("a plan that does not describe this envelope is not delivered");

        assert!(
            refused.to_string().contains("changed during delivery"),
            "{refused}"
        );
    }

    /// A payload this mod cannot read refuses delivery.
    ///
    /// The only refusal here that nothing but the owning mod could decide:
    /// `SPEC.md` §14 makes the payload the mod's business, so an envelope every
    /// admission check passed can still hold a value it cannot read.
    #[test]
    fn a_payload_this_mod_cannot_read_refuses_delivery() {
        let mut saved = SavedDataMod::new();
        saved.visit("spawn");
        let before = saved.state().clone();
        let current = plan_from(&saved, None);

        saved
            .open_saved_data(Some(&envelope(3, json!({ "visits": "many" }))), &current)
            .expect_err("a payload this mod cannot read is not delivered");

        assert_eq!(
            saved.state(),
            &before,
            "a refused payload changed the state this mod was holding"
        );
    }

    #[test]
    fn the_settings_this_mod_declares_belong_to_the_side_that_owns_a_world() {
        let declaration = ConfigurationDeclaration::parse(DECLARATION)
            .expect("the packaged declaration is valid");

        assert_eq!(declaration.owner().as_str(), MOD_ID);
        for setting in declaration.settings() {
            assert!(
                setting.side.applies_to(Side::IntegratedServer),
                "`{}` is offered somewhere a world is not open",
                setting.key
            );
            for elsewhere in [Side::Client, Side::DedicatedServer] {
                assert!(
                    !setting.side.applies_to(elsewhere),
                    "{}",
                    elsewhere.as_str()
                );
            }
        }
    }

    #[test]
    fn a_world_keeps_only_as_many_waypoints_as_the_settings_in_force_allow() {
        let declaration = ConfigurationDeclaration::parse(DECLARATION)
            .expect("the packaged declaration is valid");
        let resolved = LayeredConfiguration::resolve(declaration, None, None)
            .expect("the author defaults resolve");
        let mut host = SavedDataMod::new();
        host.configure(&resolved).expect("its own settings");
        let limit = host.waypoint_limit().expect("the host stated a limit");

        for waypoint in 0..limit + 2 {
            host.visit(&format!("waypoint {waypoint:02}"));
        }

        assert_eq!(host.state().waypoints.len(), limit);
    }

    #[test]
    fn a_visit_changes_the_state_a_reload_has_to_preserve() {
        let mut saved = SavedDataMod::new();
        saved.visit("spawn");
        saved.visit("mine");
        // The same waypoint twice is one waypoint and still two visits.
        saved.visit("mine");

        let (version, state) = saved.save();
        assert_eq!(version, saved.current());
        assert_eq!(state.visits, 3);
        assert_eq!(state.waypoints, ["mine", "spawn"]);
    }

    #[test]
    fn the_loader_drives_it_like_any_other_mod() {
        let mut saved = SavedDataMod::new();
        for state in [LifecycleState::Constructed, LifecycleState::Running] {
            saved.enter(state).expect("this mod refuses no step");
        }

        assert_eq!(
            saved.entered(),
            [LifecycleState::Constructed, LifecycleState::Running]
        );
    }
}

// The value a host constructs this package's mod from.
ravel_sdk::export_mod!(SavedDataMod::new());
