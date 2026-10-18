use std::fmt;
use std::ops::DerefMut;

use ravel_core::ModId;
use ravel_host_api::Mod;

use crate::lifecycle::LifecycleMachine;

/// Owns a lifecycle and its complete, validated mod set.
///
/// Membership cannot change after construction. Borrowing for loader operations
/// therefore neither repeats completeness checks nor reorders the stored mods.
///
/// Owned membership is not mutable through the public API:
///
/// ```compile_fail
/// fn remove_mods(loaded: &mut ravel_loader::Loaded) {
///     loaded.mods.clear();
/// }
/// ```
///
/// Nor can callers replace the validated lifecycle with a different plan:
///
/// ```compile_fail
/// fn replace_plan(loaded: &mut ravel_loader::Loaded, plan: ravel_loader::LifecycleMachine) {
///     *loaded.machine() = plan;
/// }
/// ```
pub struct Loaded {
    machine: LifecycleMachine,
    mods: Vec<Box<dyn Mod>>,
}

impl Loaded {
    /// Validates and orders the owned set once.
    ///
    /// # Errors
    /// Returns the first loading lifecycle owner without a supplied mod.
    pub fn new(
        mut machine: LifecycleMachine,
        mut mods: Vec<Box<dyn Mod>>,
    ) -> Result<Self, MissingMod> {
        LoadedMods::new(&mut machine, &mut mods)?;
        Ok(Self { machine, mods })
    }

    /// Borrows the already validated pair without allocation or validation.
    pub fn borrow(&mut self) -> LoadedMods<'_, Box<dyn Mod>> {
        LoadedMods {
            machine: &mut self.machine,
            mods: &mut self.mods,
        }
    }

    /// Reads the lifecycle without exposing mod membership.
    #[must_use]
    pub const fn machine(&self) -> &LifecycleMachine {
        &self.machine
    }

    /// Records an owner failure without changing the admitted membership.
    ///
    /// # Errors
    /// Returns the lifecycle refusal for an unknown or ended owner.
    pub fn fail(
        &mut self,
        owner: &ModId,
        failure: ravel_core::FailureAttribution,
    ) -> Result<(), crate::LifecycleError> {
        self.machine.fail(owner, failure)
    }

    /// Records an attributed callback failure.
    ///
    /// # Errors
    /// Returns the lifecycle refusal for an unknown or ended owner.
    pub fn fail_callback(
        &mut self,
        error: &ravel_host_api::HostError,
        environment: &ravel_core::FailureEnvironment,
        surface: Option<ravel_core::FailureSurface>,
    ) -> Result<crate::Requirement, crate::LifecycleError> {
        self.machine.fail_callback(error, environment, surface)
    }

    /// Claims a registry identifier for an admitted member.
    ///
    /// # Errors
    /// Returns the lifecycle's ownership or registration-barrier refusal.
    pub fn claim_registry_identifier(
        &mut self,
        owner: &ModId,
        key: &ravel_core::RegistryKey,
        behaviour: Option<ravel_data::ContentBehaviour>,
    ) -> Result<(), crate::LifecycleError> {
        self.machine
            .claim_registry_identifier(owner, key, behaviour)
    }

    /// Records the world-mutation barrier without exposing mutable membership.
    ///
    /// # Errors
    /// Returns a refusal if this lifecycle cannot mutate the world.
    pub fn record_world_mutation(&mut self) -> Result<(), crate::LifecycleError> {
        self.machine.record_world_mutation()
    }

    /// Reads the owned mods, including side-incompatible parked members.
    #[must_use]
    pub fn mods(&self) -> &[Box<dyn Mod>] {
        &self.mods
    }
}

impl fmt::Debug for Loaded {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Loaded")
            .field("side", &self.machine.side())
            .field("mods", &self.mods.len())
            .finish_non_exhaustive()
    }
}

