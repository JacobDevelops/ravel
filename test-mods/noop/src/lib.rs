#![forbid(unsafe_code)]

//! No-op lifecycle test mod.

use ravel_core::{LifecycleState, ModId};
use ravel_host_api::{HostError, HostMod};

/// The canonical identity this mod is packaged and validated under.
///
/// It matches the `id` in `tests/fixtures/public/packages/noop/package.json`,
/// because the package is what admits the mod and the loader drives whatever
/// the resolved plan named.
pub const MOD_ID: &str = "noop";

/// A mod that does nothing except remember what it was asked to do.
///
/// The point of a no-op is that it adds no behaviour of its own to the phases
/// it is driven through, so the order the loader produces is the loader's and
/// not this mod's. It still records that order, because "the pipeline ran" and
/// "the pipeline ran the mod" are different claims and only the second one is
/// what `ROADMAP.md`'s M2-W6 row asks to be shown.
///
/// This is a plain Rust value the loader owns. It is not a dynamic library, a
/// guest module, or a helper process, and it exports no foreign symbol: how a
/// mod's code arrives in the process is ADG-2's question, ADG-2 is deferred to
/// M3, and `docs/architecture/dependency-rules.md` forbids selecting a mod
/// execution model before then. [`HostMod`] is written to survive that choice,
/// so the value is constructed here and nothing about the construction is a
/// distribution decision.
///
/// A mod author will not name [`HostMod`] once the SDK carries registration.
/// This mod does because M2 has no SDK surface yet, and inventing one to hide a
/// two-method trait would be building M3's facade a milestone early.
///
/// # Examples
///
/// ```
/// use ravel_core::LifecycleState;
/// use ravel_host_api::HostMod;
/// use ravel_test_mod_noop::NoopMod;
///
/// let mut noop = NoopMod::new();
/// assert_eq!(noop.id().as_str(), "noop");
///
/// noop.enter(LifecycleState::Loaded)?;
/// noop.enter(LifecycleState::Constructed)?;
/// assert_eq!(
///     noop.entered(),
///     [LifecycleState::Loaded, LifecycleState::Constructed]
/// );
/// # Ok::<(), ravel_host_api::HostError>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct NoopMod {
    id: ModId,
    entered: Vec<LifecycleState>,
}

impl NoopMod {
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
            entered: Vec::new(),
        }
    }

    /// Returns the states this instance was taken through, in order.
    #[must_use]
    pub fn entered(&self) -> &[LifecycleState] {
        &self.entered
    }
}

impl Default for NoopMod {
    fn default() -> Self {
        Self::new()
    }
}

impl HostMod for NoopMod {
    fn id(&self) -> &ModId {
        &self.id
    }

    /// Accepts every step and does nothing with it.
    ///
    /// # Errors
    ///
    /// Never returns an error. A no-op that could refuse would be exercising
    /// the refusal path rather than the successful one, and `ravel-host-api`'s
    /// own tests already cover a mod that declines.
    fn enter(&mut self, state: LifecycleState) -> Result<(), HostError> {
        self.entered.push(state);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{LifecycleState, ModId};
    use ravel_host_api::HostMod;

    use super::{MOD_ID, NoopMod};

    #[test]
    fn the_mod_is_packaged_under_the_identity_it_reports() {
        let noop = NoopMod::new();
        assert_eq!(noop.id(), &MOD_ID.parse::<ModId>().expect("a legal mod id"));
        assert_eq!(noop.id().as_str(), "noop");
        assert_eq!(NoopMod::default(), noop);
    }

    #[test]
    fn a_fresh_instance_has_been_taken_nowhere() {
        assert_eq!(NoopMod::new().entered(), []);
    }

    #[test]
    fn every_step_is_accepted_and_recorded_in_the_order_it_arrived() {
        let mut noop = NoopMod::new();
        for state in LifecycleState::ALL {
            noop.enter(state).expect("a no-op never refuses a step");
        }
        assert_eq!(noop.entered(), LifecycleState::ALL);
    }

    #[test]
    fn two_instances_driven_the_same_way_record_the_same_thing() {
        let steps = [
            LifecycleState::Loaded,
            LifecycleState::Constructed,
            LifecycleState::Registered,
        ];
        let mut first = NoopMod::new();
        let mut second = NoopMod::new();
        for state in steps {
            first.enter(state).expect("accepted");
            second.enter(state).expect("accepted");
        }
        assert_eq!(first, second);
        assert_eq!(first.entered(), steps);
    }
}

// The value a host constructs this package's mod from.
ravel_sdk::export_mod!(NoopMod::new());
