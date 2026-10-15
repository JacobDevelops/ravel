#![forbid(unsafe_code)]

//! Lifecycle hook and event-ordering test mod.

use ravel_core::{LifecycleState, ModId};
use ravel_host_api::{GameEvent, HostError, HostMod};

/// The canonical identity this mod is packaged and validated under.
pub const MOD_ID: &str = "events";

/// A mod that records every lifecycle step and every game event it saw.
///
/// `ROADMAP.md`'s M3-W2 row asks for an event test mod and for process tests
/// covering success, ordering, side and failure. Ordering is the whole reason
/// this mod keeps two separate logs rather than one: a lifecycle step is taken
/// by the whole set at once and a game event is broadcast in resolved load
/// order, so a single interleaved log would let a test that meant to assert the
/// second accidentally assert the first.
///
/// The mod reacts to every event rather than filtering, because filtering is
/// what the loader already does. `ravel-loader`'s audience drops a mod that does
/// not load on the running side and an event that cannot happen there, so a mod
/// that checked again would be a second answer that can disagree with the first
/// — and a test asserting an empty log could no longer tell which of the two
/// kept the event out.
///
/// Like `ravel-test-mod-registries`, this names [`HostMod`] directly. The SDK
/// facade that will hide the trait is M4-W1's.
///
/// # Examples
///
/// ```
/// use ravel_core::LifecycleState;
/// use ravel_host_api::{GameEvent, HostMod};
/// use ravel_test_mod_events::EventsMod;
///
/// let mut events = EventsMod::new();
/// events.enter(LifecycleState::Running).expect("it accepts every step");
/// events.observe(GameEvent::ServerTick).expect("it accepts every event");
///
/// assert_eq!(events.steps(), [LifecycleState::Running]);
/// assert_eq!(events.observed(), [GameEvent::ServerTick]);
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct EventsMod {
    id: ModId,
    steps: Vec<LifecycleState>,
    observed: Vec<GameEvent>,
    refuses: Option<GameEvent>,
}

impl EventsMod {
    /// Builds the mod instance the loader drives.
    ///
    /// # Panics
    ///
    /// Panics if [`MOD_ID`] stops being a legal mod id, which would mean this
    /// mod can no longer be packaged under the identity its manifest declares.
    #[must_use]
    pub fn new() -> Self {
        Self {
            id: MOD_ID.parse().expect("`MOD_ID` is a legal mod id"),
            steps: Vec::new(),
            observed: Vec::new(),
            refuses: None,
        }
    }

    /// Builds an instance that refuses `event` and accepts everything else.
    ///
    /// This exists so a process test can end a broadcast part-way and assert on
    /// what the mods before and after it saw. A mod that failed unconditionally
    /// could not be driven to [`LifecycleState::Running`] to fail there.
    ///
    /// # Panics
    ///
    /// Panics for the reason [`Self::new`] does.
    #[must_use]
    pub fn refusing(event: GameEvent) -> Self {
        Self {
            refuses: Some(event),
            ..Self::new()
        }
    }

    /// Returns the lifecycle steps this mod was taken through, in order.
    #[must_use]
    pub fn steps(&self) -> &[LifecycleState] {
        &self.steps
    }

    /// Returns the events this mod saw, in the order they arrived.
    #[must_use]
    pub fn observed(&self) -> &[GameEvent] {
        &self.observed
    }

    /// Returns how many times `event` reached this mod.
    ///
    /// A tick arrives repeatedly, so a test that only asked whether an event was
    /// seen could not tell one tick from a hundred.
    #[must_use]
    pub fn count(&self, event: GameEvent) -> usize {
        self.observed
            .iter()
            .filter(|observed| **observed == event)
            .count()
    }
}

impl Default for EventsMod {
    fn default() -> Self {
        Self::new()
    }
}

impl HostMod for EventsMod {
    fn id(&self) -> &ModId {
        &self.id
    }

    /// Records the step and accepts it.
    ///
    /// # Errors
    ///
    /// Never returns an error. This mod's only refusal is on an event, because
    /// a mod that refused a lifecycle step would never reach the state in which
    /// events are delivered.
    fn enter(&mut self, state: LifecycleState) -> Result<(), HostError> {
        self.steps.push(state);
        Ok(())
    }