/// A complete borrowed lifecycle and mod set, without adapting or allocating mod storage.
///
/// Every entry loading on the machine's side must have a mod, including failed entries.
/// Side-incompatible (parked) entries need no mod. Extra mods are retained, but do not
/// become lifecycle members. Construction checks completeness in lifecycle order before
/// any loader operation can invoke callbacks or mutate lifecycle state.
///
/// Slots may be owned boxes or mutable mod references. Construction orders the actual
/// supplied slots by resolved lifecycle order, leaving extra mods afterward in prior order.
/// Service callbacks may temporarily rearrange slots, then restore this lifecycle order.
pub struct LoadedMods<'a, H: DerefMut<Target: Mod>> {
    machine: &'a mut LifecycleMachine,
    mods: &'a mut [H],
}

impl<H: DerefMut<Target: Mod>> fmt::Debug for LoadedMods<'_, H> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LoadedMods")
            .field("side", &self.machine.side())
            .field("mods", &self.mods.len())
            .finish_non_exhaustive()
    }
}

impl<'a, H: DerefMut<Target: Mod>> LoadedMods<'a, H> {
    /// Borrows and orders a complete set, refusing the first missing loading lifecycle owner.
    ///
    /// # Errors
    ///
    /// Returns [`MissingMod`] for the first loading lifecycle member without a mod,
    /// retaining that member's current lifecycle state for diagnostics.
    pub fn new(machine: &'a mut LifecycleMachine, mods: &'a mut [H]) -> Result<Self, MissingMod> {
        for (entry, state) in machine.tracked_entries() {
            if entry.sides.loads_on(machine.side())
                && !mods.iter().any(|instance| instance.id() == &entry.mod_id)
            {
                return Err(MissingMod {
                    mod_id: entry.mod_id.clone(),
                    state,
                });
            }
        }
        arrange(mods, machine.order());
        Ok(Self { machine, mods })
    }

    /// Returns the lifecycle state paired with these mods.
    #[must_use]
    pub const fn machine(&self) -> &LifecycleMachine {
        self.machine
    }

    pub(crate) fn parts(&mut self) -> (&mut LifecycleMachine, &mut [H]) {
        (self.machine, self.mods)
    }
}

/// Orders actual slots, leaving undeclared mods afterward in their prior order.
pub(crate) fn arrange<'a, H: DerefMut<Target: Mod>>(
    mods: &mut [H],
    order: impl IntoIterator<Item = &'a ModId>,
) {
    let mut placed = 0;
    for owner in order {
        if let Some(found) = mods[placed..]
            .iter()
            .position(|instance| instance.id() == owner)
        {
            mods[placed..=placed + found].rotate_right(1);
            placed += 1;
        }
    }
}

/// Looks up a member of a complete paired set.
pub(crate) fn instance<'a, H: DerefMut<Target: Mod>>(
    mods: &'a [H],
    owner: &ModId,
) -> &'a H::Target {
    mods.iter()
        .find(|instance| instance.id() == owner)
        .expect("LoadedMods checked every loading owner")
}

/// Looks up a mutable member of a complete paired set.
pub(crate) fn instance_mut<'a, H: DerefMut<Target: Mod>>(
    mods: &'a mut [H],
    owner: &ModId,
) -> &'a mut H::Target {
    mods.iter_mut()
        .find(|instance| instance.id() == owner)
        .expect("LoadedMods checked every loading owner")
}

/// A lifecycle member loading on this side has no supplied mod.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MissingMod {
    mod_id: ModId,
    state: ravel_core::LifecycleState,
}

impl MissingMod {
    /// Returns the first missing owner in lifecycle order.
    #[must_use]
    pub const fn mod_id(&self) -> &ModId {
        &self.mod_id
    }
}

impl fmt::Display for MissingMod {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "loading mod `{}` was not supplied", self.mod_id)
    }
}

impl std::error::Error for MissingMod {}

impl ravel_core::StructuredFailure for MissingMod {
    fn failure_kind(&self) -> Option<ravel_core::FailureKind> {
        None
    }

