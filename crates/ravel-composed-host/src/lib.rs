#![forbid(unsafe_code)]

//! Player-local static composition used by the M3 configuration proof.
//!
//! # Where a side declaration takes effect
//!
//! Every member below ships with the package manifest it is packaged under, and
//! the entries the lifecycle holds are [`ravel_loader::admitted`]'s answer rather
//! than this file's. So the `sides` list a mod author writes decides what starts
//! in a launched game: [`compose`] resolves a plan for the side it was handed,
//! admits the whole set against it, and hands the result to
//! [`ravel_loader::drive`], which parks a mod whose declaration does not name
//! that side before any of its code runs.
//!
//! Until M4's review found it, both entries were written here by hand and the
//! declaration was inert: a client-only mod started on a dedicated server because
//! nothing in a launch read a manifest at all. `admitted`'s only callers were
//! `ravel-testkit` and the conformance suite.
//!
//! # What this still does not do
//!
//! The manifests it *admits* are compiled in rather than discovered, which is
//! what a statically composed host is: ADG-2 makes the mod set Rust source this
//! binary was built from, so a manifest for a mod whose code is not compiled in
//! has nothing to name.
//!
//! What is no longer open is discovery itself. [`account_for_installed`] asks
//! [`ravel_loader::discover`] what the instance holds, which validates every
//! installed package's manifest and verifies its declared digests, so a launch
//! now hashes source trees it did not compile. What it cannot do is run one: a
//! discovered package has no value behind it, so the launch is refused and the
//! package named rather than silently ignored.
//!
//! The side is no longer written here. [`initialize`] asks the JVM it is running
//! inside which distribution it was started from, and a launch that cannot get an
//! answer installs nothing rather than composing a set for a guessed side;
//! `ravel_bootstrap_runtime::side` is that question and the reasons for it. What
//! it cannot answer is [`Side::IntegratedServer`], which is not a distribution but
//! a server inside a client process, so a mod declaring only that side is parked
//! on a client launch. `docs/guide/limits.md` states it.

use std::fmt;
use std::path::Path;

use jni::EnvUnowned;
use jni::objects::JClass;
use ravel_bootstrap_runtime::{
    HookError, SideError, environment, install, install_settings, pre_world_report,
    side as running_side,
};
use ravel_config::{LayerError, LayeredConfiguration, file};
use ravel_core::{ModId, Side};
use ravel_host_api::HostMod;
use ravel_loader::{
    DiscoveryRefused, DriveError, LifecycleEntry, LifecycleMachine, admitted, discover, drive,
};
use ravel_package::{CompatibilityError, ManifestError, PackageManifest};
use ravel_resolve::{ResolutionError, resolve};
use ravel_test_mod_client_only::{ClientOnlyMod, work_log};
use ravel_test_mod_configuration::ConfigurationMod;
use ravel_test_mod_saved_data::SavedDataMod;
use ravel_test_mod_server_only::{ServerOnlyMod, work_log as server_work_log};

/// The override path copied from the selected pack manifest into this generated host.
pub const PACK_CONFIGURATION_PATH: &str = "ravel-pack/configuration.json";

/// The manifest each composed member is packaged under.
///
/// Read from the package rather than restated here, so the document this host
/// admits is the same file the mod ships and there is no second copy of a `sides`
/// list to disagree with the first.
const CONFIGURATION_PACKAGE: &str = include_str!("../../../test-mods/configuration/package.json");
const SAVED_DATA_PACKAGE: &str = include_str!("../../../test-mods/saved-data/package.json");
const CLIENT_ONLY_PACKAGE: &str = include_str!("../../../test-mods/client-only/package.json");
const SERVER_ONLY_PACKAGE: &str = include_str!("../../../test-mods/server-only/package.json");

/// One member of the composed set: the package it ships as, and its value.
///
/// The pair is what makes a side declaration binding. Neither half is authority
/// on its own — the document says which sides the mod loads on, the value is the
/// code that would run there, and [`admit`] is where the first decides the second.
struct Member<'package> {
    /// The `package.json` this member ships.
    package: &'package str,
    /// The Rust value ADG-2 selected for it.
    host: Box<dyn HostMod>,
}

/// What the lifecycle is driven from: one admitted entry beside its value.
type Admitted = Vec<(LifecycleEntry, Box<dyn HostMod>)>;

