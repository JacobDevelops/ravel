#![forbid(unsafe_code)]

//! The composition every Ravel host runs.
//!
//! A host is the binary the JVM loads. There are two of them and there will
//! never usefully be more: [`ravel-composed-host`] holds a member list a Ravel
//! developer compiled, and `ravel-build` generates one from the packages a
//! player installed. This crate is what they share, and it exists so that the
//! second one cannot answer differently from the first.
//!
//! What "differently" would mean is precise. [`compose`] resolves the set with
//! [`ravel_resolve::resolve`], admits it with [`ravel_loader::admitted`], and
//! drives it with [`ravel_loader::drive`]. A generated host that called those
//! three itself would be a second route to a [`ravel_loader::LifecycleEntry`],
//! and two routes can disagree — about a side declaration, about a version
//! range, about load order. There is one route, and it is here.
//!
//! # What a host supplies
//!
//! A [`Member`] is a package document beside the value its Rust source
//! compiled to. Neither half is authority alone: the document says which sides
//! the mod loads on and what it is compatible with, the value is the code that
//! would run there.
//!
//! # What this crate deliberately does not hold
//!
//! No exported JNI symbol. Each host declares its own entry point, because two
//! hosts linking one entry symbol is a link error and the wrong member list.
//! [`initialize`] is the body behind that entry, and every host's entry is a
//! wrapper around it.

mod options;

pub use options::{OptionsError, Seeded, seed_pack};
/// The world-open half of the pack, re-exported so a caller reaches one route to
/// it rather than two: [`initialize`] installs the set and the crossing writes it.
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

/// The versions this build is, and the report a launch hands over.
///
/// Re-exported so a host names the running versions through the crate it already
/// depends on rather than reaching past it into `ravel-bootstrap`.
pub use ravel_bootstrap_runtime::environment;

/// Reads the side the agent named out of the code it reported.
///
/// A host needs the answer before it can build its members, because a member's
/// value may itself be side-dependent, so this is the one question a host asks
/// before [`initialize`].
///
/// # Errors
///
/// Returns [`LaunchError::Side`] for a code no host knows, which installs
/// nothing rather than composing a set for a guessed side.
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

/// What the lifecycle is driven from: one admitted entry beside its value.
type Admitted = Vec<(LifecycleEntry, Box<dyn HostMod>)>;

/// The driven runtime pieces, resolved settings and supplied files produced for
/// one launch.
pub type Composition = (
    LifecycleMachine,
    Vec<Box<dyn HostMod>>,
    Vec<LayeredConfiguration>,
    Supplied,
);

/// Constructs and drives a composed mod set for one launch.
///
/// The order is the order a launch has to take: what the instance holds is
/// established first, so a package the host cannot run refuses before any mod
/// code is entered; then the two document layers are read; then the admitted set
/// is configured, and only then driven.
///
/// # The settings layers
///
/// Both are read through `ravel-config`'s own file module and both are one
/// document per owner: the pack's under [`file::PACK_DIRECTORY`], which is where
/// `ravel-pack` writes it, and the player's under the game directory, which is
/// where an edited setting is saved. Reading one file for every member — which is
/// what this did — gave the second configuring mod a document written for the
/// first and refused the launch.
///
/// The walk itself is [`open_configuration`]'s rather than one written here, so
/// the loader's rules about load order, a parked side and a mod declaring another
/// mod's settings hold for a real launch and not only for its own tests. It wants
/// the admitted plan as a machine, and [`drive`] builds an equal one from the same
/// entries a moment later.
///
/// # Errors
///
/// Returns an error when the instance's packages cannot be established or name
/// no member, when a pack or player document cannot be read, when configuration
/// cannot be resolved or consumed, or when the lifecycle cannot reach running.
pub fn compose(
    game_directory: &Path,
    side: Side,
    members: Vec<Member>,
) -> Result<Composition, LaunchError> {
    account_for_installed(game_directory, &members)?;

    let packs = file::read_pack(game_directory).map_err(LaunchError::PackFile)?;
    let players = player_documents(game_directory, &members)?;

    let mut composed = admit(side, members)?;
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

    let (machine, mods) = drive(side, composed).map_err(LaunchError::Drive)?;
    // After the set is driven rather than before it, because the walk asks a mod
    // for the declaration it registered from and the registration window is
    // inside `drive`.
    let resources = open_resources(&machine, &mods).map_err(LaunchError::Resources)?;

    Ok((machine, mods, configurations, resources))
}

