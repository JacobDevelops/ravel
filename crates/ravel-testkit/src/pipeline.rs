use std::collections::BTreeSet;
use std::fmt;
use std::path::PathBuf;

use ravel_core::{LifecycleState, ModId, Side, StructuredFailure, Version};
use ravel_host_api::{HostError, HostMod};
use ravel_loader::{
    LifecycleEntry, LifecycleError, LifecycleEvent, LifecycleMachine, admitted, executes_mod_code,
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

    /// The harness's manifest filename convention, not part of the package contract.
    pub const MANIFEST_FILE: &'static str = "package.json";
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
///
/// # Errors
///
/// Returns the first validation, resolution, mod-set, lifecycle, or host failure.
pub fn run(
    packages: &[DiscoveredPackage],
    host: &Host,
    side: Side,
    mods: &mut [&mut dyn HostMod],
) -> Result<PipelineRun, PipelineError> {
    let manifests = validate(packages, host)?;
    let plan = resolve(&manifests, side).map_err(PipelineError::Resolution)?;
    // Exercise the production admission boundary even after validating compatibility.
    let entries = admitted(&plan, &manifests, &host.ravel, &host.minecraft)
        .map_err(PipelineError::Compatibility)?;
    refuse_mods_outside_the_set(&entries, mods)?;

    let mut machine = LifecycleMachine::new(side, entries).map_err(PipelineError::Lifecycle)?;
    for state in RUN_ORDER {
        machine
            .advance_all(state)
            .map_err(PipelineError::Lifecycle)?;
        if executes_mod_code(state) {
            enter_all(&plan, mods, state)?;
        }
        if state == LifecycleState::Constructed {
            register_all(&plan, mods, &mut machine)?;
        }
    }

    Ok(PipelineRun {
        plan,
        transcript: machine.transcript().to_vec(),
    })
}

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

fn refuse_mods_outside_the_set(
    entries: &[LifecycleEntry],
    mods: &[&mut dyn HostMod],
) -> Result<(), PipelineError> {
    let admitted: BTreeSet<&ModId> = entries.iter().map(|entry| &entry.mod_id).collect();
    for host in mods {
        if !admitted.contains(host.id()) {
            return Err(PipelineError::UnexpectedMod {
                mod_id: host.id().clone(),
            });
        }
    }
    Ok(())
}

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

// Claim in resolved order; preserve window violations over a mod's summary error.
fn register_all(
    plan: &LoadPlan,
    mods: &mut [&mut dyn HostMod],
    machine: &mut LifecycleMachine,
) -> Result<(), PipelineError> {
    for id in plan.order() {
        let host = mods
            .iter_mut()
            .find(|host| host.id() == id)
            .ok_or_else(|| PipelineError::MissingMod {
                mod_id: id.clone(),
                state: LifecycleState::Registered,
            })?;

        let mut window = machine
            .registration_window(id)
            .map_err(PipelineError::Lifecycle)?;
        let outcome = host.register(&mut window);
        let unexpected = window.unexpected().cloned();

        if let Some(error) = unexpected {
            return Err(PipelineError::Lifecycle(error));
        }
        outcome.map_err(PipelineError::Host)?;
    }
    Ok(())
}

/// A pipeline failure, preserving errors from the responsible crate.
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
    /// The caller supplied a value for a mod the package set does not contain.
    UnexpectedMod {
        /// The mod that is not in the set.
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
                "`{mod_id}` was supplied to the run, but the package set does not contain it"
            ),
        }
    }
}

impl std::error::Error for PipelineError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.stage()
            .map(|failure| failure as &dyn std::error::Error)
    }
}

impl PipelineError {
    /// Returns the underlying structured failure, or `None` for harness-owned errors.
    #[must_use]
    pub fn stage(&self) -> Option<&(dyn StructuredFailure + 'static)> {
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