/// The driven runtime pieces and resolved settings produced for one launch.
pub type Composition = (
    LifecycleMachine,
    Vec<Box<dyn HostMod>>,
    Vec<LayeredConfiguration>,
);

/// Constructs and drives this source-composed mod set for one launch.
///
/// # Errors
///
/// Returns an error when a selected-pack or player document cannot be read,
/// configuration cannot be resolved or consumed, or the lifecycle cannot reach
/// running.
pub fn compose(game_directory: &Path, side: Side) -> Result<Composition, LaunchError> {
    let mut configuration = ConfigurationMod::new();
    let declaration = ConfigurationMod::declaration();
    let pack_path = game_directory.join(PACK_CONFIGURATION_PATH);
    let pack = file::read_path(&pack_path, &declaration.owner).map_err(LaunchError::PackFile)?;
    let player = file::read(game_directory, &declaration.owner).map_err(LaunchError::PlayerFile)?;
    let resolved = LayeredConfiguration::resolve(declaration, pack.as_ref(), player.as_ref())
        .map_err(LaunchError::Layer)?;
    configuration
        .configure(&resolved)
        .map_err(LaunchError::Configuration)?;

    let members = vec![
        Member {
            package: CONFIGURATION_PACKAGE,
            host: Box::new(configuration),
        },
        // The world-owning half of the composition. Until this was here no mod
        // in a running build owned world data, so the saved-data round trip
        // could not be observed in a launch at all.
        Member {
            package: SAVED_DATA_PACKAGE,
            host: Box::new(SavedDataMod::new()),
        },
        // The client-only half. Its own module doc calls itself the mod the
        // side gate is about, and until it was composed here nothing in a
        // launch could show a declaration keeping code off a server.
        Member {
            package: CLIENT_ONLY_PACKAGE,
            host: Box::new(ClientOnlyMod::new(side, &work_log())),
        },
        // The other half of the same rule. With only the member above, a
        // launch proved a declaration keeping code off a server and nothing
        // proved the reverse, so the shipped set showed one direction of a
        // two-directional rule.
        Member {
            package: SERVER_ONLY_PACKAGE,
            host: Box::new(ServerOnlyMod::new(side, &server_work_log())),
        },
    ];

    account_for_installed(game_directory, &members)?;
    let composed = admit(side, members)?;
    let (machine, mods) = drive(side, composed).map_err(LaunchError::Drive)?;

    Ok((machine, mods, vec![resolved]))
}