/// Reads the player's own document for every member that declares settings.
///
/// Only those members, rather than everything the player's config directory
/// holds: a file for a mod this host was not built from is not this launch's to
/// refuse.
fn player_documents(
    game_directory: &Path,
    members: &[Member],
) -> Result<BTreeMap<ModId, ValueDocument>, LaunchError> {
    let mut players = BTreeMap::new();
    for member in members {
        let Some(declaration) = member.host.configuration_declaration() else {
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

/// Discovers what the instance has installed and refuses anything the host has
/// no value for.
///
/// Discovery locates each installed package, validates its manifest against the
/// package schema, and verifies its declared source and provenance digests, so a
/// launch that finds a package checks its integrity — `SPEC.md` §10 requires it.
///
/// A package with no member is a refusal rather than an omission. For a host
/// built from the installed set that means the instance changed after the build:
/// admitting it would order a mod with no code behind it, and ignoring it would
/// start a game the player believes holds a mod that is not there, which is the
/// failure `SPEC.md` §9.2's pre-world report exists to prevent.
///
/// # Errors
///
/// Returns [`LaunchError::Discovery`] when the instance's packages cannot be
/// established, and [`LaunchError::NotComposed`] when an installed package names
/// no member.
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

/// Turns composed members into the entries the lifecycle is driven from.
///
/// Every step of it is somebody else's rule: `ravel-package` decides what a
/// manifest means, `ravel-resolve` decides the load order for this side, and
/// [`ravel_loader::admitted`] decides which entries the set is admitted as. What
/// belongs to a host is only the pairing — which Rust value goes with which
/// admitted entry.
///
/// The set is admitted whole rather than filtered: `admitted` returns an entry
/// for every member, including one this side parks, so a player sees a
/// client-only mod listed and marked as not loading here rather than missing.
///
/// # Errors
///
/// Returns [`LaunchError::Manifest`] when a shipped package document does not
/// parse, [`LaunchError::Compatibility`] when a member declares a Ravel range or
/// a Minecraft version this build is outside of, [`LaunchError::Resolution`]
/// when the set has no legal order, and [`LaunchError::Composition`] when an
/// admitted entry names no value or a value is named by no entry.
fn admit(side: Side, members: Vec<Member>) -> Result<Admitted, LaunchError> {
    let mut manifests = Vec::with_capacity(members.len());
    let mut values = Vec::with_capacity(members.len());
    for member in members {
        manifests.push(PackageManifest::parse(&member.package).map_err(LaunchError::Manifest)?);
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

/// Composes and installs a host's members for the side the agent named.
///
/// This is the body of every host's JNI entry point. A host declares the symbol
/// the JVM calls; what happens behind it is this, so the compiled-in host and a
/// generated one start a launch the same way.
///
/// # Errors
///
/// Returns an error when the launch directory cannot be named, when the reported
/// code names no side, when [`compose`] refuses, or when the driven runtime
/// cannot be installed.
pub fn initialize(code: i32, members: Vec<Member>) -> Result<(), LaunchError> {
    let game_directory = std::env::current_dir().map_err(LaunchError::CurrentDirectory)?;
    // Read from what the agent named rather than written here. Every rule below
    // depends on it, so a launch that cannot name its side installs nothing
    // instead of composing a set for a side it guessed.
    let side = side(code)?;
    let (machine, mods, configurations, resources) = compose(&game_directory, side, members)?;
    // Before the runtime is installed, because a pack the game reads late is a
    // pack it does not read: the client builds its pack list while it starts, and
    // a launch that could not serve the models of the mods it is about to
    // register has not started correctly.
    write_resource_pack(&game_directory, &resources)?;
    // And offered to the player, once. A folder pack nobody enabled is a pack the
    // game finds and does not read, so the first launch that finds a settings file
    // puts the id in the list; after that the list is the player's.
    let seeded = seed_pack(&game_directory).map_err(LaunchError::Options)?;
    // The data half is not written here. It belongs to a world, and no world is
    // open yet, so the set is installed for the crossing that knows which one.
    install_packs(resources);
    install_settings(side, configurations);
    // The report this host hands over holds no refusal, and that is a statement
    // about this composition rather than an empty gesture: every stage before
    // the machine here fails this function instead of recording a refusal, and a
    // failure here installs no runtime at all. What the report is for is the
    // pipeline that validates packages and orders a graph, which runs when a
    // pack is compiled and not in this process.
    //
    // It does carry a notice when the pack is written and switched off, because
    // that is a launch where every model the mods registered is a placeholder and
    // nothing else would say why.
    let mut report = pre_world_report();
    if let Some(notice) = pack_notice(&seeded, side) {
        report.note(&notice);
    }
    install(machine, mods, report).map_err(LaunchError::Install)
}

/// How much of an unreadable `resourcePacks` line this quotes back.
///
/// A player recognises the line they have to repair from its opening, and the
/// rest is a value of no known length that the report, the wire and every wrap
/// the mod screen does on a resize would otherwise carry whole.
const QUOTED_LINE: usize = 60;

/// The opening of a line, ending in an ellipsis when there is more of it.
fn quoted(line: &str) -> String {
    match line.char_indices().nth(QUOTED_LINE) {
        None => line.to_owned(),
        Some((at, _)) => format!("{}…", &line[..at]),
    }
}

/// What a player is told when their settings leave Ravel's pack switched off.
///
/// [`Seeded::Added`], [`Seeded::Present`] and [`Seeded::AlreadyOffered`] say
/// nothing: in the first two the pack is on, and in the third it is off because
/// the player turned it off, which is their decision and not a diagnostic.
///
/// The rest are launches where the pack was written, the mods registered their
/// content, and either the game will show the missing-model placeholder for all
/// of it or the player's own choice about the pack will not stick. Each says what
/// happened and what can be done about it now, because none of them is worth a
/// lost session and every one of them is invisible otherwise.
///
/// A dedicated server hears only [`Seeded::Unrecorded`]. The other two end by
/// sending a player to Options, Resource Packs, and that process has no options
/// screen to send an operator to and no player to send. `Unrecorded` names a
/// record Ravel could not read or write, which is a fault on every side and one
/// nothing else on a dedicated server reports. The report itself models no
/// side — it is one host's report and a host is one side — so the side is
/// applied here, where the notice is produced and the side is known.
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

/// Writes the asset half of a composed set as a pack in `game_directory`.
///
/// The instance's own pack directory, which is where the client's folder source
/// scans, so what a mod supplies is a pack the player can see. This is a separate
/// function from [`initialize`] rather than a block inside it because
/// [`initialize`] needs a JVM to call and this needs none: a test that rebuilt the
/// same call would be a second route to the pack, and two routes can disagree
/// about where it lands.
///
/// The data half is deliberately not written here. It belongs to a world rather
/// than to an instance, and no world is open yet.
///
/// # Errors
///
/// Returns [`LaunchError::Pack`] for any filesystem refusal, which fails the
/// launch rather than starting a game serving half a pack.
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

    /// Which outcomes a player hears about, and which are none of their business.
    ///
    /// The two that produce a notice are the launches where the pack exists, the
    /// content is registered and every model of it renders as the placeholder.
    /// The three that do not are a pack that is on and a player who turned it
    /// off, and telling somebody about a choice they made is noise.
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

        // Through a real report, because a notice nothing carries is a string.
        let mut report = pre_world_report();
        report.note(&pack_notice(&Seeded::NoSettings, Side::Client).expect("this one is loud"));
        assert_eq!(report.notices().len(), 1);
        assert!(
            report.world_access().is_ok(),
            "a pack that is off withholds nobody's world"
        );
    }

    /// The unreadable line reaches the player, because it is what they have to fix.
    #[test]
    fn the_unreadable_settings_line_is_quoted_in_what_the_player_is_told() {
        use super::{Seeded, pack_notice};

        let notice = pack_notice(&Seeded::Unreadable("[1,2]".to_owned()), Side::Client)
            .expect("an unreadable list is not silent");

        assert!(notice.contains("`[1,2]`"), "{notice}");
    }

    /// A line of any length costs the notice a bounded number of characters.
    ///
    /// `options.txt` is a file anything can have written, so the value on that
    /// line has no length this code decides. Quoting it whole would put it in
    /// the report, on the wire and through every wrap the mod screen does.
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

    /// The advice rides only where the screen it names exists.
    ///
    /// The two notices that end by sending a player to Options, Resource Packs
    /// are the two a dedicated server does not hear: it has no such screen and no
    /// player at the keyboard, so a notice telling its operator to open one is an
    /// instruction that cannot be followed. A record Ravel could not read or write
    /// is not advice, it is a fault, and silencing it on the one side that has no
    /// other channel for it would leave the operator nothing.
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

    /// The property this crate exists to hold: a declaration decides what runs.
    ///
    /// Three members, driven on all three sides through the same [`admit`] every
    /// host reaches. A client-only mod runs on the client and nowhere else, a
    /// server-only mod runs on both server sides and not on the client, and the
    /// positive control runs everywhere — which is what keeps the first two from
    /// passing because nothing runs at all.
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
}
