use std::collections::BTreeSet;
use std::fmt;
use std::path::PathBuf;

use ravel_core::{LifecycleState, ModId, Side, Version};
use ravel_host_api::{HostError, HostMod};
use ravel_loader::{
    LifecycleEntry, LifecycleError, LifecycleEvent, LifecycleMachine, Requirement,
    executes_mod_code,
};
use ravel_package::{
    CompatibilityError, IntegrityError, ManifestError, PackageManifest, verify_package,
};
use ravel_resolve::{LoadPlan, ResolutionError, resolve};

/// The `SPEC.md` §14 states a successful run passes through, in order.
///
/// [`LifecycleState::Discovered`] is absent because it is where every mod
/// starts rather than somewhere it moves to, and [`LifecycleState::Failed`] is
/// absent because a successful run never reaches it.
const RUN_ORDER: [LifecycleState; 8] = [
    LifecycleState::Validated,
    LifecycleState::Resolved,
    LifecycleState::Loaded,
    LifecycleState::Constructed,
    LifecycleState::Registered,
    LifecycleState::Running,
    LifecycleState::Stopping,
    LifecycleState::Stopped,
];

/// A package the loader found on disk, before anything about it was believed.
///
/// The document is held as text rather than as a parsed manifest because
/// parsing it is the first thing `SPEC.md` §14 requires of validation, so a
/// harness that took a manifest would be skipping the step it exists to
/// exercise. Where the text came from is the caller's business: `ravel-package`
/// deliberately defines no on-disk package layout, and neither does this.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DiscoveredPackage {
    /// The package directory that the manifest's relative paths resolve against.
    pub root: PathBuf,
    /// The manifest document, exactly as it was read.
    pub document: String,
}

