//! Host-build refusals and diagnostic attribution.

use std::fmt;

use ravel_core::{FailureKind, LifecycleState, StructuredFailure};

/// Why a host could not be built from what the instance holds.
#[derive(Debug)]
pub enum BuildRefused {
    /// The Ravel installation's dependency inputs could not be parsed or validated.
    Installation {
        /// The installation-relative input.
        file: String,
        /// What failed.
        detail: String,
    },
    /// An installation input could not be read from disk.
    InstallationIo {
        /// The installation-relative input.
        file: String,
        /// The original operating-system failure.
        source: std::io::Error,
    },
    /// An installed package carries no `Cargo.toml`, so it names no crate.
    MissingCargoManifest {
        /// The package directory's name.
        directory: String,
    },
    /// An installed package's `Cargo.toml` cannot be read as one.
    CargoManifest {
        /// The package directory's name.
        directory: String,
        /// What the parser or schema validation reported.
        detail: String,
    },
    /// An installed package's Cargo manifest could not be read from disk.
    CargoManifestIo {
        /// The package directory's name.
        directory: String,
        /// The original operating-system failure.
        source: std::io::Error,
    },
    /// Two installed packages declare one crate name.
    DuplicateCrate {
        /// The name both declare.
        crate_name: String,
        /// The second directory declaring it, in mod-id order.
        directory: String,
        /// The first directory declaring it.
        first: String,
    },
    /// The generated workspace could not be written.
    Write {
        /// The generated file, named relative to the instance.
        file: String,
        /// What the operating system reported.
        source: std::io::Error,
    },
    /// The toolchain could not be run at all.
    Toolchain {
        /// What the operating system reported.
        source: std::io::Error,
    },
    /// The toolchain ran and refused the installed source.
    Compile {
        /// What it said, with the instance's own path redacted.
        detail: String,
    },
    /// The instance's resolved dependency lock could not be parsed or validated.
    ResolvedLock {
        /// What failed.
        detail: String,
    },
    /// The instance's resolved dependency lock could not be read from disk.
    ResolvedLockIo {
        /// The original operating-system failure reading `ravel/Cargo.lock`.
        source: std::io::Error,
    },
    /// Resolution requires an external package the installation did not vet.
    UnpinnedDependency {
        /// The external package's name.
        package: String,
        /// The resolved version, requested requirement, or `<unresolved>` if Cargo found no package.
        version: String,
        /// Installed mods whose dependency graph reaches it.
        mods: Vec<String>,
    },
    /// The toolchain reported success and produced no library.
    MissingArtifact {
        /// The library that is not there.
        library: String,
    },
}

impl fmt::Display for BuildRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Installation { file, detail } => write!(
                formatter,
                "Ravel installation `{file}` is invalid: {detail}"
            ),
            Self::InstallationIo { file, source } => write!(
                formatter,
                "Ravel installation `{file}` could not be read: {source}"
            ),
            Self::MissingCargoManifest { directory } => write!(
                formatter,
                "`{directory}` carries no `Cargo.toml`, so nothing says what crate to build"
            ),
            Self::CargoManifest { directory, detail } => {
                write!(
                    formatter,
                    "`{directory}` declares no buildable crate: {detail}"
                )
            }
            Self::CargoManifestIo { directory, source } => write!(
                formatter,
                "`{directory}/Cargo.toml` could not be read: {source}"
            ),
            Self::DuplicateCrate {
                crate_name,
                directory,
                first,
            } => write!(
                formatter,
                "`{directory}` builds the crate `{crate_name}`, which `{first}` already builds"
            ),
            Self::Write { file, source } => {
                write!(formatter, "`{file}` could not be written: {source}")
            }
            Self::Toolchain { source } => write!(
                formatter,
                "the Rust toolchain that compiles installed mods could not be run: {source}"
            ),
            Self::Compile { detail } => {
                write!(formatter, "the installed mods did not compile:\n{detail}")
            }
            Self::ResolvedLock { detail } => write!(
                formatter,
                "the instance dependency lock `ravel/Cargo.lock` is invalid: {detail}"
            ),
            Self::ResolvedLockIo { source } => write!(
                formatter,
                "the instance dependency lock `ravel/Cargo.lock` could not be read: {source}"
            ),
            Self::UnpinnedDependency {
                package,
                version,
                mods,
            } => write!(
                formatter,
                "resolved external dependency {package} {version} is not pinned by the Ravel \
                 installation; required by installed mod(s): {}",
                if mods.is_empty() {
                    "<unknown>".to_owned()
                } else {
                    mods.join(", ")
                }
            ),
            Self::MissingArtifact { library } => write!(
                formatter,
                "the build reported success and produced no `{library}`"
            ),
        }
    }
}

impl std::error::Error for BuildRefused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::InstallationIo { source, .. }
            | Self::CargoManifestIo { source, .. }
            | Self::Write { source, .. }
            | Self::Toolchain { source }
            | Self::ResolvedLockIo { source } => Some(source),
            Self::Installation { .. }
            | Self::MissingCargoManifest { .. }
            | Self::CargoManifest { .. }
            | Self::DuplicateCrate { .. }
            | Self::Compile { .. }
            | Self::ResolvedLock { .. }
            | Self::UnpinnedDependency { .. }
            | Self::MissingArtifact { .. } => None,
        }
    }
}

impl StructuredFailure for BuildRefused {
    fn failure_kind(&self) -> Option<FailureKind> {
        None
    }

    fn lifecycle_state(&self) -> LifecycleState {
        LifecycleState::Validated
    }

    fn quotes_untrusted_text(&self) -> bool {
        match self {
            Self::Installation { .. }
            | Self::InstallationIo { .. }
            | Self::CargoManifestIo { .. }
            | Self::CargoManifest { .. }
            | Self::Compile { .. }
            | Self::Toolchain { .. }
            | Self::ResolvedLock { .. }
            | Self::ResolvedLockIo { .. }
            | Self::UnpinnedDependency { .. } => true,
            Self::MissingCargoManifest { .. }
            | Self::DuplicateCrate { .. }
            | Self::Write { .. }
            | Self::MissingArtifact { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::BuildRefused;

    #[test]
    fn io_refusals_preserve_the_original_io_source_kind() {
        let original = || std::io::Error::from(std::io::ErrorKind::PermissionDenied);
        for refused in [
            BuildRefused::InstallationIo {
                file: "Cargo.toml".to_owned(),
                source: original(),
            },
            BuildRefused::CargoManifestIo {
                directory: "example".to_owned(),
                source: original(),
            },
            BuildRefused::Write {
                file: "host/Cargo.toml".to_owned(),
                source: original(),
            },
            BuildRefused::Toolchain { source: original() },
            BuildRefused::ResolvedLockIo { source: original() },
        ] {
            let source = std::error::Error::source(&refused)
                .expect("every IO refusal has a source")
                .downcast_ref::<std::io::Error>()
                .expect("the source is the original IO error");
            assert_eq!(source.kind(), std::io::ErrorKind::PermissionDenied);
        }
    }
}