    fn lifecycle_state(&self) -> ravel_core::LifecycleState {
        self.state
    }

    fn quotes_untrusted_text(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{LifecycleState, ModId, Side};
    use ravel_host_api::{Mod, ModError};

    use super::{Loaded, LoadedMods};
    use crate::lifecycle::LifecycleMachine;
    use crate::tests::{advance_through, entry};

    struct Recording {
        id: ModId,
        calls: usize,
    }

    impl Recording {
        fn new(name: &str) -> Self {
            Self {
                id: name.parse().expect("valid id"),
                calls: 0,
            }
        }
    }

    impl Mod for Recording {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _: LifecycleState) -> Result<(), ModError> {
            self.calls += 1;
            Ok(())
        }
    }

    #[test]
    fn owned_pair_refuses_incomplete_membership_at_construction() {
        let machine = LifecycleMachine::new(
            Side::Client,
            vec![
                entry("present", &[Side::Client]),
                entry("missing", &[Side::Client]),
            ],
        )
        .expect("valid lifecycle");
        let mods: Vec<Box<dyn Mod>> = vec![Box::new(Recording::new("present"))];
        let error = Loaded::new(machine, mods).expect_err("missing members cannot be owned");
        assert_eq!(error.mod_id().as_str(), "missing");
    }

    #[test]
    fn owned_pair_reuses_validated_lifecycle_order_across_infallible_borrows() {
        let machine = LifecycleMachine::new(
            Side::Client,
            vec![
                entry("provider", &[Side::Client]),
                entry("consumer", &[Side::Client]),
            ],
        )
        .expect("valid lifecycle");
        let mods: Vec<Box<dyn Mod>> = vec![
            Box::new(Recording::new("consumer")),
            Box::new(Recording::new("provider")),
        ];
        let mut loaded = Loaded::new(machine, mods).expect("complete owned set");
        let storage = loaded.mods().as_ptr();
        for _ in 0..3 {
            let mut borrowed = loaded.borrow();
            let (_, mods) = borrowed.parts();
            assert_eq!(mods.as_ptr(), storage);
            assert_eq!(mods[0].id().as_str(), "provider");
            assert_eq!(mods[1].id().as_str(), "consumer");
        }
    }

