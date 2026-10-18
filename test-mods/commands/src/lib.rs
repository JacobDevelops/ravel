#![forbid(unsafe_code)]

//! Command registration and execution test mod.

use ravel_sdk::content::{
    ContentDeclaration, ContentId, ContentKind, Registrar, RegistryKey, claim_all,
};
use ravel_sdk::events::CommandInvocation;
use ravel_sdk::{Declared, LifecycleState, Mod, ModError, declare_mod};

/// The canonical identity this mod is packaged and validated under.
pub const MOD_ID: &str = "commands";

/// The command that refuses invocations without server-side identity.
pub const SERVER_PROBE: &str = "server_probe";

// Claim roots, not subcommands; the mod parses `warp set` and `warp list`.
const ROOTS: [&str; 3] = ["warp", "waypoint", SERVER_PROBE];

/// Owns command roots and records routed invocations in arrival order.
///
/// # Examples
///
/// ```
/// use ravel_test_mod_commands::CommandsMod;
///
/// let commands = CommandsMod::new();
/// assert_eq!(commands.claims().len(), 3);
/// assert_eq!(commands.claims()[0].id().to_string(), "commands:warp");
/// assert!(commands.invocations().is_empty());
/// ```
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CommandsMod {
    invocations: Vec<CommandInvocation>,
}

impl CommandsMod {
    /// Builds the mod instance the loader drives.
    #[must_use]
    pub fn new() -> Self {
        Self {
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
    /// Panics if `root` is not a legal [`ContentId`] path.
    #[must_use]
    pub fn key(&self, root: &str) -> RegistryKey {
        RegistryKey::new(
            ContentKind::Command,
            ContentId::new(Self::declared_id().clone(), root).expect("a legal content path"),
        )
    }

    /// Returns every invocation the loader routed here, in arrival order.
    #[must_use]
    pub fn invocations(&self) -> &[CommandInvocation] {
        &self.invocations
    }

    fn declaration() -> Result<ContentDeclaration, ModError> {
        let mut declaration = ContentDeclaration::new(Self::declared_id().clone());
        for root in ROOTS {
            declaration
                .entry(ContentKind::Command, root)
                .map_err(ModError::from_display)?;
        }
        Ok(declaration)
    }
}

impl Default for CommandsMod {
    fn default() -> Self {
        Self::new()
    }
}

#[declare_mod(id = "commands")]
impl Mod for CommandsMod {
    /// Accepts every step and does nothing with it.
    ///
    /// # Errors
    ///
    /// Never returns an error.
    fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
        Ok(())
    }

    /// Claims every root literal, including the server-side probe.
    ///
    /// # Errors
    ///
    /// Returns a [`ModError`] on the first rejected claim.
    // Intentionally claim-only: no declaration enters the generated resource list.
    fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), ModError> {
        claim_all(&Self::declaration()?, registrar)
    }

    /// Records one invocation.
    ///
    /// # Errors
    ///
    /// Returns a [`ModError`] when the probe runs on a client or another command
    /// has no subcommand.
    fn run_command(&mut self, invocation: &CommandInvocation) -> Result<(), ModError> {
        let server_probe = invocation.key.id().path() == SERVER_PROBE;
        if server_probe && !invocation.side.is_server() {
            return Err(ModError::refused("server_probe requires a server side"));
        }
        if !server_probe && invocation.arguments.is_empty() {
            return Err(ModError::refused(format!(
                "`{}` needs a subcommand",
                invocation.key
            )));
        }
        self.invocations.push(invocation.clone());
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::content::{ContentKind, Registrar, RegistrationRefused, RegistryKey};
    use ravel_sdk::events::CommandInvocation;
    use ravel_sdk::{Mod, ModId, Side};

    use super::{CommandsMod, MOD_ID, ROOTS, SERVER_PROBE};

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

        fn loads(&self, mod_id: &ModId) -> bool {
            let _ = mod_id;
            false
        }
    }

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

        // A double holds no load plan, so it knows of no other mod.
        fn loads(&self, mod_id: &ModId) -> bool {
            let _ = mod_id;
            false
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
        assert_eq!(commands.invocations(), []);
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
        assert_eq!(commands.invocations(), []);

        // The same root with an argument is accepted, so the refusal is about
        // the arguments and not about the command.
        commands
            .run_command(&invocation("warp", &["set"]))
            .expect("it has a subcommand now");
        assert_eq!(commands.invocations().len(), 1);
    }

    #[test]
    fn the_server_probe_refuses_clients_and_records_both_server_identities() {
        let mut commands = CommandsMod::new();
        let mut probe = invocation(SERVER_PROBE, &[]);
        let error = commands.run_command(&probe).expect_err("not a server");
        assert!(
            error.to_string().contains("requires a server side"),
            "{error}"
        );
        assert_eq!(commands.invocations(), []);

        for side in [Side::IntegratedServer, Side::DedicatedServer] {
            probe.side = side;
            commands.run_command(&probe).expect("a server identity");
            assert_eq!(commands.invocations().last(), Some(&probe));
        }
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
