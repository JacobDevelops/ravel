use ravel_core::{ContentBehaviour, ModId, RegistryKey};
use ravel_host_api::{Registrar, RegistrationRefused};

use crate::lifecycle::{LifecycleError, LifecycleMachine};

/// A mod's registration window, borrowing the lifecycle machine for the call.
pub struct Registration<'a> {
    machine: &'a mut LifecycleMachine,
    mod_id: ModId,
    claimed: Vec<RegistryKey>,
    unexpected: Option<LifecycleError>,
}

impl<'a> Registration<'a> {
    pub(crate) fn new(machine: &'a mut LifecycleMachine, mod_id: ModId) -> Self {
        Self {
            machine,
            mod_id,
            claimed: Vec::new(),
            unexpected: None,
        }
    }

    /// Returns the detailed loader refusal behind the mod-facing closed-window response.
    #[must_use]
    pub const fn unexpected(&self) -> Option<&LifecycleError> {
        self.unexpected.as_ref()
    }
}

impl Registrar for Registration<'_> {
    fn claim(&mut self, key: RegistryKey) -> Result<(), RegistrationRefused> {
        self.record(key, None)
    }

    fn claim_describing(
        &mut self,
        key: RegistryKey,
        behaviour: ContentBehaviour,
    ) -> Result<(), RegistrationRefused> {
        self.record(key, Some(behaviour))
    }

    fn claimed(&self) -> &[RegistryKey] {
        &self.claimed
    }

    fn loads(&self, mod_id: &ModId) -> bool {
        self.machine.loads(mod_id)
    }
}

impl Registration<'_> {
    fn record(
        &mut self,
        key: RegistryKey,
        behaviour: Option<ContentBehaviour>,
    ) -> Result<(), RegistrationRefused> {
        let identifier = key.to_string();
        match self
            .machine
            .claim_described_registry_identifier(&self.mod_id, &identifier, behaviour)
        {
            Ok(()) => {
                self.claimed.push(key);
                Ok(())
            }
            Err(LifecycleError::ConflictingRegistryClaim { owner, .. }) => {
                Err(RegistrationRefused::Conflict { key, owner })
            }
            Err(error) => {
                // Preserve detailed loader errors behind the mod-facing closed window.
                if self.unexpected.is_none()
                    && !matches!(error, LifecycleError::RegistrationBarrierClosed { .. })
                {
                    self.unexpected = Some(error);
                }
                Err(RegistrationRefused::Closed { key })
            }
        }
    }
}