    #[test]
    fn missing_mod_refuses_the_whole_view_before_any_callback_or_transition() {
        let mut machine = LifecycleMachine::new(
            Side::Client,
            vec![
                entry("present", &[Side::Client]),
                entry("missing_first", &[Side::Client]),
                entry("missing_second", &[Side::Client]),
            ],
        )
        .expect("valid lifecycle");
        let mut instance = Recording::new("present");
        let error = LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut instance])
            .expect_err("a partial mod set cannot be used by any operation");
        assert_eq!(error.mod_id().as_str(), "missing_first");
        assert_eq!(
            ravel_core::StructuredFailure::lifecycle_state(&error),
            LifecycleState::Discovered
        );
        assert_eq!(instance.calls, 0);
        assert_eq!(machine.state(&instance.id), Ok(LifecycleState::Discovered));
    }

    #[test]
    fn a_different_identity_cannot_supply_the_declared_owner() {
        let mut machine =
            LifecycleMachine::new(Side::Client, vec![entry("example_mod", &[Side::Client])])
                .expect("valid lifecycle");
        let mut other = Recording::new("other_mod");
        let error = LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut other])
            .expect_err("a different identity cannot supply the declared owner");
        assert_eq!(error.mod_id().as_str(), "example_mod");
        assert_ne!(error.mod_id(), &other.id);
        assert_eq!(other.calls, 0);
        assert_eq!(
            machine.state(error.mod_id()),
            Ok(LifecycleState::Discovered)
        );
    }

    #[test]
    fn missing_mod_retains_the_running_members_diagnostic_state() {
        let mut machine =
            LifecycleMachine::new(Side::Client, vec![entry("missing", &[Side::Client])])
                .expect("valid lifecycle");
        advance_through(&mut machine, LifecycleState::Running);
        let error = LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [])
            .expect_err("running owners still need mods");
        assert_eq!(
            ravel_core::StructuredFailure::lifecycle_state(&error),
            LifecycleState::Running
        );
    }

    #[test]
    fn parked_members_need_no_mod_and_extra_mods_are_retained() {
        let mut machine = LifecycleMachine::new(
            Side::Client,
            vec![entry("server_only", &[Side::DedicatedServer])],
        )
        .expect("valid lifecycle");
        let mut extra = Recording::new("extra");
        let extra_id = extra.id.clone();
        let mut slots: [&mut dyn Mod; 1] = [&mut extra];
        let mut loaded =
            LoadedMods::new(&mut machine, &mut slots).expect("parked member needs no mod");
        assert_eq!(
            loaded
                .parts()
                .1
                .iter()
                .map(|instance| instance.id())
                .collect::<Vec<_>>(),
            [&extra_id]
        );
    }

    #[test]
    fn construction_orders_slots_by_lifecycle_not_identity_or_insertion_order() {
        let mut machine = LifecycleMachine::new(
            Side::Client,
            vec![
                entry("provider", &[Side::Client]),
                entry("consumer", &[Side::Client]),
            ],
        )
        .expect("valid lifecycle");
        let mut slots: Vec<Box<dyn Mod>> = vec![
            Box::new(Recording::new("extra_first")),
            Box::new(Recording::new("consumer")),
            Box::new(Recording::new("extra_second")),
            Box::new(Recording::new("provider")),
        ];
        let mut loaded = LoadedMods::new(&mut machine, &mut slots).expect("complete set");
        assert_eq!(
            loaded
                .parts()
                .1
                .iter()
                .map(|instance| instance.id().as_str())
                .collect::<Vec<_>>(),
            ["provider", "consumer", "extra_first", "extra_second"]
        );
    }

    #[test]
    fn arranging_mutates_box_slots_and_preserves_undeclared_mods() {
        let mut machine = LifecycleMachine::new(
            Side::Client,
            vec![
                entry("consumer", &[Side::Client]),
                entry("provider", &[Side::Client]),
            ],
        )
        .expect("valid lifecycle");
        let mut slots: Vec<Box<dyn Mod>> = vec![
            Box::new(Recording::new("extra_first")),
            Box::new(Recording::new("extra_second")),
            Box::new(Recording::new("consumer")),
            Box::new(Recording::new("provider")),
        ];
        let mut loaded = LoadedMods::new(&mut machine, &mut slots).expect("complete set");
        super::arrange(
            loaded.parts().1,
            &[
                "provider".parse().expect("valid id"),
                "consumer".parse().expect("valid id"),
            ],
        );
        assert_eq!(
            slots
                .iter()
                .map(|instance| instance.id().as_str())
                .collect::<Vec<_>>(),
            ["provider", "consumer", "extra_first", "extra_second"]
        );
    }

    #[test]
    fn arranging_mutates_reference_slots() {
        let mut machine = LifecycleMachine::new(
            Side::Client,
            vec![
                entry("consumer", &[Side::Client]),
                entry("provider", &[Side::Client]),
            ],
        )
        .expect("valid lifecycle");
        let mut consumer = Recording::new("consumer");
        let mut provider = Recording::new("provider");
        let mut extra_first = Recording::new("extra_first");
        let mut extra_second = Recording::new("extra_second");
        let order = [provider.id.clone(), consumer.id.clone()];
        let mut slots: [&mut dyn Mod; 4] = [
            &mut extra_first,
            &mut extra_second,
            &mut consumer,
            &mut provider,
        ];
        let mut loaded = LoadedMods::new(&mut machine, &mut slots).expect("complete set");
        super::arrange(loaded.parts().1, &order);
        assert_eq!(
            slots
                .iter()
                .map(|instance| instance.id().as_str())
                .collect::<Vec<_>>(),
            ["provider", "consumer", "extra_first", "extra_second"]
        );
    }
}