    /// Records the event, unless this instance was built to refuse it.
    ///
    /// # Errors
    ///
    /// Returns [`HostError::Refused`] for the event passed to [`Self::refusing`],
    /// naming it. The event is not recorded, so a test can tell a refusal apart
    /// from an event that was seen and then failed on.
    fn observe(&mut self, event: GameEvent) -> Result<(), HostError> {
        if self.refuses == Some(event) {
            return Err(HostError::Refused {
                mod_id: self.id.clone(),
                state: LifecycleState::Running,
                detail: format!("`{event}` is refused by this instance"),
            });
        }
        self.observed.push(event);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{LifecycleState, ModId};
    use ravel_host_api::{GameEvent, HostMod};

    use super::{EventsMod, MOD_ID};

    #[test]
    fn the_mod_is_packaged_under_the_identity_it_reports() {
        let events = EventsMod::new();
        assert_eq!(events.id(), &MOD_ID.parse::<ModId>().expect("a legal id"));
        assert_eq!(EventsMod::default(), events);
        assert_eq!(events.steps(), []);
        assert_eq!(events.observed(), []);
    }

    #[test]
    fn lifecycle_steps_and_game_events_are_recorded_in_separate_orders() {
        let mut events = EventsMod::new();
        events
            .enter(LifecycleState::Loaded)
            .expect("it accepts every step");
        events
            .observe(GameEvent::ServerTick)
            .expect("it accepts every event");
        events
            .enter(LifecycleState::Running)
            .expect("it accepts every step");

        // The event happened between two steps and neither log knows it.
        assert_eq!(
            events.steps(),
            [LifecycleState::Loaded, LifecycleState::Running]
        );
        assert_eq!(events.observed(), [GameEvent::ServerTick]);
    }

    #[test]
    fn every_event_reaches_this_mod_because_filtering_belongs_to_the_loader() {
        let mut events = EventsMod::new();
        for event in GameEvent::ALL {
            events.observe(event).expect("it accepts every event");
        }
        assert_eq!(events.observed(), GameEvent::ALL);
    }

    #[test]
    fn the_same_event_arriving_twice_is_counted_twice() {
        let mut events = EventsMod::new();
        for _ in 0..3 {
            events.observe(GameEvent::ServerTick).expect("accepted");
        }
        events.observe(GameEvent::ClientTick).expect("accepted");

        assert_eq!(events.count(GameEvent::ServerTick), 3);
        assert_eq!(events.count(GameEvent::ClientTick), 1);
        assert_eq!(events.count(GameEvent::ResourcesReloaded), 0);
        assert_eq!(events.observed().len(), 4);
    }

    #[test]
    fn a_refused_event_names_itself_and_is_not_recorded() {
        let mut events = EventsMod::refusing(GameEvent::ResourcesReloaded);

        events
            .observe(GameEvent::ServerTick)
            .expect("only one event is refused");
        let error = events
            .observe(GameEvent::ResourcesReloaded)
            .expect_err("this instance refuses it");

        let text = error.to_string();
        assert!(
            text.contains(GameEvent::ResourcesReloaded.as_str()),
            "{text}"
        );
        assert_eq!(error.state(), LifecycleState::Running);
        assert_eq!(error.mod_id().as_str(), MOD_ID);

        // The refused event is absent, so a test can tell a refusal apart from
        // an event that arrived and then failed.
        assert_eq!(events.observed(), [GameEvent::ServerTick]);
        assert_eq!(events.count(GameEvent::ResourcesReloaded), 0);
    }

    #[test]
    fn a_refusing_instance_still_takes_every_lifecycle_step() {
        // A mod that could not reach `Running` could never refuse an event
        // there, so the refusal has to be on the event alone.
        let mut events = EventsMod::refusing(GameEvent::ServerTick);
        for state in [LifecycleState::Loaded, LifecycleState::Running] {
            events.enter(state).expect("no step is refused");
        }
        assert_eq!(events.steps().len(), 2);
    }
}

// The value a host constructs this package's mod from.
ravel_sdk::export_mod!(EventsMod::new());
