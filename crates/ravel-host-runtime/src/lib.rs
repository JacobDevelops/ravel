#![forbid(unsafe_code)]

//! Shared package admission, configuration, lifecycle, and pack setup for hosts.
//! Hosts supply their members and export their own JNI entry point.

mod options;

pub use options::{OptionsError, Seeded, seed_pack};

use std::collections::BTreeMap;
use std::fmt;
use std::path::Path;

use ravel_bootstrap_runtime::{
    Composition, GamePackError, InstallError, SideError, environment, install, pre_world_report,
    write_resource_pack,
};
use ravel_config::file;
use ravel_core::{ModId, Side};
use ravel_host_api::Mod;
use ravel_loader::{
    ConfigRefused, DiscoveryRefused, DriveError, LifecycleEntry, LifecycleError, LifecycleMachine,
    Loaded, LoadedMods, ResourceRefused, admitted, discover, open_configuration, open_resources,
    start_all,
};
use ravel_observability::Profiler;
use ravel_package::{CompatibilityError, ManifestError, PackageManifest};
use ravel_resolve::{ResolutionError, resolve};
use semver::Version;

/// One member of a composed set: the package it ships as, and its value.
pub struct Member {
    /// The `package.json` this member ships, as written.
    pub package: String,
    /// The compiled Rust value for this package.
    pub value: Box<dyn Mod>,
}

struct Admitted {
    entries: Vec<LifecycleEntry>,
    mods: Vec<Box<dyn Mod>>,
    packages: BTreeMap<ModId, Version>,
}

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

    let admitted = admit(side, members)?;
    let machine = LifecycleMachine::new(side, admitted.entries).map_err(LaunchError::Lifecycle)?;
    let mut owned = Loaded::new(machine, admitted.mods)
        .map_err(|error| LaunchError::Composition(error.mod_id().clone()))?;
    let mut loaded = owned.borrow();
    let configurations = open_configuration(
        &mut loaded,
        &packs,
        |owner| file::read(game_directory, owner),
        &environment(),
    )
    .map_err(LaunchError::Config)?
    .into_resolved();
    start_all(&mut loaded, &environment()).map_err(LaunchError::Drive)?;
    let (interceptions, resources) = open_declarations(&mut loaded)?;

    Ok(Composition {
        loaded: owned,
        configurations,
        resources,
        interceptions,
        installed_packages: admitted.packages,
    })
}

fn open_declarations(
    loaded: &mut LoadedMods<'_, Box<dyn Mod>>,
) -> Result<(ravel_host_api::AdmittedPoints, ravel_loader::Supplied), LaunchError> {
    // Refused optional interceptors must fail before any of their files are layered.
    let interceptions = ravel_loader::open_interceptions(loaded, &environment())
        .map_err(LaunchError::Interceptions)?;
    let resources = open_resources(loaded, &environment()).map_err(LaunchError::Resources)?;
    Ok((interceptions, resources))
}

