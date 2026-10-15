#![forbid(unsafe_code)]

//! Shared package admission, configuration, lifecycle, and pack setup for hosts.
//! Hosts supply their members and export their own JNI entry point.

mod options;

pub use options::{OptionsError, Seeded, seed_pack};
/// World-pack installation and writing at the world-open boundary.
pub use ravel_bootstrap_runtime::{install_packs, write_world_pack};

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use ravel_bootstrap_runtime::{
    HookError, SideError, install, install_settings, pre_world_report, resource_pack_metadata,
    side as running_side,
};
use ravel_config::{LayeredConfiguration, ValueDocument, file};
use ravel_core::{ModId, Side};
use ravel_host_api::HostMod;
pub use ravel_loader::Written;
use ravel_loader::{
    ConfigRefused, DiscoveryRefused, DriveError, Half, LifecycleEntry, LifecycleError,
    LifecycleMachine, PACK_DIRECTORY, PackError, RESOURCE_PACKS, ResourceRefused, Supplied,
    admitted, discover, drive, open_configuration, open_resources, write_pack,
};
use ravel_package::{CompatibilityError, ManifestError, PackageManifest};
use ravel_resolve::{ResolutionError, resolve};

/// Running versions and launch-report environment.
pub use ravel_bootstrap_runtime::environment;

/// Decodes the running side before side-dependent members are constructed.
///
/// # Errors
///
/// Returns [`LaunchError::Side`] for an unknown side code.
pub fn side(code: i32) -> Result<Side, LaunchError> {
    running_side(code).map_err(LaunchError::Side)
}

/// One member of a composed set: the package it ships as, and its value.
pub struct Member {
    /// The `package.json` this member ships, as written.
    pub package: String,
    /// The compiled Rust value for this package.
    pub host: Box<dyn HostMod>,
}

type Admitted = Vec<(LifecycleEntry, Box<dyn HostMod>)>;

/// Lifecycle, mods, resolved settings, and supplied files for one launch.
pub type Composition = (
    LifecycleMachine,
    Vec<Box<dyn HostMod>>,
    Vec<LayeredConfiguration>,
    Supplied,
);

/// Admits and configures a composed set, opening services before any mod starts.
///
/// # Errors
///
/// Returns an error on package admission, configuration, resources, services, or lifecycle failure.
pub fn compose(
    game_directory: &Path,
    side: Side,
    members: Vec<Member>,
) -> Result<Composition, LaunchError> {
    account_for_installed(game_directory, &members)?;

    let packs = file::read_pack(game_directory).map_err(LaunchError::PackFile)?;

    let mut composed = admit(side, members)?;
    let players = player_documents(game_directory, side, &composed)?;
    let plan = composed.iter().map(|(entry, _)| entry.clone()).collect();
    let machine = LifecycleMachine::new(side, plan).map_err(LaunchError::Lifecycle)?;
    let configurations = {
        let mut hosts: Vec<&mut dyn HostMod> = Vec::with_capacity(composed.len());
        for (_, host) in &mut composed {
            hosts.push(&mut **host);
        }
        open_configuration(&machine, &mut hosts, &packs, &players)
            .map_err(LaunchError::Config)?
            .resolved()
            .to_vec()
    };

    let (machine, mods) = drive(side, composed).map_err(|error| match error {
        DriveError::Services(error) => LaunchError::Services(error),
        error @ (DriveError::Identity { .. } | DriveError::Lifecycle(_) | DriveError::Host(_)) => {
            LaunchError::Drive(error)
        }
    })?;
    // Resource declarations are available only after registration in `drive`.
    let resources = open_resources(&machine, &mods).map_err(LaunchError::Resources)?;

    Ok((machine, mods, configurations, resources))
}

fn player_documents(
    game_directory: &Path,
    side: Side,
    members: &[(LifecycleEntry, Box<dyn HostMod>)],
) -> Result<BTreeMap<ModId, ValueDocument>, LaunchError> {
    let mut players = BTreeMap::new();
    for (entry, host) in members {
        if !entry.sides.loads_on(side) {
            continue;
        }
        let Some(declaration) = host.configuration_declaration().map_err(|source| {
            LaunchError::Config(ConfigRefused::Mod {
                mod_id: host.id().clone(),
                source,
            })
        })?
        else {
            continue;
        };
        if let Some(document) =
            file::read(game_directory, &declaration.owner).map_err(LaunchError::PlayerFile)?
        {
            players.insert(declaration.owner, document);
        }
    }
    Ok(players)
}

