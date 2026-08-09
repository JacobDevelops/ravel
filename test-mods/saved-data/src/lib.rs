#![forbid(unsafe_code)]

//! Saved-data versioning and migration test mod.

use std::num::NonZero;

use ravel_core::{LifecycleState, ModId};
use ravel_host_api::{HostError, HostMod};
use ravel_save::{
    SavedDataDeclaration, SavedDataEnvelope, SavedDataError, SavedDataPlan, SavedDataVersion,
};
use serde::{Deserialize, Serialize};

/// The canonical identity this mod is packaged and validated under.
pub const MOD_ID: &str = "saved_data";

/// Every version of its data this build can read, lowest first.
///
/// A packaged mod declares these in its manifest and the loader carries them
/// into the plan as `saved_data_versions`; this mod is driven without a package,
/// so it states them here and the world fixtures under
/// `tests/fixtures/public/worlds/` are written against the same three.
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
/// Like `ravel-test-mod-registries`, this is a plain Rust value the loader owns
/// and it names [`HostMod`] directly. The SDK facade that will hide that trait
/// is M4-W1's.
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
    id: ModId,
    entered: Vec<LifecycleState>,
    state: SavedState,
}

impl SavedDataMod {
    /// Builds the mod instance the loader drives, holding a world's worth of
    /// nothing until one is opened.
    ///
    /// # Panics
    ///
    /// Panics if [`MOD_ID`] stops being a legal mod id, which would mean this
    /// mod can no longer be packaged under the identity its manifest declares.
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: MOD_ID.parse().expect("`MOD_ID` is a legal mod id"),
            entered: Vec::new(),
            state: SavedState::default(),
        }
    }

    /// Returns what this mod declares about the data it owns.
    ///
    /// # Panics
    ///
    /// Panics if [`VERSIONS`] is empty or holds a zero, neither of which a
    /// constant array of three non-zero literals can be.
    #[must_use]
    pub fn declaration(&self) -> SavedDataDeclaration {
        SavedDataDeclaration::new(
            self.id.clone(),
            VERSIONS
                .into_iter()
                .map(|raw| NonZero::new(raw).expect("every declared version is non-zero")),
        )
        .expect("this mod declares three versions")
    }

    /// Returns the version this build writes.
    #[must_use]
    pub fn current(&self) -> SavedDataVersion {
        self.declaration().current()
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
        let declaration = self.declaration();
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

    /// Records a visit to the world, which is what a reload has to preserve.
    pub fn visit(&mut self, waypoint: &str) {
        self.state.visits = self.state.visits.saturating_add(1);
        self.state.waypoints.push(waypoint.to_owned());
        self.state.waypoints.sort();
        self.state.waypoints.dedup();
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

impl HostMod for SavedDataMod {
    fn id(&self) -> &ModId {
        &self.id
    }

    /// Records the step and does nothing else.
    ///
    /// Opening saved data is not driven from here. `SPEC.md` §14 decides it
    /// before a world opens, which is earlier than any state a mod runs in, so
    /// [`SavedDataMod::open`] is called by whatever is opening the world.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    fn enter(&mut self, state: LifecycleState) -> Result<(), HostError> {
        self.entered.push(state);
        Ok(())
    }

    fn saved_data_declaration(&self) -> Option<SavedDataDeclaration> {
        Some(self.declaration())
    }

    fn open_saved_data(
        &mut self,
        envelope: Option<&SavedDataEnvelope>,
        plan: &SavedDataPlan,
    ) -> Result<(), HostError> {
        let state = envelope
            .map(|stored| {
                serde_json::from_value::<SavedState>(stored.payload().clone())
                    .map(|state| (stored.data_version(), state))
            })
            .transpose()
            .map_err(|error| HostError::Refused {
                mod_id: self.id.clone(),
                state: LifecycleState::Registered,
                detail: error.to_string(),
            })?;
        let opened = self.open(state).map_err(|error| HostError::Refused {
            mod_id: self.id.clone(),
            state: LifecycleState::Registered,
            detail: error.to_string(),
        })?;
        if opened != *plan {
            return Err(HostError::Refused {
                mod_id: self.id.clone(),
                state: LifecycleState::Registered,
                detail: "the admitted saved-data plan changed during delivery".to_owned(),
            });
        }
        Ok(())
    }

    fn save_saved_data(&self) -> Result<Option<SavedDataEnvelope>, HostError> {
        let (version, state) = self.save();
        let payload = serde_json::to_value(state).map_err(|error| HostError::Refused {
            mod_id: self.id.clone(),
            state: LifecycleState::Running,
            detail: error.to_string(),
        })?;
        Ok(Some(SavedDataEnvelope::new(
            self.id.clone(),
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

    use ravel_core::{LifecycleState, ModId};
    use ravel_host_api::HostMod;
    use ravel_save::{SavedDataEnvelope, SavedDataPlan, SavedDataVersion};
    use serde_json::json;

    use super::{MOD_ID, SavedDataMod, SavedState, VERSIONS};

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
        SavedDataPlan::open(&saved.declaration(), at.map(version))
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
        assert!(saved.entered().is_empty());
        assert_eq!(saved.state(), &SavedState::default());
    }

    #[test]
    fn the_declaration_holds_every_version_this_build_reads_and_writes_the_highest() {
        let saved = SavedDataMod::new();
        let declaration = saved.declaration();

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