/// Refuses installed packages that have no compiled member.
fn account_for_installed(game_directory: &Path, members: &[Member]) -> Result<(), LaunchError> {
    for installed in discover(game_directory)
        .map_err(LaunchError::Discovery)?
        .packages()
    {
        let id = &installed.manifest().id;
        if !members.iter().any(|member| member.value.id() == id) {
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
        values.push(member.value);
    }

    let plan = resolve(&manifests, side).map_err(LaunchError::Resolution)?;
    let running = environment();
    let entries = admitted(&plan, &manifests, &running.ravel, &running.minecraft)
        .map_err(LaunchError::Compatibility)?;

    if let Some(unclaimed) = values
        .iter()
        .find(|value| !entries.iter().any(|entry| &entry.mod_id == value.id()))
    {
        return Err(LaunchError::Composition(unclaimed.id().clone()));
    }
    let installed_packages = manifests
        .into_iter()
        .map(|manifest| (manifest.id, manifest.version))
        .collect();
    Ok(Admitted {
        entries,
        mods: values,
        packages: installed_packages,
    })
}

/// Enters the shared launch path and translates failures into redacted JVM exceptions.
pub fn enter(
    mut env: jni::EnvUnowned<'_>,
    side_code: jni::sys::jint,
    profiling_requested: jni::sys::jboolean,
    members: impl FnOnce(Side) -> Vec<Member>,
) {
    ravel_bootstrap_runtime::install_panic_hook();
    env.with_env(|env| -> Result<(), LaunchError> {
        let result = std::env::current_dir()
            .map_err(LaunchError::CurrentDirectory)
            .and_then(|directory| {
                let side = ravel_bootstrap_runtime::side_from_agent_code(side_code)
                    .map_err(LaunchError::Side)?;
                initialize(
                    &directory,
                    side,
                    || members(side),
                    profiling_requested != jni::sys::JNI_FALSE,
                )
            });
        if let Err(error) = result {
            env.throw(error.redacted())?;
        }
        Ok(())
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

/// Enters an independent integrated-server composition on its owning game thread.
pub fn enter_integrated(mut env: jni::EnvUnowned<'_>, members: impl FnOnce(Side) -> Vec<Member>) {
    ravel_bootstrap_runtime::install_panic_hook();
    env.with_env(|env| -> Result<(), LaunchError> {
        if !ravel_bootstrap_runtime::integrated_installed() {
            let result = ravel_bootstrap_runtime::process_launch_context()
                .map_err(LaunchError::Install)
                .and_then(|(directory, profiler)| {
                    let composition = compose(
                        &directory,
                        Side::IntegratedServer,
                        members(Side::IntegratedServer),
                    )?;
                    ravel_bootstrap_runtime::install_integrated(composition, directory, profiler)
                        .map_err(LaunchError::Install)
                });
            if let Err(error) = result {
                env.throw(error.redacted())?;
            }
        }
        Ok(())
    })
    .resolve::<jni::errors::ThrowRuntimeExAndDefault>();
}

/// Constructs, composes, and installs members only if no runtime is installed.
///
/// # Errors
///
/// Returns an error if composition, pack setup, or runtime installation fails.
pub fn initialize(
    game_directory: &Path,
    side: Side,
    members: impl FnOnce() -> Vec<Member>,
    profiling_requested: bool,
) -> Result<(), LaunchError> {
    if ravel_bootstrap_runtime::is_installed() {
        return Err(LaunchError::Install(InstallError::AlreadyInstalled));
    }
    let composition = compose(game_directory, side, members())?;
    let mut report = pre_world_report();
    // Legacy injects its resource proxy directly; a second pack would shadow it.
    if !ravel_bootstrap_runtime::LEGACY_ADAPTER {
        // The client scans resource packs during startup, before runtime installation.
        write_resource_pack(game_directory, &composition.resources).map_err(LaunchError::Pack)?;
        let seeded = seed_pack(game_directory).map_err(LaunchError::Options)?;
        if let Some(notice) = pack_notice(&seeded, side) {
            report.note(&notice);
        }
    }
    install(
        composition,
        report,
        game_directory.to_path_buf(),
        Profiler::from_switch(profiling_requested),
    )
    .map_err(LaunchError::Install)
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
    /// The set's interception declarations could not be admitted.
    Interceptions(ravel_loader::InterceptRefused),
    /// The set's files could not be written as a pack the game reads.
    Pack(GamePackError),
    /// The player's settings could not be read or replaced.
    Options(OptionsError),
    /// Services or lifecycle startup refused a composed value.
    Drive(DriveError),
    /// The driven runtime could not be installed.
    Install(InstallError),
}

impl LaunchError {
    /// Redacts launch diagnostics before they cross into a thrown JVM exception.
    pub fn redacted(&self) -> String {
        ravel_core::DiagnosticMessage::redacted(&self.to_string())
            .text()
            .to_owned()
    }
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
            Self::Interceptions(error) => {
                write!(
                    formatter,
                    "the set's interceptions were not admitted: {error}"
                )
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
            Self::PackFile(error) => Some(error),
            Self::Discovery(error) => Some(error),
            Self::Manifest(error) => Some(error),
            Self::Compatibility(error) => Some(error),
            Self::Resolution(error) => Some(error),
            Self::Lifecycle(error) => Some(error),
            Self::Config(error) => Some(error),
            Self::Resources(error) => Some(error),
            Self::Interceptions(error) => Some(error),
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
    use ravel_host_api::Mod;

    use super::{Member, admit, environment};

    #[test]
    fn initialize_only_publishes_resource_packs_for_modern_adapters() {
        const CHILD: &str = "RAVEL_INITIALIZE_PACK_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
                .args([
                    "--exact",
                    "tests::initialize_only_publishes_resource_packs_for_modern_adapters",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .output()
                .expect("isolated initialization test");
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
            return;
        }
        let game = std::env::temp_dir().join(format!("ravel-pack-init-{}", std::process::id()));
        std::fs::create_dir_all(&game).expect("game directory");
        let original = "resourcePacks:[\"vanilla\"]\n";
        std::fs::write(game.join("options.txt"), original).expect("initial settings");
        super::initialize(&game, Side::Client, Vec::new, false).expect("empty launch");
        let settings = std::fs::read_to_string(game.join("options.txt")).expect("player settings");
        if ravel_bootstrap_runtime::LEGACY_ADAPTER {
            assert!(!game.join("resourcepacks/ravel").exists());
            assert!(!game.join("ravel/seeded-packs").exists());
            assert_eq!(settings, original);
        } else {
            assert!(game.join("resourcepacks/ravel/pack.mcmeta").is_file());
            assert!(game.join("ravel/seeded-packs").is_file());
            assert_eq!(
                settings,
                format!(
                    "resourcePacks:[\"vanilla\",\"{}\"]\n",
                    ravel_bootstrap_runtime::pack_id()
                )
            );
        }
        std::fs::remove_dir_all(game).expect("remove game directory");
    }

    #[test]
    fn repeated_initialize_never_constructs_members_or_touches_launch_files() {
        let game =
            std::env::temp_dir().join(format!("ravel-repeat-initialize-{}", std::process::id()));
        assert!(
            !game.exists(),
            "the rejected launch must not create its directory"
        );
        ravel_bootstrap_runtime::install(
            super::Composition {
                loaded: ravel_loader::Loaded::new(
                    ravel_loader::LifecycleMachine::new(Side::Client, vec![]).expect("empty plan"),
                    vec![],
                )
                .expect("complete empty set"),
                configurations: vec![],
                resources: ravel_loader::Supplied::default(),
                interceptions: ravel_host_api::Points::default()
                    .admit()
                    .expect("empty interceptions"),
                installed_packages: std::collections::BTreeMap::new(),
            },
            ravel_bootstrap_runtime::pre_world_report(),
            game.clone(),
            ravel_observability::Profiler::off(),
        )
        .expect("install the first runtime");
        let mut constructed = false;
        let error = super::initialize(
            &game,
            Side::Client,
            || {
                constructed = true;
                std::fs::create_dir_all(&game).expect("factory side effect");
                Vec::new()
            },
            false,
        )
        .expect_err("a second launch is refused");
        assert!(matches!(
            error,
            super::LaunchError::Install(ravel_bootstrap_runtime::InstallError::AlreadyInstalled)
        ));
        assert!(!constructed);
        assert!(
            !game.exists(),
            "rejected initialization has no filesystem effects"
        );
    }

    #[test]
    fn initialize_preserves_the_callers_process_panic_hook() {
        const CHILD: &str = "RAVEL_INITIALIZE_PANIC_HOOK_CHILD";
        if std::env::var_os(CHILD).is_none() {
            let output = std::process::Command::new(std::env::current_exe().expect("test binary"))
                .args([
                    "--exact",
                    "tests::initialize_preserves_the_callers_process_panic_hook",
                    "--nocapture",
                ])
                .env(CHILD, "1")
                .output()
                .expect("isolated panic-hook test");
            assert!(
                output.status.success(),
                "{}",
                String::from_utf8_lossy(&output.stderr)
            );
            assert!(String::from_utf8_lossy(&output.stdout).contains("1 passed"));
            return;
        }
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let observed = Arc::clone(&calls);
        let previous = std::panic::take_hook();
        std::panic::set_hook(Box::new(move |_| {
            observed.fetch_add(1, std::sync::atomic::Ordering::SeqCst);
        }));
        let result = std::panic::catch_unwind(|| {
            super::initialize(
                &std::env::temp_dir(),
                Side::Client,
                || panic!("member factory"),
                false,
            )
        });
        std::panic::set_hook(previous);
        assert!(result.is_err());
        assert_eq!(calls.load(std::sync::atomic::Ordering::SeqCst), 1);
    }

    #[test]
    fn launch_exception_diagnostics_redact_secret_tokens() {
        let error = super::LaunchError::CurrentDirectory(std::io::Error::other(concat!(
            "cannot open /home/player/.minecraft: token=",
            "ghp_",
            "secretvalue"
        )));
        let diagnostic = error.redacted();
        assert!(diagnostic.contains("[redacted]"), "{diagnostic}");
        assert!(
            !diagnostic.contains(concat!("ghp_", "secretvalue")),
            "{diagnostic}"
        );
        assert!(!diagnostic.contains("/home/player"), "{diagnostic}");
    }

    struct RefusingInterceptor {
        id: ModId,
    }

    impl Mod for RefusingInterceptor {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _: LifecycleState) -> Result<(), ravel_host_api::ModError> {
            Ok(())
        }

        fn content_declaration(
            &self,
        ) -> Result<
            Option<std::borrow::Cow<'_, ravel_data::ContentDeclaration>>,
            ravel_host_api::ModError,
        > {
            let mut declaration = ravel_data::ContentDeclaration::new(self.id.clone());
            for path in [
                "assets/optional_interceptor/textures/failed.png",
                "data/optional_interceptor/recipe/failed.json",
            ] {
                declaration
                    .resource(
                        ravel_core::RelativePath::try_from(path.to_owned()).expect("resource path"),
                        b"failed-owner bytes".to_vec(),
                    )
                    .expect("unique resource");
            }
            Ok(Some(std::borrow::Cow::Owned(declaration)))
        }

        fn interception_declaration(
            &self,
        ) -> Result<Option<ravel_host_api::InterceptionDeclaration>, ravel_host_api::ModError>
        {
            Err(ravel_host_api::ModError::Unsupported)
        }
    }

    #[test]
    fn an_optional_refused_interceptor_contributes_no_resource_or_world_pack_bytes() {
        let game =
            std::env::temp_dir().join(format!("ravel-refused-interceptor-{}", std::process::id()));
        let world = game.join("world");
        std::fs::create_dir_all(&world).expect("world directory");
        let (package, id, _) = member("optional_interceptor", &["client", "integrated-server"]);
        let admitted = admit(
            Side::Client,
            vec![Member {
                package,
                value: Box::new(RefusingInterceptor { id: id.clone() }),
            }],
        )
        .expect("package admission");
        let super::Admitted {
            mut entries,
            mut mods,
            ..
        } = admitted;
        entries[0].requirement = ravel_loader::Requirement::Optional;
        let mut machine = ravel_loader::LifecycleMachine::new(Side::Client, entries).expect("plan");
        let mut loaded =
            ravel_loader::LoadedMods::new(&mut machine, &mut mods).expect("paired mods");
        ravel_loader::start_all(&mut loaded, &environment()).expect("registration");
        let (_, resources) =
            super::open_declarations(&mut loaded).expect("optional failure is isolated");
        assert!(
            resources.generated().is_empty(),
            "failed owner files must not be layered"
        );
        assert_eq!(machine.state(&id), Ok(LifecycleState::Failed));
        ravel_bootstrap_runtime::write_resource_pack(&game, &resources).expect("resource pack");
        ravel_bootstrap_runtime::write_world_pack(&world, &resources).expect("world pack");
        assert!(
            !game
                .join(ravel_bootstrap_runtime::RESOURCE_PACKS)
                .join(ravel_bootstrap_runtime::GAME_PACK_DIRECTORY)
                .join("assets/optional_interceptor/textures/failed.png")
                .exists()
        );
        assert!(
            !world
                .join(ravel_bootstrap_runtime::WORLD_DATA_PACKS)
                .join(ravel_bootstrap_runtime::GAME_PACK_DIRECTORY)
                .join("data/optional_interceptor/recipe/failed.json")
                .exists()
        );
        std::fs::remove_dir_all(game).expect("remove scratch game");
    }

    struct Recording {
        id: ModId,
        states: Arc<Mutex<Vec<LifecycleState>>>,
    }

    impl Mod for Recording {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, state: LifecycleState) -> Result<(), ravel_host_api::ModError> {
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

    impl Mod for ServiceConsumer {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, state: LifecycleState) -> Result<(), ravel_host_api::ModError> {
            self.states
                .lock()
                .unwrap_or_else(PoisonError::into_inner)
                .push(state);
            Ok(())
        }

        fn service_declaration(
            &self,
        ) -> Result<Option<ravel_host_api::ServiceDeclaration>, ravel_host_api::ModError> {
            Ok(Some({
                let mut declaration = ravel_host_api::ServiceDeclaration::new(self.id.clone());
                declaration
                    .require(ravel_host_api::ServiceId::new(
                        "absent:clock".parse().expect("a valid service id"),
                    ))
                    .expect("a valid service declaration");
                declaration
            }))
        }
    }

    struct CountingConfiguration {
        id: ModId,
        calls: Arc<std::sync::atomic::AtomicUsize>,
    }

    impl Mod for CountingConfiguration {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), ravel_host_api::ModError> {
            Ok(())
        }

        fn configuration_declaration(
            &self,
        ) -> Result<Option<ravel_config::ConfigurationDeclaration>, ravel_host_api::ModError>
        {
            self.calls
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            Ok(Some(
                ravel_config::ConfigurationDeclaration::parse(&format!(
                    r#"{{"schemaVersion":1,"owner":"{}","version":1,"settings":[]}}"#,
                    self.id
                ))
                .expect("valid empty settings"),
            ))
        }

        fn configure(
            &mut self,
            _resolved: &ravel_config::LayeredConfiguration,
        ) -> Result<(), ravel_host_api::ModError> {
            Ok(())
        }
    }

    #[test]
    fn composition_reads_each_configuration_declaration_once() {
        let (package, id, _) = member("count_configuration", &["client"]);
        let calls = Arc::new(std::sync::atomic::AtomicUsize::new(0));
        let game =
            std::env::temp_dir().join(format!("ravel-configuration-once-{}", std::process::id()));
        std::fs::create_dir_all(&game).expect("create launch directory");
        let composition = super::compose(
            &game,
            Side::Client,
            vec![Member {
                package,
                value: Box::new(CountingConfiguration {
                    id,
                    calls: Arc::clone(&calls),
                }),
            }],
        )
        .expect("configuration starts");
        assert_eq!(calls.load(std::sync::atomic::Ordering::Relaxed), 1);
        assert_eq!(composition.configurations.len(), 1);
        std::fs::remove_dir_all(game).expect("remove launch directory");
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
                value: Box::new(ServiceConsumer {
                    id,
                    states: Arc::clone(&states),
                }),
            }],
        );
        std::fs::remove_dir_all(game).expect("remove launch directory");
        assert!(matches!(
            result,
            Err(super::LaunchError::Drive(
                ravel_loader::DriveError::Services(ravel_loader::ServiceRefused::NoProvider { .. })
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

    impl Mod for RejectingConfiguration {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), ravel_host_api::ModError> {
            Ok(())
        }

        fn configuration_declaration(
            &self,
        ) -> Result<Option<ravel_config::ConfigurationDeclaration>, ravel_host_api::ModError>
        {
            Err(ravel_host_api::ModError::Unsupported)
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
                value: Box::new(RejectingConfiguration { id }),
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
                    value: Box::new(Recording {
                        id: mod_id.clone(),
                        states: Arc::clone(log),
                    }),
                })
                .collect();

            let super::Admitted {
                entries, mut mods, ..
            } = admit(side, members).expect("a compatible set");
            let mut machine =
                ravel_loader::LifecycleMachine::new(side, entries).expect("compatible lifecycle");
            ravel_loader::start_all(
                &mut ravel_loader::LoadedMods::new(&mut machine, &mut mods).expect("complete set"),
                &environment(),
            )
            .expect("the composed set should start");

            let expected = [side == Side::Client, side.is_server(), true];
            for (((_, mod_id, log), (name, _)), runs) in
                built.iter().zip(&declarations).zip(expected)
            {
                assert_eq!(
                    machine.state(mod_id) == Ok(LifecycleState::Running),
                    runs,
                    "`{name}` on `{}`",
                    side.as_str()
                );
                assert_eq!(
                    !steps(log).is_empty(),
                    runs,
                    "`{name}` ran mod code on `{}`",
                    side.as_str()
                );
            }
        }
    }
}