/// Refuses installed packages that have no compiled member.
fn account_for_installed(game_directory: &Path, members: &[Member]) -> Result<(), LaunchError> {
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

fn admit(side: Side, members: Vec<Member>) -> Result<Admitted, LaunchError> {
    let mut manifests = Vec::with_capacity(members.len());
    let mut values = Vec::with_capacity(members.len());
    for member in members {
        manifests.push(PackageManifest::parse(&member.package).map_err(LaunchError::Manifest)?);
        values.push(member.host);
    }

    let plan = resolve(&manifests, side).map_err(LaunchError::Resolution)?;
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

/// Composes and installs a host's members for the agent-reported side.
///
/// # Errors
///
/// Returns an error if the directory, side, composition, pack setup, or installation fails.
pub fn initialize(code: i32, members: Vec<Member>) -> Result<(), LaunchError> {
    let game_directory = std::env::current_dir().map_err(LaunchError::CurrentDirectory)?;
    let side = side(code)?;
    let (machine, mods, configurations, resources) = compose(&game_directory, side, members)?;
    // The client scans resource packs during startup, before runtime installation.
    write_resource_pack(&game_directory, &resources)?;
    let seeded = seed_pack(&game_directory).map_err(LaunchError::Options)?;
    // World data is written later, when the world-open crossing supplies its path.
    install_packs(resources);
    install_settings(side, configurations);
    let mut report = pre_world_report();
    if let Some(notice) = pack_notice(&seeded, side) {
        report.note(&notice);
    }
    install(machine, mods, report).map_err(LaunchError::Install)
}

/// Maximum characters quoted from an unreadable settings line.
const QUOTED_LINE: usize = 60;

fn quoted(line: &str) -> String {
    match line.char_indices().nth(QUOTED_LINE) {
        None => line.to_owned(),
        Some((at, _)) => format!("{}…", &line[..at]),
    }
}

// Player opt-outs are silent; dedicated servers receive only record failures.
fn pack_notice(seeded: &Seeded, side: Side) -> Option<String> {
    let ending = "Turn `ravel` on under Options, Resource Packs to use them now.";
    let advisable = side != Side::DedicatedServer;
    match seeded {
        Seeded::Added | Seeded::Present | Seeded::AlreadyOffered => None,
        Seeded::NoSettings => advisable.then(|| {
            format!(
                "Ravel wrote the resource pack its mods supply and could not switch it on, because \
             this instance has no `options.txt` yet. It will be switched on the next time the \
             game starts. {ending}"
            )
        }),
        Seeded::Unrecorded(reason) => Some(format!(
            "Ravel could not keep its record of having offered this instance the resource pack \
             its mods supply, so it will offer again the next time the game starts — turning the \
             pack off may not stick until this is fixed. The reason was: {reason}"
        )),
        Seeded::Unreadable(line) => advisable.then(|| {
            format!(
                "Ravel wrote the resource pack its mods supply and could not switch it on, because \
             the `resourcePacks` list in `options.txt` is not a list of pack names and was left \
             exactly as it is. It starts `{}`. Repairing or deleting that line lets the next \
             launch switch the pack on. {ending}",
                quoted(line)
            )
        }),
    }
}

/// Writes the composed assets to the instance's resource-pack directory.
///
/// World data is written separately at world open.
///
/// # Errors
///
/// Returns [`LaunchError::Pack`] if writing the pack fails.
pub fn write_resource_pack(
    game_directory: &Path,
    resources: &Supplied,
) -> Result<Written, LaunchError> {
    write_pack(
        &game_directory.join(RESOURCE_PACKS).join(PACK_DIRECTORY),
        resources,
        Half::Assets,
        &resource_pack_metadata(Half::Assets.description()),
    )
    .map_err(LaunchError::Pack)
}

/// Why a composed set could not start.
#[derive(Debug)]
pub enum LaunchError {
    /// The JNI frame could not establish its guarded environment.
    Jvm(jni::errors::Error),
    /// The process could not name its launch directory.
    CurrentDirectory(std::io::Error),
    /// The code the agent reported names no side this host knows.
    Side(SideError),
    /// The pack's settings layer could not be read.
    PackFile(file::FileError),
    /// The player's existing value document could not be read.
    PlayerFile(file::FileError),
    /// The instance's installed packages could not be established.
    Discovery(DiscoveryRefused),
    /// An installed package names no value this host holds.
    NotComposed(ModId),
    /// A composed member's shipped package manifest does not parse.
    Manifest(ManifestError),
    /// A composed member declares a Ravel or Minecraft version this build is not.
    Compatibility(CompatibilityError),
    /// The composed package set has no legal load order on this side.
    Resolution(ResolutionError),
    /// An admitted entry and the composed values do not account for each other.
    Composition(ModId),
    /// The admitted plan could not be established as a lifecycle.
    Lifecycle(LifecycleError),
    /// The set's settings could not be resolved or consumed.
    Config(ConfigRefused),
    /// The set's supplied files could not be layered.
    Resources(ResourceRefused),
    /// The set's services could not be resolved or consumed.
    Services(ravel_loader::ServiceRefused),
    /// The set's files could not be written as a pack the game reads.
    Pack(PackError),
    /// The player's settings could not be read or replaced.
    Options(OptionsError),
    /// The lifecycle could not drive a composed value.
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
                "`{mod_id}` is installed and this host holds no value for it, so its code cannot \
                 run"
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
                "`{mod_id}` was admitted without a composed value, or holds one nothing admitted"
            ),
            Self::Lifecycle(error) => {
                write!(formatter, "the admitted set has no lifecycle: {error}")
            }
            Self::Config(error) => {
                write!(formatter, "the set's settings were not applied: {error}")
            }
            Self::Resources(error) => {
                write!(formatter, "the set's files were not layered: {error}")
            }
            Self::Services(error) => {
                write!(formatter, "the set's services were not opened: {error}")
            }
            Self::Pack(error) => {
                write!(formatter, "the set's pack was not written: {error}")
            }
            Self::Options(error) => {
                write!(
                    formatter,
                    "the player's settings were not readable: {error}"
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
            Self::Lifecycle(error) => Some(error),
            Self::Config(error) => Some(error),
            Self::Resources(error) => Some(error),
            Self::Services(error) => Some(error),
            Self::Pack(error) => Some(error),
            Self::Options(error) => Some(error),
            Self::Composition(_) | Self::NotComposed(_) => None,
            Self::Drive(error) => Some(error),
            Self::Install(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex, PoisonError};

    use ravel_core::{LifecycleState, ModId, Side};
    use ravel_host_api::{HostError, HostMod};
    use ravel_loader::drive;

    use super::{Member, admit, environment};

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

    struct ServiceConsumer {
        id: ModId,
        states: Arc<Mutex<Vec<LifecycleState>>>,
    }

    impl HostMod for ServiceConsumer {
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

        fn service_declaration(
            &self,
        ) -> Result<Option<ravel_host_api::ServiceDeclaration>, HostError> {
            Ok(Some(
                ravel_host_api::ServiceDeclaration::new(
                    self.id.clone(),
                    [],
                    [ravel_host_api::ServiceId::new(
                        "absent:clock".parse().expect("a valid service id"),
                    )],
                )
                .expect("a valid service declaration"),
            ))
        }
    }

    #[test]
    fn missing_services_refuse_launch_before_any_mod_starts() {
        let (package, id, states) = member("consumer", &["client"]);
        let game =
            std::env::temp_dir().join(format!("ravel-services-missing-{}", std::process::id()));
        std::fs::create_dir_all(&game).expect("create launch directory");
        let result = super::compose(
            &game,
            Side::Client,
            vec![Member {
                package,
                host: Box::new(ServiceConsumer {
                    id,
                    states: Arc::clone(&states),
                }),
            }],
        );
        std::fs::remove_dir_all(game).expect("remove launch directory");
        assert!(matches!(
            result,
            Err(super::LaunchError::Services(
                ravel_loader::ServiceRefused::NoProvider { .. }
            ))
        ));
        assert!(
            !steps(&states).contains(&LifecycleState::Running),
            "service refusal must happen before entering Running"
        );
    }

    struct RejectingConfiguration {
        id: ModId,
    }

    impl HostMod for RejectingConfiguration {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
            Ok(())
        }

        fn configuration_declaration(
            &self,
        ) -> Result<Option<ravel_config::ConfigurationDeclaration>, HostError> {
            Err(HostError::Unsupported {
                mod_id: self.id.clone(),
                state: LifecycleState::Constructed,
            })
        }
    }

    #[test]
    fn a_wrong_side_mod_cannot_refuse_configuration_on_this_side() {
        let (package, id, _) = member("client_configuration", &["client"]);
        let game = std::env::temp_dir().join(format!(
            "ravel-wrong-side-configuration-{}",
            std::process::id()
        ));
        std::fs::create_dir_all(&game).expect("create launch directory");
        let result = super::compose(
            &game,
            Side::DedicatedServer,
            vec![Member {
                package,
                host: Box::new(RejectingConfiguration { id }),
            }],
        );
        std::fs::remove_dir_all(game).expect("remove launch directory");
        result.expect("wrong-side declaration must not refuse launch");
    }

    fn steps(log: &Arc<Mutex<Vec<LifecycleState>>>) -> Vec<LifecycleState> {
        log.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

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

    #[test]
    fn only_a_launch_whose_pack_is_off_by_accident_tells_the_player_anything() {
        use super::{Seeded, pack_notice, pre_world_report};

        for silent in [Seeded::Added, Seeded::Present, Seeded::AlreadyOffered] {
            assert_eq!(pack_notice(&silent, Side::Client), None, "{silent:?}");
        }

        for loud in [
            Seeded::NoSettings,
            Seeded::Unreadable("[unclosed".to_owned()),
            Seeded::Unrecorded("permission denied".to_owned()),
        ] {
            let notice = pack_notice(&loud, Side::Client)
                .unwrap_or_else(|| panic!("{loud:?} is not silent"));
            assert!(
                notice.contains("Options, Resource Packs") || notice.contains("offer again"),
                "a notice a player cannot act on is noise: {notice}"
            );
        }
        assert!(
            pack_notice(&Seeded::NoSettings, Side::Client)
                .expect("this one is loud")
                .contains("options.txt")
        );

        let mut report = pre_world_report();
        report.note(&pack_notice(&Seeded::NoSettings, Side::Client).expect("this one is loud"));
        assert_eq!(report.notices().len(), 1);
        assert!(
            report.world_access().is_ok(),
            "a pack that is off withholds nobody's world"
        );
    }

    #[test]
    fn the_unreadable_settings_line_is_quoted_in_what_the_player_is_told() {
        use super::{Seeded, pack_notice};

        let notice = pack_notice(&Seeded::Unreadable("[1,2]".to_owned()), Side::Client)
            .expect("an unreadable list is not silent");

        assert!(notice.contains("`[1,2]`"), "{notice}");
    }

    #[test]
    fn an_unreadable_line_of_any_length_is_quoted_within_a_bound() {
        use super::{QUOTED_LINE, Seeded, pack_notice};

        let short = "[1,2]";
        let long = "x".repeat(10_000);

        let bounded = pack_notice(&Seeded::Unreadable(long.clone()), Side::Client)
            .expect("an unreadable list is not silent");
        let whole = pack_notice(&Seeded::Unreadable(short.to_owned()), Side::Client)
            .expect("an unreadable list is not silent");

        assert!(
            bounded.len() < whole.len() + 2 * QUOTED_LINE,
            "a ten-thousand-character line produced {} characters",
            bounded.len()
        );
        assert!(bounded.contains('…'), "{bounded}");
        assert!(
            !bounded.contains(&long),
            "the whole line reached the player anyway"
        );
        assert!(
            !whole.contains('…'),
            "a line that fits is quoted whole: {whole}"
        );
    }

    #[test]
    fn a_dedicated_server_is_told_nothing_about_a_screen_it_does_not_have() {
        use super::{Seeded, pack_notice};

        for advice in [
            Seeded::NoSettings,
            Seeded::Unreadable("[unclosed".to_owned()),
        ] {
            assert_eq!(
                pack_notice(&advice, Side::DedicatedServer),
                None,
                "{advice:?} reached a dedicated server"
            );
            assert!(
                pack_notice(&advice, Side::Client).is_some(),
                "{advice:?} says nothing anywhere, so the case above proves nothing"
            );
        }

        let unrecorded = Seeded::Unrecorded("permission denied".to_owned());
        for side in Side::ALL {
            assert!(
                pack_notice(&unrecorded, side).is_some(),
                "an unwritable record said nothing on {side:?}"
            );
        }
    }

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
                    package: document.clone(),
                    host: Box::new(Recording {
                        id: mod_id.clone(),
                        states: Arc::clone(log),
                    }),
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
}