/// Discovers what the instance has installed and refuses anything unrunnable.
///
/// This is [`ravel_loader::discover`]'s caller in a launch, and it is where a
/// package that was never compiled into this binary stops being invisible.
/// Discovery locates each installed package, validates its manifest against the
/// package schema, and verifies its declared source and provenance digests — so
/// a launch that finds a package now checks its integrity, which `SPEC.md` §10
/// requires and no launch did before.
///
/// What it cannot do is run one. ADG-2 makes a mod's code Rust source this
/// binary was built from, so a package installed into an instance has a manifest
/// and no value: there is nothing to construct and nothing to enter. The honest
/// answer is to refuse the launch and name it. Admitting it would order a mod
/// with no code behind it; ignoring it would start a game the player believes
/// holds a mod that is not there, which is the failure `SPEC.md` §9.2's
/// pre-world report exists to prevent.
///
/// An installed package that *is* a compiled-in member is accounted for and its
/// integrity checked, and the document [`admit`] admits stays the compiled-in
/// one: the code that will run came from that source tree, so the manifest that
/// describes it is the one packaged beside it.
///
/// # Errors
///
/// Returns [`LaunchError::Discovery`] when the instance's packages cannot be
/// established, and [`LaunchError::NotComposed`] when an installed package names
/// no compiled-in value.
fn account_for_installed(game_directory: &Path, members: &[Member<'_>]) -> Result<(), LaunchError> {
    for installed in discover(game_directory)
        .map_err(LaunchError::Discovery)?
        .packages()
    {
        let id = &installed.manifest().id;
        if !members.iter().any(|member| member.host.id() == id) {
            return Err(LaunchError::NotComposed(id.clone()));
        }
    }
    Ok(())
}

/// Turns the composed members into the entries the lifecycle is driven from.
///
/// This is the whole of the fix M4's review asked for, and every step of it is
/// somebody else's rule: `ravel-package` decides what a manifest means,
/// `ravel-resolve` decides the load order for this side, and
/// [`ravel_loader::admitted`] decides which entries the set is admitted as. What
/// belongs to this host is only the pairing — which Rust value goes with which
/// admitted entry — because the values are the ones it was compiled from.
///
/// The set is admitted whole rather than filtered: `admitted` returns an entry for
/// every installed package, including one this side parks, so a player sees a
/// client-only mod listed and marked as not loading here rather than missing. The
/// parking itself is [`ravel_loader::drive`]'s, through
/// [`LifecycleMachine`], which never enters a mod whose declaration omits the
/// running side.
///
/// # Errors
///
/// Returns [`LaunchError::Manifest`] when a shipped package document does not
/// parse, [`LaunchError::Compatibility`] when a member declares a Ravel range or a
/// Minecraft version this build is outside of, [`LaunchError::Resolution`] when
/// the set has no legal order, and [`LaunchError::Composition`] when an admitted
/// entry names no compiled-in value or a compiled-in value is named by no entry.
fn admit(side: Side, members: Vec<Member<'_>>) -> Result<Admitted, LaunchError> {
    let mut manifests = Vec::with_capacity(members.len());
    let mut values = Vec::with_capacity(members.len());
    for member in members {
        manifests.push(PackageManifest::parse(member.package).map_err(LaunchError::Manifest)?);
        values.push(member.host);
    }

    let plan = resolve(&manifests, side).map_err(LaunchError::Resolution)?;
    // The versions this build is, read from the one place that already answers
    // that question for a failure report, so a refusal and an admission cannot
    // disagree about what is running.
    let running = environment();
    let entries = admitted(&plan, &manifests, &running.ravel, &running.minecraft)
        .map_err(LaunchError::Compatibility)?;

    let mut composed = Vec::with_capacity(entries.len());
    for entry in entries {
        let at = values
            .iter()
            .position(|host| host.id() == &entry.mod_id)
            .ok_or_else(|| LaunchError::Composition(entry.mod_id.clone()))?;
        composed.push((entry, values.remove(at)));
    }
    if let Some(unclaimed) = values.first() {
        return Err(LaunchError::Composition(unclaimed.id().clone()));
    }
    Ok(composed)
}

fn initialize(env: &mut jni::Env<'_>) -> Result<(), LaunchError> {
    let game_directory = std::env::current_dir().map_err(LaunchError::CurrentDirectory)?;
    // Asked of the JVM this launch is inside rather than written here. Every rule
    // below depends on it, so a launch that cannot name its side installs nothing
    // instead of composing a set for a side it guessed.
    let side = running_side(env).map_err(LaunchError::Side)?;
    let (machine, mods, configurations) = compose(&game_directory, side)?;
    install_settings(side, configurations);
    // The report this host hands over holds no refusal, and that is a statement
    // about this composition rather than an empty gesture: every stage before
    // the machine here — reading the selected pack's document, resolving
    // configuration, driving the set — fails this function instead of recording
    // a refusal, and a failure here installs no runtime at all, which the open
    // crossing already refuses on. What the report is for is the pipeline that
    // validates packages and orders a graph, which runs when a pack is compiled
    // and not in this process, so nothing in a launch can add to it yet.
    install(machine, mods, pre_world_report()).map_err(LaunchError::Install)
}

/// Starts the player-local statically composed set before game methods run.
#[jni::jni_mangle("dev.ravel.jvm.Hook", "ravelInitialize")]
pub fn ravel_initialize<'local>(mut env: EnvUnowned<'local>, _class: JClass<'local>) {
    env.with_env(initialize)
        .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

/// Why the selected source-composed set could not start.
#[derive(Debug)]
pub enum LaunchError {
    /// The JNI frame could not establish its guarded environment.
    Jvm(jni::errors::Error),
    /// The process could not name its launch directory.
    CurrentDirectory(std::io::Error),
    /// The JVM could not say which side this process is.
    Side(SideError),
    /// The selected pack's override document could not be read.
    PackFile(file::FileError),
    /// The player's existing value document could not be read.
    PlayerFile(file::FileError),
    /// The instance's installed packages could not be established.
    Discovery(DiscoveryRefused),
    /// An installed package names no value this binary was compiled from.
    NotComposed(ModId),
    /// A composed member's shipped package manifest does not parse.
    Manifest(ManifestError),
    /// A composed member declares a Ravel or Minecraft version this build is not.
    Compatibility(CompatibilityError),
    /// The composed package set has no legal load order on this side.
    Resolution(ResolutionError),
    /// An admitted entry and the compiled-in values do not account for each other.
    Composition(ModId),
    /// The declaration and its layers could not be resolved.
    Layer(LayerError),
    /// The concrete mod could not consume its declared settings.
    Configuration(String),
    /// The lifecycle could not drive the composed value.
    Drive(DriveError),
    /// The driven runtime could not be installed.
    Install(HookError),
}

impl fmt::Display for LaunchError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Jvm(error) => write!(formatter, "the JVM refused initialization: {error}"),
            Self::CurrentDirectory(error) => {
                write!(formatter, "the launch directory cannot be read: {error}")
            }
            Self::Side(error) => write!(formatter, "the running side cannot be named: {error}"),
            Self::PackFile(error) => {
                write!(formatter, "pack configuration cannot be read: {error}")
            }
            Self::PlayerFile(error) => {
                write!(formatter, "player configuration cannot be read: {error}")
            }
            Self::Discovery(error) => write!(
                formatter,
                "the installed packages cannot be established: {error}"
            ),
            Self::NotComposed(mod_id) => write!(
                formatter,
                "`{mod_id}` is installed and this build was not compiled from it, so its code \
                 cannot run"
            ),
            Self::Manifest(error) => {
                write!(formatter, "a composed package does not parse: {error}")
            }
            Self::Compatibility(error) => write!(
                formatter,
                "a composed package was built for another version: {error}"
            ),
            Self::Resolution(error) => {
                write!(formatter, "the composed set has no load order: {error}")
            }
            Self::Composition(mod_id) => write!(
                formatter,
                "`{mod_id}` was admitted without a compiled-in value, or holds one nothing admitted"
            ),
            Self::Layer(error) => write!(formatter, "configuration cannot be resolved: {error}"),
            Self::Configuration(error) => {
                write!(
                    formatter,
                    "the configuration mod refused its declaration: {error}"
                )
            }
            Self::Drive(error) => write!(formatter, "the composed mod set cannot start: {error}"),
            Self::Install(error) => write!(
                formatter,
                "the running mod set cannot be installed: {error}"
            ),
        }
    }
}

