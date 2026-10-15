//! Host-build refusals and diagnostic attribution.

use std::fmt;

use ravel_core::{FailureKind, LifecycleState, StructuredFailure};

/// Why a host could not be built from what the instance holds.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum BuildRefused {
    /// An installed package carries no `Cargo.toml`, so it names no crate.
    MissingCargoManifest {
        /// The package directory's name.
        directory: String,
    },
    /// An installed package's `Cargo.toml` cannot be read as one.
    CargoManifest {
        /// The package directory's name.
        directory: String,
        /// What the parser or the operating system reported.
        detail: String,
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
        detail: String,
    },
    /// The toolchain could not be run at all.
    Toolchain {
        /// What the operating system reported.
        detail: String,
    },
    /// The toolchain ran and refused the installed source.
    Compile {
        /// What it said, with the instance's own path redacted.
        detail: String,
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
            Self::DuplicateCrate {
                crate_name,
                directory,
                first,
            } => write!(
                formatter,
                "`{directory}` builds the crate `{crate_name}`, which `{first}` already builds"
            ),
            Self::Write { file, detail } => {
                write!(formatter, "`{file}` could not be written: {detail}")
            }
            Self::Toolchain { detail } => write!(
                formatter,
                "the Rust toolchain that compiles installed mods could not be run: {detail}"
            ),
            Self::Compile { detail } => {
                write!(formatter, "the installed mods did not compile:\n{detail}")
            }
            Self::MissingArtifact { library } => write!(
                formatter,
                "the build reported success and produced no `{library}`"
            ),
        }
    }
}

impl std::error::Error for BuildRefused {}

impl StructuredFailure for BuildRefused {
    fn failure_kind(&self) -> Option<FailureKind> {
        None
    }

    fn lifecycle_state(&self) -> LifecycleState {
        LifecycleState::Validated
    }

    fn quotes_untrusted_text(&self) -> bool {
        match self {
            Self::CargoManifest { .. } | Self::Compile { .. } | Self::Toolchain { .. } => true,
            Self::MissingCargoManifest { .. }
            | Self::DuplicateCrate { .. }
            | Self::Write { .. }
            | Self::MissingArtifact { .. } => false,
        }
    }
}