impl DiscoveredPackage {
    /// Records a package found at `root` with the manifest text `document`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>, document: impl Into<String>) -> Self {
        Self {
            root: root.into(),
            document: document.into(),
        }
    }

    /// Reads a package whose manifest sits at `root/`[`Self::MANIFEST_FILE`].
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::Unreadable`] when the manifest cannot be read.
    pub fn read(root: impl Into<PathBuf>) -> Result<Self, PipelineError> {
        let root = root.into();
        let document =
            std::fs::read_to_string(root.join(Self::MANIFEST_FILE)).map_err(|error| {
                PipelineError::Unreadable {
                    file: Self::MANIFEST_FILE.to_owned(),
                    detail: error.to_string(),
                }
            })?;
        Ok(Self { root, document })
    }

    /// The manifest file name [`Self::read`] looks for.
    ///
    /// This is a convention of this harness and not of the package contract.
    /// `schemas/package.schema.json` describes a manifest's contents and says
    /// nothing about what it is called, so naming one here is a test-support
    /// decision that a later packaging tool is free to overrule.
    pub const MANIFEST_FILE: &'static str = "package.json";
}

/// The versions a package set is being admitted against.
///
/// Both are supplied rather than discovered. Ravel's own version is the running
/// loader's, and the Minecraft version belongs to `minecraft/versions/`, so a
/// version-neutral crate that guessed either would be encoding a version
/// boundary `SPEC.md` §3.7 puts somewhere else.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Host {
    /// The Ravel version a package's `compatibility.ravel` range is checked
    /// against.
    pub ravel: Version,
    /// The exact Minecraft version a package's `compatibility.minecraft` list
    /// is checked against.
    pub minecraft: String,
}

/// What one completed run of the pipeline did.
///
/// The transcript is the evidence `ROADMAP.md`'s M2-W6 row asks for: every
/// state change, in the order it happened, for a package set that was validated
/// and resolved first. It is a property of the package set alone, because the
/// plan it was driven from is the lexicographically smallest topological order
/// of the dependency graph and every traversal here preserves that order.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct PipelineRun {
    plan: LoadPlan,
    transcript: Vec<LifecycleEvent>,
}

impl PipelineRun {
    /// Returns the plan the run was driven from.
    #[must_use]
    pub const fn plan(&self) -> &LoadPlan {
        &self.plan
    }

    /// Returns every state change, in the order it happened.
    #[must_use]
    pub fn transcript(&self) -> &[LifecycleEvent] {
        &self.transcript
    }

    /// Returns the states one mod was taken through, in order.
    ///
    /// This is the transcript projected onto a single identity, which is what a
    /// report about one mod needs and what a per-mod determinism check compares.
    #[must_use]
    pub fn states_of(&self, mod_id: &ModId) -> Vec<LifecycleState> {
        self.transcript
            .iter()
            .filter(|event| event.mod_id == *mod_id)
            .map(|event| event.state)
            .collect()
    }
}

/// Drives a discovered package set through the whole pre-world pipeline.
///
/// The run is validate, resolve, then the `SPEC.md` §14 states in order, and it
/// is the composition M2-W6 exists to demonstrate rather than any new rule:
/// `ravel-package` decides what validates, `ravel-resolve` decides the order,
/// and `ravel-loader` decides which transitions are legal. Nothing here
/// overrides any of them.
///
/// Mod code is entered exactly when [`executes_mod_code`] says a state runs it,
/// which is [`LifecycleState::Loaded`] through [`LifecycleState::Stopping`].
/// Every mod is driven in resolved order, after the whole set has taken the
/// step, because `LifecycleMachine::advance_all` moves a set together or not at
/// all.
///
/// `mods` are values the caller already holds. This function never loads,
/// links, opens, or spawns anything to obtain one: how a mod's code arrives in
/// the process is ADG-2's question, ADG-2 is deferred to M3, and
/// `docs/architecture/dependency-rules.md` forbids selecting a mod execution
/// model before then. When M3 selects one, the mechanism produces the values
/// passed here and this function does not change.
///
/// Two `SPEC.md` §14 validation facets are deliberately absent. Target
/// selection is ADG-2's, and `ravel-package` refuses to select a target for the
/// same reason; the pre-world diagnostic path that reports an unsupported one is
/// M2-W7's.
///
/// # Errors
///
/// Returns the first failure in pipeline order, so a package set that breaks
/// two rules reports the earlier one: [`PipelineError::Manifest`] and
/// [`PipelineError::Integrity`] and [`PipelineError::Compatibility`] during
/// validation, [`PipelineError::Resolution`] during resolution, and
/// [`PipelineError::Host`] or [`PipelineError::Lifecycle`] while running.
/// [`PipelineError::UnexpectedMod`] is raised before any state changes and
/// [`PipelineError::MissingMod`] when the plan reaches a mod the caller did not
/// supply.
pub fn run(
    packages: &[DiscoveredPackage],
    host: &Host,
    side: Side,
    mods: &mut [&mut dyn HostMod],
) -> Result<PipelineRun, PipelineError> {
    let manifests = validate(packages, host)?;
    let plan = resolve(&manifests, side).map_err(PipelineError::Resolution)?;
    refuse_mods_outside_the_plan(&plan, mods)?;

    let mut machine = LifecycleMachine::new(side, entries(&plan, &manifests))
        .map_err(PipelineError::Lifecycle)?;
    for state in RUN_ORDER {
        machine
            .advance_all(state)
            .map_err(PipelineError::Lifecycle)?;
        if executes_mod_code(state) {
            enter_all(&plan, mods, state)?;
        }
    }

    Ok(PipelineRun {
        plan,
        transcript: machine.transcript().to_vec(),
    })
}

/// Admits every package, or none of them.
///
/// The four checks are what `SPEC.md` §14 requires before
/// [`LifecycleState::Validated`], minus the target facet ADG-2 owns. They run
/// over the whole set before resolution begins, because a package set is
/// admitted together and resolving a set that holds an unvalidated package
/// would be reading declarations nothing has vouched for.
fn validate(
    packages: &[DiscoveredPackage],
    host: &Host,
) -> Result<Vec<PackageManifest>, PipelineError> {
    let mut manifests = Vec::with_capacity(packages.len());
    for package in packages {
        let manifest =
            PackageManifest::parse(&package.document).map_err(PipelineError::Manifest)?;
        verify_package(&package.root, &manifest).map_err(PipelineError::Integrity)?;
        manifest
            .compatibility
            .check_ravel(&host.ravel)
            .map_err(PipelineError::Compatibility)?;
        manifest
            .compatibility
            .check_minecraft(&host.minecraft)
            .map_err(PipelineError::Compatibility)?;
        manifests.push(manifest);
    }
    Ok(manifests)
}

/// Builds the lifecycle view of each mod the plan ordered.
///
/// Every mod in a plan is [`Requirement::Required`]. The resolver has already
/// dropped an optional dependency that was absent, so everything that reached
/// the plan is installed and expected to load, and treating any of it as
/// droppable would grant world access to a set that lost a mod the player
/// installed. Which mods a failure may be tolerated for is M2-W7's to refine.
fn entries(plan: &LoadPlan, manifests: &[PackageManifest]) -> Vec<LifecycleEntry> {
    plan.order()
        .iter()
        .filter_map(|id| {
            manifests
                .iter()
                .find(|manifest| manifest.id == *id)
                .map(|manifest| LifecycleEntry {
                    mod_id: manifest.id.clone(),
                    sides: manifest.sides.clone(),
                    requirement: Requirement::Required,
                    saved_data_versions: manifest.saved_data_versions.iter().copied().collect(),
                })
        })
        .collect()
}

/// Refuses a mod value the resolved plan never asked for.
///
/// A value for a mod that is not loading is a harness mistake worth catching
/// early: it would sit untouched through the whole run and its absence from the
/// transcript would read as a mod that failed rather than one that was never
/// part of the set.
fn refuse_mods_outside_the_plan(
    plan: &LoadPlan,
    mods: &[&mut dyn HostMod],
) -> Result<(), PipelineError> {
    let planned: BTreeSet<&ModId> = plan.order().iter().collect();
    for host in mods {
        if !planned.contains(host.id()) {
            return Err(PipelineError::UnexpectedMod {
                mod_id: host.id().clone(),
            });
        }
    }
    Ok(())
}

/// Takes every mod into `state`, in resolved order.
fn enter_all(
    plan: &LoadPlan,
    mods: &mut [&mut dyn HostMod],
    state: LifecycleState,
) -> Result<(), PipelineError> {
    for id in plan.order() {
        let host = mods
            .iter_mut()
            .find(|host| host.id() == id)
            .ok_or_else(|| PipelineError::MissingMod {
                mod_id: id.clone(),
                state,
            })?;
        host.enter(state).map_err(PipelineError::Host)?;
    }
    Ok(())
}

/// Why a package set did not complete the pipeline.
///
/// The first five variants forward the failure the owning crate raised, kept
/// whole rather than flattened into text, so a caller can assert on the rule
/// that was broken instead of on a sentence. The last three are this harness's
/// own, and every one of them names a mod or a file rather than a path, for the
/// reason `SPEC.md` §9.1 gives.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum PipelineError {
    /// A manifest file could not be read.
    Unreadable {
        /// The declared file name.
        file: String,
        /// What the operating system reported.
        detail: String,
    },
    /// A manifest document is not a package manifest.
    Manifest(ManifestError),
    /// A declared file is not the file its manifest describes.
    Integrity(IntegrityError),
    /// A package does not support the host it was asked to load on.
    Compatibility(CompatibilityError),
    /// A validated package set has no load order.
    Resolution(ResolutionError),
    /// The lifecycle refused a step of the run.
    Lifecycle(LifecycleError),
    /// A mod declined a step it was driven through.
    Host(HostError),
    /// The plan ordered a mod the caller supplied no value for.
    MissingMod {
        /// The mod the plan named.
        mod_id: ModId,
        /// The state it would have entered.
        state: LifecycleState,
    },
    /// The caller supplied a value for a mod the plan never ordered.
    UnexpectedMod {
        /// The mod that is not in the plan.
        mod_id: ModId,
    },
}

impl fmt::Display for PipelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { file, detail } => {
                write!(formatter, "`{file}` could not be read: {detail}")
            }
            Self::Manifest(error) => write!(formatter, "validation failed: {error}"),
            Self::Integrity(error) => write!(formatter, "validation failed: {error}"),
            Self::Compatibility(error) => write!(formatter, "validation failed: {error}"),
            Self::Resolution(error) => write!(formatter, "resolution failed: {error}"),
            Self::Lifecycle(error) => write!(formatter, "the run failed: {error}"),
            Self::Host(error) => write!(formatter, "the run failed: {error}"),
            Self::MissingMod { mod_id, state } => write!(
                formatter,
                "the plan orders `{mod_id}`, but no mod was supplied to enter `{}`",
                state.name()
            ),
            Self::UnexpectedMod { mod_id } => write!(
                formatter,
                "`{mod_id}` was supplied to the run, but the plan does not order it"
            ),
        }
    }
}

impl std::error::Error for PipelineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Manifest(error) => Some(error),
            Self::Integrity(error) => Some(error),
            Self::Compatibility(error) => Some(error),
            Self::Resolution(error) => Some(error),
            Self::Lifecycle(error) => Some(error),
            Self::Host(error) => Some(error),
            Self::Unreadable { .. } | Self::MissingMod { .. } | Self::UnexpectedMod { .. } => None,
        }
    }
}

/// Returns the states a mod is taken through, which is what
/// [`executes_mod_code`] admits.
///
/// A caller comparing a mod's own record against the run needs the same list
/// the run used, and deriving it twice is how the two drift apart.
#[must_use]
pub fn executed_states() -> Vec<LifecycleState> {
    RUN_ORDER
        .into_iter()
        .filter(|state| executes_mod_code(*state))
        .collect()
}

/// Returns the states a successful run passes through, in order.
#[must_use]
pub fn run_order() -> Vec<LifecycleState> {
    RUN_ORDER.to_vec()
}
