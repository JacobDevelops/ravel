#![forbid(unsafe_code)]

//! Package contracts for source-distributed Ravel mods.

mod compatibility;
mod integrity;
mod manifest;
mod values;

pub use compatibility::CompatibilityError;
pub use integrity::{IntegrityError, verify_package, verify_provenance, verify_source_file};
pub use manifest::{
    Author, Compatibility, Dependency, DependencyOrder, ManifestError, PackageManifest, Provenance,
    Source, SourceFile,
};
pub use values::{
    EmptyText, NonEmptyText, RelativePath, RelativePathError, SchemaVersion, SchemaVersionError,
    Sha256Digest, Sha256Error,
};