impl From<jni::errors::Error> for LaunchError {
    fn from(error: jni::errors::Error) -> Self {
        Self::Jvm(error)
    }
}

impl std::error::Error for LaunchError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Jvm(error) => Some(error),
            Self::CurrentDirectory(error) => Some(error),
            Self::Side(error) => Some(error),
            Self::PackFile(error) | Self::PlayerFile(error) => Some(error),
            Self::Discovery(error) => Some(error),
            Self::Manifest(error) => Some(error),
            Self::Compatibility(error) => Some(error),
            Self::Resolution(error) => Some(error),
            Self::Layer(error) => Some(error),
            Self::Composition(_) | Self::NotComposed(_) | Self::Configuration(_) => None,
            Self::Drive(error) => Some(error),
            Self::Install(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex, PoisonError};

    use ravel_bootstrap_runtime::environment;
    use ravel_config::{ValueDocument, file};
    use ravel_core::{LifecycleState, ModId, Side};
    use ravel_host_api::{HostError, HostMod};
    use ravel_loader::drive;
    use serde_json::json;

    use super::{
        CLIENT_ONLY_PACKAGE, CONFIGURATION_PACKAGE, DiscoveryRefused, LaunchError, Member,
        PACK_CONFIGURATION_PATH, SAVED_DATA_PACKAGE, SERVER_ONLY_PACKAGE, admit, compose,
    };

    fn directory(name: &str) -> std::path::PathBuf {
        let directory = std::env::temp_dir().join(format!(
            "ravel-configuration-host-{}-{name}",
            std::process::id()
        ));
        let _ = std::fs::remove_dir_all(&directory);
        std::fs::create_dir_all(&directory).expect("a writable temporary directory");
        directory
    }

    fn owner() -> ModId {
        "configuration"
            .parse()
            .expect("the tracked mod id is valid")
    }

    fn saved_owner() -> ModId {
        "saved_data".parse().expect("the tracked mod id is valid")
    }

    fn client_owner() -> ModId {
        "client_only".parse().expect("the tracked mod id is valid")
    }

    fn server_owner() -> ModId {
        "server_only".parse().expect("the tracked mod id is valid")
    }

    /// A mod that records every lifecycle step it was taken into.
    ///
    /// The log is what makes "this code never ran" observable from outside. A
    /// state read off the machine says what the machine believes; an empty log
    /// says the mod's own body was never entered.
    struct Recording {
        id: ModId,
        states: Arc<Mutex<Vec<LifecycleState>>>,
    }

    impl HostMod for Recording {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, state: LifecycleState) -> Result<(), HostError> {
            self.states
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(state);
            Ok(())
        }
    }

    fn steps(log: &Arc<Mutex<Vec<LifecycleState>>>) -> Vec<LifecycleState> {
        log.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// A member whose shipped manifest declares exactly `sides`.
    ///
    /// The document is written out and parsed rather than built, so the `sides`
    /// list travels the route an author's does. The two versions come from
    /// [`environment`] rather than being spelled here: a crate under `crates/`
    /// may not name a Minecraft version, and a literal would also stop admitting
    /// this set the day either version moved.
    fn member(id: &str, sides: &[&str]) -> (String, ModId, Arc<Mutex<Vec<LifecycleState>>>) {
        let running = environment();
        let sides = sides
            .iter()
            .map(|side| format!("\"{side}\""))
            .collect::<Vec<_>>()
            .join(", ");
        let zero = "0".repeat(64);
        let document = format!(
            r#"{{
              "schemaVersion": 1,
              "id": "{id}",
              "version": "0.0.0",
              "compatibility": {{
                "ravel": ">={ravel}",
                "minecraft": ["{minecraft}"]
              }},
              "sides": [{sides}],
              "authors": [{{ "name": "Ravel" }}],
              "license": "MIT",
              "source": {{
                "repository": "https://example.invalid/r",
                "files": [{{ "path": "src/lib.rs", "sha256": "{zero}" }}]
              }},
              "provenance": {{ "record": "provenance.json", "sha256": "{zero}" }}
            }}"#,
            ravel = running.ravel,
            minecraft = running.minecraft,
        );
        let mod_id = id.parse().expect("a legal mod id");
        (document, mod_id, Arc::new(Mutex::new(Vec::new())))
    }

    /// The property this composition exists to hold: a declaration decides what runs.
    ///
    /// Three members, driven on all three sides through the same [`admit`] the
    /// shipped launch calls. A client-only mod runs on the client and nowhere
    /// else, a server-only mod runs on both server sides and not on the client,
    /// and the positive control runs everywhere — which is what keeps the first
    /// two from passing because nothing runs at all.
    #[test]
    fn a_shipped_side_declaration_decides_which_composed_mods_run() {
        let declarations = [
            ("client_side", vec!["client"]),
            ("server_side", vec!["integrated-server", "dedicated-server"]),
            (
                "every_side",
                vec!["client", "integrated-server", "dedicated-server"],
            ),
        ];

        for side in Side::ALL {
            let built: Vec<_> = declarations
                .iter()
                .map(|(id, sides)| member(id, sides))
                .collect();
            let members = built
                .iter()
                .map(|(document, mod_id, log)| Member {
                    package: document.as_str(),
                    host: Box::new(Recording {
                        id: mod_id.clone(),
                        states: Arc::clone(log),
                    }) as Box<dyn HostMod>,
                })
                .collect();

            let (machine, _) = drive(side, admit(side, members).expect("a compatible set"))
                .expect("the composed set should start");

            let expected = [side == Side::Client, side.is_server(), true];
            for (((_, mod_id, log), (name, _)), runs) in
                built.iter().zip(&declarations).zip(expected)
            {
                assert_eq!(
                    machine.state(mod_id) == Ok(LifecycleState::Running),
                    runs,
                    "`{name}` on `{}`",
                    side.name()
                );
                assert_eq!(
                    !steps(log).is_empty(),
                    runs,
                    "`{name}` ran mod code on `{}`",
                    side.name()
                );
            }
        }
    }

    /// The same property with the real composition rather than a written one.
    ///
    /// `client_only` ships `"sides": ["client"]`, so a dedicated server admits it
    /// — a player who installed it sees it listed — and never starts it.
    #[test]
    fn the_client_only_member_is_admitted_and_parked_on_a_dedicated_server() {
        let game = directory("dedicated");
        let (machine, mods, _) =
            compose(&game, Side::DedicatedServer).expect("the tracked composition should start");

        assert_eq!(
            machine.state(&client_owner()),
            Ok(LifecycleState::Resolved),
            "a client-only mod started on a dedicated server"
        );
        assert_eq!(machine.state(&owner()), Ok(LifecycleState::Running));
        assert_eq!(machine.state(&saved_owner()), Ok(LifecycleState::Running));
        assert_eq!(
            machine.state(&server_owner()),
            Ok(LifecycleState::Running),
            "a server-only mod was kept off the side it declares"
        );
        assert!(
            mods.iter().any(|held| held.id() == &client_owner()),
            "a parked mod is installed rather than absent"
        );
    }

    /// The other direction of the same rule, through the same shipped path.
    ///
    /// `server_only` ships no `"client"` in its `sides`, so a client admits it —
    /// a player sees it listed — and never starts it. Without this the shipped
    /// composition proved one direction of a two-directional rule.
    #[test]
    fn the_server_only_member_is_admitted_and_parked_on_the_client() {
        let game = directory("client");
        let (machine, mods, _) =
            compose(&game, Side::Client).expect("the tracked composition should start");

        assert_eq!(
            machine.state(&server_owner()),
            Ok(LifecycleState::Resolved),
            "a server-only mod started on the client"
        );
        assert_eq!(machine.state(&client_owner()), Ok(LifecycleState::Running));
        assert_eq!(machine.state(&owner()), Ok(LifecycleState::Running));
        assert!(
            mods.iter().any(|held| held.id() == &server_owner()),
            "a parked mod is installed rather than absent"
        );
    }

    /// Every side, and on each one the pair disagrees about who runs.
    ///
    /// The two members above are one test each; this is what makes them a rule.
    /// On the client exactly one of the pair runs and on each server side the
    /// other does, so a composition that started everything everywhere — or
    /// nothing anywhere — fails here rather than passing two half-tests.
    #[test]
    fn the_shipped_pair_runs_on_opposite_sides_of_every_launch() {
        for side in Side::ALL {
            let game = directory(&format!("pair-{}", side.name()));
            let (machine, _, _) =
                compose(&game, side).expect("the tracked composition should start");

            assert_eq!(
                machine.state(&client_owner()) == Ok(LifecycleState::Running),
                side == Side::Client,
                "client_only on `{}`",
                side.name()
            );
            assert_eq!(
                machine.state(&server_owner()) == Ok(LifecycleState::Running),
                side.is_server(),
                "server_only on `{}`",
                side.name()
            );
        }
    }

    #[test]
    fn the_selected_composition_drives_every_mod_its_manifests_admit() {
        let game = directory("defaults");
        let (machine, mods, configurations) =
            compose(&game, Side::Client).expect("the tracked composition should start");

        for id in [owner(), saved_owner(), client_owner()] {
            assert_eq!(machine.state(&id), Ok(LifecycleState::Running), "{id}");
        }
        assert_eq!(
            machine.state(&server_owner()),
            Ok(LifecycleState::Resolved),
            "the server-only member is parked on the client"
        );

        let driven: Vec<&ModId> = mods.iter().map(|held| held.id()).collect();
        assert_eq!(driven.len(), 4);
        for id in [owner(), saved_owner(), client_owner(), server_owner()] {
            assert!(driven.contains(&&id), "{id}");
        }

        // Only the configuration mod declares settings, so the resolved
        // configurations stay one however many mods the composition holds.
        assert_eq!(configurations.len(), 1);
        assert_eq!(configurations[0].owner(), &owner());
    }

    /// The manifests are the authority, so nothing may quietly disagree with them.
    #[test]
    fn every_shipped_manifest_names_the_value_and_the_versions_it_is_composed_with() {
        let manifests = [
            (CONFIGURATION_PACKAGE, owner()),
            (SAVED_DATA_PACKAGE, saved_owner()),
            (CLIENT_ONLY_PACKAGE, client_owner()),
            (SERVER_ONLY_PACKAGE, server_owner()),
        ];
        let running = environment();
        for (document, expected) in manifests {
            let manifest =
                ravel_package::PackageManifest::parse(document).expect("a shipped manifest parses");
            assert_eq!(manifest.id, expected);
            manifest
                .compatibility
                .check_ravel(&running.ravel)
                .unwrap_or_else(|refused| panic!("{expected}: {refused}"));
            manifest
                .compatibility
                .check_minecraft(&running.minecraft)
                .unwrap_or_else(|refused| panic!("{expected}: {refused}"));
        }

        let saved = ravel_test_mod_saved_data::SavedDataMod::new();
        let declared: std::collections::BTreeSet<_> = saved.declaration().versions().collect();
        let manifest = ravel_package::PackageManifest::parse(SAVED_DATA_PACKAGE)
            .expect("a shipped manifest parses");
        assert_eq!(
            manifest
                .saved_data_versions
                .iter()
                .copied()
                .collect::<std::collections::BTreeSet<_>>(),
            declared,
            "the manifest and the mod disagree about the versions it can read"
        );
    }

    /// The bytes an installed fixture carries, and the digests they hash to.
    ///
    /// The digests came from `sha256sum` rather than from the library the code
    /// under test hashes with, so the fixture and the check do not agree by
    /// construction. `OTHER_DIGEST` is the digest of bytes no fixture writes.
    const INSTALLED_SOURCE: &[u8] = b"pub fn load() {}\n";
    const INSTALLED_RECORD: &[u8] = br#"{"origin":"example"}"#;
    const SOURCE_DIGEST: &str = "d43fb0c057531a1578825aa660a5cc1b415a10ae8eb1b42994ca7ceda27d6706";
    const RECORD_DIGEST: &str = "5cf5e6735c38fa43af69f9fe2210ca13d9fd75cc98cd9650ee969c0498a28ab0";
    const OTHER_DIGEST: &str = "a126bbfe4662a8d151f8e62afbb278a4a273e01fe0a88fc1545b25d017f19fb3";

    /// Installs a package into an instance the way a player would.
    fn install(game: &std::path::Path, id: &str, source_digest: &str) {
        let running = environment();
        let root = game
            .join(ravel_loader::PACKAGES_DIRECTORY)
            .join(format!("{id}-1.0.0"));
        std::fs::create_dir_all(root.join("src")).expect("a package directory is creatable");
        std::fs::write(
            root.join(ravel_loader::MANIFEST_FILE),
            format!(
                r#"{{
                  "schemaVersion": 1,
                  "id": "{id}",
                  "version": "1.0.0",
                  "compatibility": {{
                    "ravel": ">={ravel}",
                    "minecraft": ["{minecraft}"]
                  }},
                  "sides": ["client"],
                  "authors": [{{ "name": "Ravel" }}],
                  "license": "MIT",
                  "source": {{
                    "repository": "https://example.invalid/{id}",
                    "files": [{{ "path": "src/lib.rs", "sha256": "{source_digest}" }}]
                  }},
                  "provenance": {{
                    "record": "provenance.json",
                    "sha256": "{RECORD_DIGEST}"
                  }}
                }}"#,
                ravel = running.ravel,
                minecraft = running.minecraft,
            ),
        )
        .expect("manifest written");
        std::fs::write(root.join("src/lib.rs"), INSTALLED_SOURCE).expect("source written");
        std::fs::write(root.join("provenance.json"), INSTALLED_RECORD).expect("record written");
    }

    /// A launch now finds what an instance holds, and says so when it cannot run it.
    #[test]
    fn a_package_installed_into_the_instance_that_this_build_cannot_run_refuses_the_launch() {
        let game = directory("installed-stranger");
        install(&game, "stranger_mod", SOURCE_DIGEST);

        let Err(refused) = compose(&game, Side::Client) else {
            panic!("a package no value was compiled from should refuse the launch");
        };
        let LaunchError::NotComposed(mod_id) = &refused else {
            panic!("the refusal should name the installed package: {refused:?}");
        };
        assert_eq!(mod_id.as_str(), "stranger_mod");
        assert!(refused.to_string().contains("stranger_mod"));
    }

    /// The integrity check `docs/guide/limits.md` recorded as absent from a launch.
    #[test]
    fn an_installed_package_whose_source_is_not_what_it_declares_refuses_the_launch() {
        let game = directory("installed-altered");
        install(&game, "client_only", OTHER_DIGEST);

        let Err(refused) = compose(&game, Side::Client) else {
            panic!("a package whose source was altered should refuse the launch");
        };
        assert!(
            matches!(
                refused,
                LaunchError::Discovery(DiscoveryRefused::Integrity { .. })
            ),
            "{refused:?}"
        );
    }

    /// And an intact package this build *was* compiled from still starts the set.
    #[test]
    fn an_installed_package_this_build_was_compiled_from_launches() {
        let game = directory("installed-known");
        install(&game, "client_only", SOURCE_DIGEST);

        let (machine, _, _) = compose(&game, Side::Client).expect("the composition should start");
        assert_eq!(machine.state(&client_owner()), Ok(LifecycleState::Running));
    }

    #[test]
    fn the_world_owning_mod_carries_the_versions_it_declares() {
        let game = directory("saved-versions");
        let (machine, _, _) =
            compose(&game, Side::Client).expect("the tracked composition should start");

        assert!(
            machine.order().any(|id| id == &saved_owner()),
            "the world-owning mod is part of the driven load order"
        );
    }

    #[test]
    fn the_selected_pack_file_applies_per_setting_policies() {
        let game = directory("pack-values");
        let pack = ValueDocument::parse(
            &json!({
                "schemaVersion": 1,
                "owner": "configuration",
                "values": {
                    "client.enabled": false,
                    "integrated.mob_limit": 40,
                    "dedicated.motd": "Pack",
                    "common.profile": "verbose"
                }
            })
            .to_string(),
        )
        .expect("a valid pack document");
        let player = ValueDocument::parse(
            &json!({
                "schemaVersion": 1,
                "owner": "configuration",
                "values": {
                    "client.enabled": true,
                    "integrated.mob_limit": 10,
                    "dedicated.motd": "Player",
                    "common.profile": "quiet"
                }
            })
            .to_string(),
        )
        .expect("a valid player document");
        let pack_path = game.join(PACK_CONFIGURATION_PATH);
        std::fs::create_dir_all(pack_path.parent().expect("the path has a parent"))
            .expect("the pack directory should be writable");
        std::fs::write(&pack_path, pack.to_json()).expect("the pack file should be writable");
        file::write(&game, &player).expect("the player file should be writable");

        let (_, _, configurations) =
            compose(&game, Side::Client).expect("the selected pack values should resolve");
        let resolved = &configurations[0];

        assert_eq!(
            resolved.value(&"client.enabled".parse().expect("a setting key")),
            Some(&json!(true)),
            "allowed leaves the last word to the player"
        );
        assert_eq!(
            resolved.value(&"integrated.mob_limit".parse().expect("a setting key")),
            Some(&json!(40)),
            "locked leaves the last word to the pack"
        );
        assert_eq!(
            resolved.value(&"dedicated.motd".parse().expect("a setting key")),
            Some(&json!("Pack")),
            "required is supplied by and fixed to the pack"
        );
        assert_eq!(
            resolved.value(&"common.profile".parse().expect("a setting key")),
            Some(&json!("quiet"))
        );
    }

    #[test]
    fn a_player_file_changes_the_selected_values() {
        let game = directory("player-values");
        let player = ValueDocument::parse(
            &json!({
                "schemaVersion": 1,
                "owner": "configuration",
                "values": {
                    "client.enabled": false,
                    "common.profile": "verbose"
                }
            })
            .to_string(),
        )
        .expect("a valid player document");
        file::write(&game, &player).expect("the player file should be writable");

        let (_, _, configurations) =
            compose(&game, Side::Client).expect("the player values should resolve");

        assert_eq!(
            configurations[0].value(&"client.enabled".parse().expect("a setting key")),
            Some(&json!(false))
        );
        assert_eq!(
            configurations[0].value(&"common.profile".parse().expect("a setting key")),
            Some(&json!("verbose"))
        );
    }
}
