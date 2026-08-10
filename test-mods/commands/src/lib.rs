#![forbid(unsafe_code)]

//! Command registration and execution test mod.

use ravel_core::{ContentId, ContentKind, LifecycleState, ModId, RegistryKey};
use ravel_host_api::{CommandInvocation, HostError, HostMod, Registrar};

/// The canonical identity this mod is packaged and validated under.
pub const MOD_ID: &str = "commands";

/// The root literals this mod claims.
///
/// Two of them, because one command proves a mod can own a name and two prove
/// it can tell them apart when the loader routes to it. Neither is a
/// subcommand: `ravel-core`'s [`ContentKind::Command`] claims the root literal
/// alone, so `warp set` and `warp list` are this mod's to parse and not the
/// loader's to own.
const ROOTS: [&str; 2] = ["warp", "waypoint"];

/// A mod that owns two commands and runs them.
///
/// `ROADMAP.md`'s M3-W2 row asks for a command test mod and for process tests
/// covering success, ordering, side and failure. This mod is the success and
/// ordering half: it claims its roots through the same registration barrier
/// M3-W1 built, and records every invocation the loader routed to it, in the
/// order it arrived. A root claimed by two mods is
/// `ravel-test-mod-failure-cases`' half, because a mod that both succeeds and
/// fails proves neither cleanly.
///
/// Like `ravel-test-mod-registries`, this names [`HostMod`] directly. The SDK
/// facade that will hide the trait is M4-W1's.
///
/// # Examples
///
/// ```
/// use ravel_test_mod_commands::CommandsMod;
///
/// let commands = CommandsMod::new();
/// assert_eq!(commands.claims().len(), 2);
/// assert_eq!(commands.claims()[0].id().to_string(), "commands:warp");
/// assert!(commands.invocations().is_empty());
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandsMod {
    id: ModId,
    invocations: Vec<CommandInvocation>,
}

impl CommandsMod {
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
            invocations: Vec::new(),
        }
    }

    /// Returns the command keys this mod claims, in claim order.
    #[must_use]
    pub fn claims(&self) -> Vec<RegistryKey> {
        ROOTS.into_iter().map(|root| self.key(root)).collect()
    }

    /// Builds the key for one root literal in this mod's own namespace.
    ///
    /// # Panics
    ///
    /// Panics if `root` is not a legal [`ContentId`] path. Every caller in this
    /// crate passes a constant from [`ROOTS`], and a test asserts they parse.
    #[must_use]
    pub fn key(&self, root: &str) -> RegistryKey {
        RegistryKey::new(
            ContentKind::Command,
            ContentId::new(self.id.clone(), root).expect("a legal content path"),
        )
    }

    /// Returns every invocation the loader routed here, in arrival order.
    #[must_use]
    pub fn invocations(&self) -> &[CommandInvocation] {
        &self.invocations
    }
}

impl Default for CommandsMod {
    fn default() -> Self {
        Self::new()
    }
}

impl HostMod for CommandsMod {
    fn id(&self) -> &ModId {
        &self.id
    }

