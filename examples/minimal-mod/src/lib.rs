#![forbid(unsafe_code)]

//! A minimal mod that claims one command and counts server ticks.
//!
//! [`boundary`] checks the mod-author dependency surface.

pub mod boundary;

use ravel_sdk::content::{ContentId, ContentKind, Registrar, RegistryKey};
use ravel_sdk::events::{CommandInvocation, GameEvent};
use ravel_sdk::{Declared, LifecycleState, Mod, ModError, declare_mod};

/// A mod that greets whoever types its command and counts server ticks.
///
/// # Examples
///
/// ```
/// use ravel_example_minimal_mod::MinimalMod;
/// use ravel_sdk::events::GameEvent;
/// use ravel_sdk::{Declared, LifecycleState, Mod};
///
/// let mut minimal = MinimalMod::new();
/// assert_eq!(minimal.id().as_str(), "minimal");
///
/// minimal.enter(LifecycleState::Loaded)?;
/// minimal.enter(LifecycleState::Constructed)?;
/// minimal.observe(GameEvent::ServerTick)?;
/// minimal.observe(GameEvent::ServerTick)?;
///
/// assert_eq!(minimal.ticks(), 2);
/// assert_eq!(MinimalMod::greeting_command().to_string(), "command:minimal:greet");
/// # Ok::<(), ravel_sdk::ModError>(())
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct MinimalMod {
    ticks: u64,
    greetings: u64,
}

impl MinimalMod {
    /// Builds the mod instance the loader drives.
    #[must_use]
    pub const fn new() -> Self {
        Self {
            ticks: 0,
            greetings: 0,
        }
    }

    /// Returns the one command this mod claims.
    ///
    /// The key is built from the identity `#[declare_mod]` bound to the type, so
    /// the command cannot drift away from the mod that owns it.
    ///
    /// # Panics
    ///
    /// Panics if `greet` stops being a legal content path, which would mean this
    /// mod can no longer name the command it is written around.
    #[must_use]
    pub fn greeting_command() -> RegistryKey {
        RegistryKey::new(
            ContentKind::Command,
            ContentId::new(Self::declared_id().clone(), "greet")
                .expect("`greet` is a legal content path"),
        )
    }

    /// Returns how many server ticks this instance has seen.
    #[must_use]
    pub const fn ticks(&self) -> u64 {
        self.ticks
    }

    /// Returns how many times the claimed command has run.
    #[must_use]
    pub const fn greetings(&self) -> u64 {
        self.greetings
    }
}

