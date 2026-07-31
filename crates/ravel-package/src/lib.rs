#![forbid(unsafe_code)]

//! Package contracts for precompiled Ravel mods.

mod compatibility;
mod integrity;
mod manifest;
mod values;

pub use compatibility::{CompatibilityError, TargetError};
pub use integrity::{IntegrityError, verify_artifact, verify_package, verify_provenance};
pub use manifest::{
    Artifact, ArtifactRole, Author, Compatibility, Dependency, DependencyOrder, ManifestError,
    PackageManifest, Provenance, Target,
};
pub use values::{
    EmptyText, NonEmptyText, RelativePath, RelativePathError, SchemaVersion, SchemaVersionError,
    Sha256Digest, Sha256Error,
};