    /// Accepts every step and does nothing with it.
    ///
    /// # Errors
    ///
    /// Never returns an error. Everything this mod does happens in
    /// [`HostMod::register`] and [`HostMod::run_command`].
    fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
        Ok(())
    }

    /// Claims both root literals.
    ///
    /// A refusal ends registration rather than skipping the root. A command a
    /// mod could not claim is one a player may still type, and carrying on would
    /// leave this mod running while another mod answers for its name.
    ///
    /// # Errors
    ///
    /// Returns [`HostError::Refused`] naming the root the loader would not give
    /// it and why.
    fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), HostError> {
        for key in self.claims() {
            registrar.claim(key).map_err(|refused| HostError::Refused {
                mod_id: self.id.clone(),
                state: LifecycleState::Constructed,
                detail: refused.to_string(),
            })?;
        }
        Ok(())
    }

    /// Records one invocation.
    ///
    /// The loader routes a command only to the mod that owns it, so this does
    /// not check the key against [`Self::claims`]. Checking would assert the
    /// barrier's decision a second time from a place that cannot see the claim
    /// table, and the two answers could disagree.
    ///
    /// # Errors
    ///
    /// Returns [`HostError::Refused`] when the invocation carries no arguments.
    /// Both of this mod's commands take a subcommand, so an empty invocation is
    /// a usage error, and reporting it is what makes M3-W2's failure case
    /// something a process test can assert on without breaking the set.
    fn run_command(&mut self, invocation: &CommandInvocation) -> Result<(), HostError> {
        if invocation.arguments.is_empty() {
            return Err(HostError::Refused {
                mod_id: self.id.clone(),
                state: LifecycleState::Running,
                detail: format!("`{}` needs a subcommand", invocation.key),
            });
        }
        self.invocations.push(invocation.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{ContentKind, LifecycleState, ModId, RegistryKey, Side};
    use ravel_host_api::{CommandInvocation, HostMod, Registrar, RegistrationRefused};

    use super::{CommandsMod, MOD_ID, ROOTS};

    /// A registrar that accepts everything, so a test sees what was asked for.
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
    }

    /// A registrar that refuses the root it was built with.
    struct Refusing {
        root: &'static str,
        claimed: Vec<RegistryKey>,
    }

    impl Registrar for Refusing {
        fn claim(&mut self, key: RegistryKey) -> Result<(), RegistrationRefused> {
            if key.id().path() == self.root {
                return Err(RegistrationRefused::Conflict {
                    key,
                    owner: "other_mod".parse().expect("a legal mod id"),
                });
            }
            self.claimed.push(key);
            Ok(())
        }

        fn claimed(&self) -> &[RegistryKey] {
            &self.claimed
        }
    }

    fn invocation(root: &str, arguments: &[&str]) -> CommandInvocation {
        CommandInvocation::new(
            CommandsMod::new().key(root),
            Side::Client,
            arguments.iter().map(|word| (*word).to_owned()).collect(),
        )
    }

    #[test]
    fn the_mod_is_packaged_under_the_identity_it_reports() {
        let commands = CommandsMod::new();
        assert_eq!(
            commands.id(),
            &MOD_ID.parse::<ModId>().expect("a legal mod id")
        );
        assert_eq!(CommandsMod::default(), commands);
        assert!(commands.invocations().is_empty());
    }

    #[test]
    fn every_claimed_root_is_a_legal_command_key_in_this_mods_own_namespace() {
        let commands = CommandsMod::new();
        let claims = commands.claims();

        assert_eq!(claims.len(), ROOTS.len());
        for key in &claims {
            assert_eq!(key.kind(), ContentKind::Command);
            assert_eq!(key.id().namespace().as_str(), MOD_ID);
            // The canonical string form is what the barrier stores, so it has
            // to survive a round trip.
            assert_eq!(
                key.to_string().parse::<RegistryKey>().expect("a legal key"),
                *key
            );
        }
    }

    #[test]
    fn an_accepted_registration_claims_every_root_in_order() {
        let mut commands = CommandsMod::new();
        let mut registrar = Accepting::default();

        commands
            .register(&mut registrar)
            .expect("nothing refuses this mod");
        assert_eq!(registrar.claimed(), CommandsMod::new().claims());
    }

    #[test]
    fn a_refused_root_stops_registration_and_names_the_command() {
        for root in ROOTS {
            let mut commands = CommandsMod::new();
            let mut registrar = Refusing {
                root,
                claimed: Vec::new(),
            };

            let error = commands
                .register(&mut registrar)
                .expect_err("one root is refused");
            let text = error.to_string();
            assert!(text.contains("other_mod"), "{text}");
            assert!(text.contains(root), "{text}");

            // Nothing after the refused root was attempted.
            let refused_at = ROOTS
                .iter()
                .position(|candidate| *candidate == root)
                .expect("the root came from the list");
            assert_eq!(registrar.claimed().len(), refused_at);
        }
    }

    #[test]
    fn every_routed_invocation_is_recorded_in_the_order_it_arrived() {
        let mut commands = CommandsMod::new();
        let first = invocation("warp", &["set", "base"]);
        let second = invocation("waypoint", &["list"]);

        commands.run_command(&first).expect("it has a subcommand");
        commands.run_command(&second).expect("it has a subcommand");

        assert_eq!(commands.invocations(), [first, second]);
    }

    #[test]
    fn a_command_with_no_subcommand_is_refused_and_not_recorded() {
        let mut commands = CommandsMod::new();
        let empty = invocation("warp", &[]);

        let error = commands.run_command(&empty).expect_err("no subcommand");
        assert!(error.to_string().contains("needs a subcommand"), "{error}");
        assert_eq!(error.state(), LifecycleState::Running);
        assert_eq!(error.mod_id().as_str(), MOD_ID);
        assert!(commands.invocations().is_empty());

        // The same root with an argument is accepted, so the refusal is about
        // the arguments and not about the command.
        commands
            .run_command(&invocation("warp", &["set"]))
            .expect("it has a subcommand now");
        assert_eq!(commands.invocations().len(), 1);
    }

    #[test]
    fn the_side_an_invocation_carries_is_recorded_rather_than_checked() {
        // Which sides this mod loads on is its manifest's business and the
        // lifecycle machine's to enforce. A mod that checked side here would be
        // a second answer that can disagree with the first.
        let mut commands = CommandsMod::new();
        let invocation = CommandInvocation::new(
            commands.key("warp"),
            Side::DedicatedServer,
            vec!["set".to_owned()],
        );

        commands
            .run_command(&invocation)
            .expect("recorded as it arrived");
        assert_eq!(commands.invocations()[0].side, Side::DedicatedServer);
    }
}

// The value a host constructs this package's mod from.
ravel_sdk::export_mod!(CommandsMod::new());
