use std::fmt;
use std::ops::DerefMut;

use ravel_core::ModId;
use ravel_host_api::HostMod;

use crate::lifecycle::LifecycleMachine;

/// A complete borrowed lifecycle and host set, without adapting or allocating host storage.
///
/// Every entry loading on the machine's side must have a host, including failed entries.
/// Side-incompatible (parked) entries need no host. Extra hosts are retained, but do not
/// become lifecycle members. Construction checks completeness in lifecycle order before
/// any loader operation can invoke callbacks or mutate lifecycle state.
///
/// Slots may be owned boxes or mutable host references. Service ordering rearranges the
/// actual supplied slots in either case, rather than a temporary collection of references.
pub struct LoadedMods<'a, H: DerefMut<Target: HostMod>> {
    machine: &'a mut LifecycleMachine,
    hosts: &'a mut [H],
}

impl<H: DerefMut<Target: HostMod>> fmt::Debug for LoadedMods<'_, H> {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("LoadedMods")
            .field("side", &self.machine.side())
            .field("hosts", &self.hosts.len())
            .finish_non_exhaustive()
    }
}

impl<'a, H: DerefMut<Target: HostMod>> LoadedMods<'a, H> {
    /// Borrows a complete set, refusing the first missing side-compatible lifecycle owner.
    ///
    /// # Errors
    ///
    /// Returns [`MissingHost`] for the first loading lifecycle member without a host,
    /// retaining that member's current lifecycle state for diagnostics.
    pub fn new(machine: &'a mut LifecycleMachine, hosts: &'a mut [H]) -> Result<Self, MissingHost> {
        for entry in machine.entries() {
            if entry.sides.loads_on(machine.side())
                && !hosts.iter().any(|host| host.id() == &entry.mod_id)
            {
                return Err(MissingHost {
                    mod_id: entry.mod_id.clone(),
                    state: machine
                        .state(&entry.mod_id)
                        .expect("entry belongs to this machine"),
                });
            }
        }
        Ok(Self::admitted(machine, hosts))
    }

    /// Borrows storage whose completeness the admitting caller has already established.
    pub(crate) const fn admitted(machine: &'a mut LifecycleMachine, hosts: &'a mut [H]) -> Self {
        Self { machine, hosts }
    }

    /// Returns the lifecycle state paired with these hosts.
    #[must_use]
    pub const fn machine(&self) -> &LifecycleMachine {
        self.machine
    }

    pub(crate) fn parts(&mut self) -> (&mut LifecycleMachine, &mut [H]) {
        (self.machine, self.hosts)
    }
}

/// Orders actual slots, leaving undeclared hosts afterward in their prior order.
pub(crate) fn arrange<'a, H: DerefMut<Target: HostMod>>(
    hosts: &mut [H],
    order: impl IntoIterator<Item = &'a ModId>,
) {
    let mut placed = 0;
    for owner in order {
        if let Some(found) = hosts[placed..].iter().position(|host| host.id() == owner) {
            hosts[placed..=placed + found].rotate_right(1);
            placed += 1;
        }
    }
}

/// A lifecycle member loading on this side has no supplied host.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MissingHost {
    mod_id: ModId,
    state: ravel_core::LifecycleState,
}

impl MissingHost {
    /// Returns the first missing owner in lifecycle order.
    #[must_use]
    pub const fn mod_id(&self) -> &ModId {
        &self.mod_id
    }
}

impl fmt::Display for MissingHost {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(formatter, "loading mod `{}` has no host", self.mod_id)
    }
}

impl std::error::Error for MissingHost {}

impl ravel_core::StructuredFailure for MissingHost {
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
    use ravel_host_api::{HostMod, ModError};

    use super::LoadedMods;
    use crate::lifecycle::LifecycleMachine;
    use crate::tests::entry;

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

    impl HostMod for Recording {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _: LifecycleState) -> Result<(), ModError> {
            self.calls += 1;
            Ok(())
        }
    }

    #[test]
    fn missing_host_refuses_the_whole_view_before_any_callback_or_transition() {
        let mut machine = LifecycleMachine::new(
            Side::Client,
            vec![
                entry("present", &[Side::Client]),
                entry("missing_first", &[Side::Client]),
                entry("missing_second", &[Side::Client]),
            ],
        )
        .expect("valid lifecycle");
        let mut host = Recording::new("present");
        let error = LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut host])
            .expect_err("a partial host set cannot be used by any operation");
        assert_eq!(error.mod_id().as_str(), "missing_first");
        assert_eq!(
            ravel_core::StructuredFailure::lifecycle_state(&error),
            LifecycleState::Discovered
        );
        assert_eq!(host.calls, 0);
        assert_eq!(machine.state(&host.id), Ok(LifecycleState::Discovered));
    }

    #[test]
    fn missing_host_retains_the_running_members_diagnostic_state() {
        let mut machine =
            LifecycleMachine::new(Side::Client, vec![entry("missing", &[Side::Client])])
                .expect("valid lifecycle");
        for state in [
            LifecycleState::Validated,
            LifecycleState::Resolved,
            LifecycleState::Loaded,
            LifecycleState::Constructed,
            LifecycleState::Registered,
            LifecycleState::Running,
        ] {
            machine.advance_all(state).expect("valid lifecycle step");
        }
        let error = LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [])
            .expect_err("running owners still need hosts");
        assert_eq!(
            ravel_core::StructuredFailure::lifecycle_state(&error),
            LifecycleState::Running
        );
    }

    #[test]
    fn parked_members_need_no_host_and_extra_hosts_are_retained() {
        let mut machine = LifecycleMachine::new(
            Side::Client,
            vec![entry("server_only", &[Side::DedicatedServer])],
        )
        .expect("valid lifecycle");
        let mut extra = Recording::new("extra");
        let extra_id = extra.id.clone();
        let mut slots: [&mut dyn HostMod; 1] = [&mut extra];
        let mut loaded =
            LoadedMods::new(&mut machine, &mut slots).expect("parked member needs no host");
        assert_eq!(
            loaded
                .parts()
                .1
                .iter()
                .map(|host| host.id())
                .collect::<Vec<_>>(),
            [&extra_id]
        );
    }

    #[test]
    fn arranging_mutates_box_slots_and_preserves_undeclared_hosts() {
        let mut machine = LifecycleMachine::new(
            Side::Client,
            vec![
                entry("consumer", &[Side::Client]),
                entry("provider", &[Side::Client]),
            ],
        )
        .expect("valid lifecycle");
        let mut slots: Vec<Box<dyn HostMod>> = vec![
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
                .map(|host| host.id().as_str())
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
        let mut slots: [&mut dyn HostMod; 4] = [
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
                .map(|host| host.id().as_str())
                .collect::<Vec<_>>(),
            ["provider", "consumer", "extra_first", "extra_second"]
        );
    }
}