impl LifecycleMachine {
    /// Opens the registration window for one mod.
    ///
    /// # Errors
    ///
    /// Returns [`LifecycleError::UnknownMod`] for an id outside the plan.
    pub fn registration_window(
        &mut self,
        mod_id: &ModId,
    ) -> Result<Registration<'_>, LifecycleError> {
        self.state(mod_id)?;
        Ok(Registration::new(self, mod_id.clone()))
    }

    /// Returns the identity that owns a typed registry entry.
    #[must_use]
    pub fn owner_of(&self, key: &RegistryKey) -> Option<&ModId> {
        self.registry_owner(&key.to_string())
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use ravel_core::{ContentKind, LifecycleState, ModId, RegistryKey, Side, SideSet};
    use ravel_host_api::{Registrar, RegistrationRefused};

    use crate::lifecycle::{LifecycleEntry, LifecycleError, LifecycleMachine, Requirement};

    fn id(text: &str) -> ModId {
        text.parse().expect("a legal mod id")
    }

    fn key(kind: ContentKind, text: &str) -> RegistryKey {
        RegistryKey::new(kind, text.parse().expect("a legal content id"))
    }

    fn entry(name: &str) -> LifecycleEntry {
        LifecycleEntry {
            mod_id: id(name),
            sides: SideSet::new([Side::Client]).expect("one side"),
            requirement: Requirement::Required,
            saved_data_versions: BTreeSet::new(),
        }
    }

    fn constructed(names: &[&str]) -> LifecycleMachine {
        let mut machine =
            LifecycleMachine::new(Side::Client, names.iter().map(|name| entry(name)).collect())
                .expect("a legal plan");
        for state in [
            LifecycleState::Validated,
            LifecycleState::Resolved,
            LifecycleState::Loaded,
            LifecycleState::Constructed,
        ] {
            machine.advance_all(state).expect("a legal step");
        }
        machine
    }

    #[test]
    fn a_mod_claims_through_the_window_and_the_machine_records_the_owner() {
        let mut machine = constructed(&["example_mod"]);
        let block = key(ContentKind::Block, "example_mod:copper");
        let item = key(ContentKind::Item, "example_mod:copper");

        let mut window = machine
            .registration_window(&id("example_mod"))
            .expect("the mod is in the plan");
        window.claim(block.clone()).expect("nobody owns it");
        window.claim(item.clone()).expect("a different registry");
        assert_eq!(window.claimed(), [block.clone(), item.clone()]);
        assert!(window.unexpected().is_none());

        assert_eq!(machine.owner_of(&block), Some(&id("example_mod")));
        assert_eq!(machine.owner_of(&item), Some(&id("example_mod")));
    }

    #[test]
    fn an_unclaimed_key_has_no_owner() {
        let machine = constructed(&["example_mod"]);
        assert_eq!(machine.owner_of(&key(ContentKind::Recipe, "a:b")), None);
    }

    #[test]
    fn the_second_mod_to_claim_an_identifier_is_told_who_holds_it() {
        let mut machine = constructed(&["first_mod", "second_mod"]);
        let contested = key(ContentKind::Block, "shared:copper");

        machine
            .registration_window(&id("first_mod"))
            .expect("in the plan")
            .claim(contested.clone())
            .expect("nobody owns it yet");

        let mut window = machine
            .registration_window(&id("second_mod"))
            .expect("in the plan");
        let refused = window.claim(contested.clone()).expect_err("already owned");

        assert_eq!(
            refused,
            RegistrationRefused::Conflict {
                key: contested.clone(),
                owner: id("first_mod"),
            }
        );
        assert_eq!(window.claimed(), []);
        assert!(window.unexpected().is_none());
        assert_eq!(machine.owner_of(&contested), Some(&id("first_mod")));
    }

    #[test]
    fn a_claim_once_the_set_has_registered_is_refused_as_closed() {
        let mut machine = constructed(&["example_mod"]);
        machine
            .advance_all(LifecycleState::Registered)
            .expect("a legal step");
        assert!(machine.registration_barrier_closed());

        let late = key(ContentKind::Item, "example_mod:copper");
        let mut window = machine
            .registration_window(&id("example_mod"))
            .expect("in the plan");
        assert_eq!(
            window.claim(late.clone()),
            Err(RegistrationRefused::Closed { key: late.clone() })
        );
        assert!(window.unexpected().is_none());
        assert_eq!(machine.owner_of(&late), None);
    }

    #[test]
    fn a_claim_from_a_mod_that_is_not_constructed_tells_the_loader_more_than_the_mod() {
        // Two mods keep the barrier open while only one of them is constructed.
        let mut machine =
            LifecycleMachine::new(Side::Client, vec![entry("early_mod"), entry("late_mod")])
                .expect("a legal plan");
        for state in [LifecycleState::Validated, LifecycleState::Resolved] {
            machine.advance_all(state).expect("a legal step");
        }

        let early = key(ContentKind::Block, "early_mod:copper");
        let mut window = machine
            .registration_window(&id("early_mod"))
            .expect("in the plan");
        assert_eq!(
            window.claim(early.clone()),
            Err(RegistrationRefused::Closed { key: early.clone() })
        );

        match window.unexpected() {
            Some(LifecycleError::RegistrationWindow { mod_id, state, .. }) => {
                assert_eq!(mod_id, &id("early_mod"));
                assert_eq!(*state, LifecycleState::Resolved);
            }
            other => panic!("expected a registration-window refusal, got {other:?}"),
        }
        assert_eq!(machine.owner_of(&early), None);
    }

    #[test]
    fn a_window_cannot_be_opened_for_a_mod_outside_the_plan() {
        let mut machine = constructed(&["example_mod"]);
        assert!(matches!(
            machine.registration_window(&id("absent_mod")),
            Err(LifecycleError::UnknownMod { .. })
        ));
    }

    #[test]
    fn only_the_first_unexpected_refusal_is_kept() {
        let mut machine =
            LifecycleMachine::new(Side::Client, vec![entry("early_mod")]).expect("a legal plan");
        machine
            .advance_all(LifecycleState::Validated)
            .expect("a legal step");

        let mut window = machine
            .registration_window(&id("early_mod"))
            .expect("in the plan");
        for path in ["early_mod:first", "early_mod:second"] {
            window
                .claim(key(ContentKind::Block, path))
                .expect_err("the window is not open");
        }

        match window.unexpected() {
            Some(LifecycleError::RegistrationWindow { state, .. }) => {
                assert_eq!(*state, LifecycleState::Validated);
            }
            other => panic!("expected the first refusal to be kept, got {other:?}"),
        }
    }
}
