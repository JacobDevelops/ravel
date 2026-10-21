use std::collections::BTreeSet;
use std::fmt;
use std::path::PathBuf;

use ravel_core::{FailureEnvironment, LifecycleState, ModId, Side, StructuredFailure, Version};
use ravel_host_api::Mod;
use ravel_loader::{
    DriveError, LifecycleEntry, LifecycleError, LifecycleEvent, LifecycleMachine, LoadedMods,
    MANIFEST_FILE, MissingMod, admitted, executes_mod_code, start_all, stop_all,
};
use ravel_package::{
    CompatibilityError, IntegrityError, ManifestError, PackageManifest, verify_package,
};
use ravel_resolve::{LoadPlan, ResolutionError, resolve};

// Discovered is the initial state; successful runs never enter Failed.
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

/// A package root and unparsed manifest, preserving the validation boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TestPackage {
    /// The package directory that the manifest's relative paths resolve against.
    pub root: PathBuf,
    /// The manifest document, exactly as it was read.
    pub document: String,
}

impl TestPackage {
    /// Records a package found at `root` with the manifest text `document`.
    #[must_use]
    pub fn new(root: impl Into<PathBuf>, document: impl Into<String>) -> Self {
        Self {
            root: root.into(),
            document: document.into(),
        }
    }

    /// Reads a package whose root contains [`MANIFEST_FILE`].
    ///
    /// # Errors
    ///
    /// Returns [`PipelineError::Unreadable`] when the manifest cannot be read.
    pub fn read(root: impl Into<PathBuf>) -> Result<Self, PipelineError> {
        let root = root.into();
        let document = std::fs::read_to_string(root.join(MANIFEST_FILE)).map_err(|error| {
            PipelineError::Unreadable {
                file: MANIFEST_FILE.to_owned(),
                source: error,
            }
        })?;
        Ok(Self { root, document })
    }
}

/// Explicit host versions used to check package compatibility.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Host {
    /// The Ravel version a package's `compatibility.ravel` range is checked
    /// against.
    pub ravel: Version,
    /// The exact Minecraft version a package's `compatibility.minecraft` list
    /// is checked against.
    pub minecraft: String,
}

/// A resolved package plan and its ordered lifecycle transcript.
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
    #[must_use]
    pub fn states_of(&self, mod_id: &ModId) -> Vec<LifecycleState> {
        self.transcript
            .iter()
            .filter(|event| event.mod_id == *mod_id)
            .map(|event| event.state)
            .collect()
    }
}

/// Validates and resolves all packages before driving mods through the lifecycle.
///
/// Mods run in resolved order after each set-wide transition. Packages not active
/// on this side remain at [`LifecycleState::Resolved`] and need no mod value.
/// The caller supplies already-constructed mods; this function loads no code.
/// Startup and shutdown both use the production loader drivers, with the supplied
/// host versions and the current process's platform and architecture.
///
/// # Errors
///
/// Returns the first validation, resolution, mod-set, lifecycle, or host failure.
pub fn run_pipeline(
    packages: &[TestPackage],
    host: &Host,
    side: Side,
    mods: &mut [&mut dyn Mod],
) -> Result<PipelineRun, PipelineError> {
    let manifests = validate(packages, host)?;
    let plan = resolve(&manifests, side).map_err(PipelineError::Resolution)?;
    // Exercise the production admission boundary even after validating compatibility.
    let entries = admitted(&plan, &manifests, &host.ravel, &host.minecraft)
        .map_err(PipelineError::Compatibility)?;
    refuse_mods_outside_the_set(&entries, mods)?;

    let environment = FailureEnvironment {
        ravel: host.ravel.clone(),
        minecraft: host.minecraft.clone(),
        platform: std::env::consts::OS.to_owned(),
        architecture: std::env::consts::ARCH.to_owned(),
    };
    let mut machine = LifecycleMachine::new(side, entries).map_err(PipelineError::Lifecycle)?;
    let mut loaded = LoadedMods::new(&mut machine, mods).map_err(PipelineError::MissingMod)?;
    start_all(&mut loaded, &environment).map_err(PipelineError::from)?;
    stop_all(&mut loaded, &environment).map_err(PipelineError::from)?;

    Ok(PipelineRun {
        plan,
        transcript: machine.transcript().to_vec(),
    })
}

fn validate(packages: &[TestPackage], host: &Host) -> Result<Vec<PackageManifest>, PipelineError> {
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

fn refuse_mods_outside_the_set(
    entries: &[LifecycleEntry],
    mods: &[&mut dyn Mod],
) -> Result<(), PipelineError> {
    let admitted: BTreeSet<&ModId> = entries.iter().map(|entry| &entry.mod_id).collect();
    for member in mods {
        if !admitted.contains(member.id()) {
            return Err(PipelineError::UnexpectedMod {
                mod_id: member.id().clone(),
            });
        }
    }
    Ok(())
}

/// A pipeline failure, preserving errors from the responsible crate.
#[derive(Debug)]
pub enum PipelineError {
    /// A manifest file could not be read.
    Unreadable {
        /// The declared file name.
        file: String,
        /// What the operating system reported.
        source: std::io::Error,
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
    /// The production driver refused the set, retaining its failure attribution.
    Drive(DriveError),
    /// An active lifecycle member has no supplied mod value.
    MissingMod(MissingMod),
    /// The caller supplied a value for a mod the package set does not contain.
    UnexpectedMod {
        /// The mod that is not in the set.
        mod_id: ModId,
    },
}

impl fmt::Display for PipelineError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unreadable { file, source } => {
                write!(formatter, "`{file}` could not be read: {source}")
            }
            Self::Manifest(error) => write!(formatter, "validation failed: {error}"),
            Self::Integrity(error) => write!(formatter, "validation failed: {error}"),
            Self::Compatibility(error) => write!(formatter, "validation failed: {error}"),
            Self::Resolution(error) => write!(formatter, "resolution failed: {error}"),
            Self::Lifecycle(error) => write!(formatter, "the run failed: {error}"),
            Self::Drive(error) => write!(formatter, "the run failed: {error}"),
            Self::MissingMod(error) => write!(formatter, "the run failed: {error}"),
            Self::UnexpectedMod { mod_id } => write!(
                formatter,
                "`{mod_id}` was supplied to the run, but the package set does not contain it"
            ),
        }
    }
}

impl std::error::Error for PipelineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unreadable { source, .. } => Some(source),
            Self::Manifest(error) => Some(error),
            Self::Integrity(error) => Some(error),
            Self::Compatibility(error) => Some(error),
            Self::Resolution(error) => Some(error),
            Self::Lifecycle(error) => Some(error),
            Self::Drive(error) => Some(error),
            Self::MissingMod(error) => Some(error),
            Self::UnexpectedMod { .. } => None,
        }
    }
}

impl PipelineError {
    /// Returns the underlying structured failure, or `None` for harness-owned errors.
    #[must_use]
    pub fn failure(&self) -> Option<&(dyn StructuredFailure + 'static)> {
        match self {
            Self::Manifest(error) => Some(error),
            Self::Integrity(error) => Some(error),
            Self::Compatibility(error) => Some(error),
            Self::Resolution(error) => Some(error),
            Self::Lifecycle(error) => Some(error),
            Self::Drive(error) => Some(error),
            Self::MissingMod(error) => Some(error),
            Self::Unreadable { .. } | Self::UnexpectedMod { .. } => None,
        }
    }
}
impl From<DriveError> for PipelineError {
    fn from(error: DriveError) -> Self {
        Self::Drive(error)
    }
}

/// Returns the run states admitted by [`executes_mod_code`].
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