#[declare_mod(id = "minimal")]
impl Mod for MinimalMod {
    /// Accepts every lifecycle step.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        let _ = state;
        Ok(())
    }

    /// Claims the one command this mod owns.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] on a conflicting name or closed window.
    fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), ModError> {
        registrar
            .claim(Self::greeting_command())
            .map_err(ModError::from_display)
    }

    /// Counts server ticks and ignores everything else.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    fn observe(&mut self, event: GameEvent) -> Result<(), ModError> {
        if matches!(event, GameEvent::ServerTick) {
            self.ticks = self.ticks.saturating_add(1);
        }
        Ok(())
    }

    /// Runs the claimed command.
    ///
    /// The loader routes an invocation only to the mod that claimed the root
    /// literal, so there is nothing to check about `invocation` before acting on
    /// it.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    fn run_command(&mut self, invocation: &CommandInvocation) -> Result<(), ModError> {
        let _ = invocation;
        self.greetings = self.greetings.saturating_add(1);
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::content::{Registrar, RegistrationRefused, RegistryKey};
    use ravel_sdk::events::{CommandInvocation, GameEvent};
    use ravel_sdk::{Declared, LifecycleState, Mod, ModError, ModId, Side};

    use super::MinimalMod;

    /// A registrar that accepts every claim, standing in for the loader's own
    /// claim table.
    #[derive(Default)]
    struct Accepting {
        claimed: Vec<RegistryKey>,
    }

    impl Registrar for Accepting {
        fn claim(&mut self, key: RegistryKey) -> Result<(), RegistrationRefused> {
            self.claimed.push(key);
            Ok(())
        }

        fn claimed(&self) -> &[RegistryKey] {
            &self.claimed
        }

        // A double holds no load plan, so it knows of no other mod.
        fn loads(&self, mod_id: &ModId) -> bool {
            let _ = mod_id;
            false
        }
    }

    /// A registrar that refuses every claim as already owned.
    struct Contested {
        owner: ModId,
    }

    impl Registrar for Contested {
        fn claim(&mut self, key: RegistryKey) -> Result<(), RegistrationRefused> {
            Err(RegistrationRefused::Conflict {
                key,
                owner: self.owner.clone(),
            })
        }

        fn claimed(&self) -> &[RegistryKey] {
            &[]
        }

        // A double holds no load plan, so it knows of no other mod.
        fn loads(&self, mod_id: &ModId) -> bool {
            let _ = mod_id;
            false
        }
    }

    #[test]
    fn the_mod_reports_the_identity_it_declared() {
        let expected: ModId = "minimal".parse().expect("`minimal` is a legal mod id");
        assert_eq!(MinimalMod::declared_id(), &expected);
        assert_eq!(MinimalMod::new().id(), &expected);
        assert_eq!(MinimalMod::new(), MinimalMod::default());
    }

    #[test]
    fn a_fresh_instance_has_seen_nothing() {
        let minimal = MinimalMod::new();
        assert_eq!(minimal.ticks(), 0);
        assert_eq!(minimal.greetings(), 0);
    }

    #[test]
    fn every_lifecycle_step_is_accepted() {
        let mut minimal = MinimalMod::new();
        for state in LifecycleState::ALL {
            minimal
                .enter(state)
                .expect("the minimal mod accepts every step");
        }
    }

    #[test]
    fn the_claimed_command_is_named_in_the_declared_namespace() {
        let key = MinimalMod::greeting_command();
        assert_eq!(key.id().namespace(), MinimalMod::declared_id());
        assert_eq!(key.to_string(), "command:minimal:greet");
    }

    #[test]
    fn registration_claims_exactly_the_one_command_the_mod_owns() {
        let mut minimal = MinimalMod::new();
        let mut registrar = Accepting::default();
        minimal
            .register(&mut registrar)
            .expect("an accepting registrar refuses nothing");
        assert_eq!(registrar.claimed(), [MinimalMod::greeting_command()]);
    }

    #[test]
    fn a_contested_command_fails_the_mod_rather_than_loading_it_uninvokable() {
        let owner: ModId = "other".parse().expect("`other` is a legal mod id");
        let mut minimal = MinimalMod::new();
        let error = minimal
            .register(&mut Contested {
                owner: owner.clone(),
            })
            .expect_err("a contested claim is fatal for this mod");

        assert!(
            matches!(&error, ModError::Refused { detail, .. } if detail.contains(owner.as_str())),
            "{error}"
        );
    }

    #[test]
    fn only_server_ticks_are_counted() {
        let mut minimal = MinimalMod::new();
        for event in GameEvent::ALL {
            minimal
                .observe(event)
                .expect("the minimal mod ignores what it does not count");
        }
        assert_eq!(
            minimal.ticks(),
            1,
            "exactly one of `GameEvent::ALL` is a server tick"
        );
    }

    #[test]
    fn running_the_claimed_command_counts_it() {
        let mut minimal = MinimalMod::new();
        let invocation = CommandInvocation::new(
            MinimalMod::greeting_command(),
            Side::IntegratedServer,
            vec!["everyone".to_owned()],
        );
        minimal
            .run_command(&invocation)
            .expect("the mod runs the command it claimed");
        minimal
            .run_command(&invocation)
            .expect("the mod runs the command it claimed");
        assert_eq!(minimal.greetings(), 2);
    }

    #[test]
    fn a_mod_that_owns_no_world_data_declares_none_and_saves_none() {
        let minimal = MinimalMod::new();
        assert!(
            minimal
                .saved_data_declaration()
                .expect("valid declaration")
                .is_none()
        );
        assert!(
            minimal
                .save_saved_data()
                .expect("a stateless mod produces no envelope")
                .is_none()
        );
    }
}

// The value a host constructs this package's mod from.
ravel_sdk::export_mod!(MinimalMod::default());
