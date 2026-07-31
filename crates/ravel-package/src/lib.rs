#![forbid(unsafe_code)]

//! Package contracts for precompiled Ravel mods.

mod compatibility;
mod integrity;
mod manifest;
mod values;

use ravel_core::{DiagnosticMessage, FailureKind, LifecycleState, Redaction};

pub use compatibility::CompatibilityError;
pub use integrity::{IntegrityError, verify_artifact, verify_package, verify_provenance};
pub use manifest::{
    Artifact, ArtifactRole, Author, Compatibility, Dependency, DependencyOrder, ManifestError,
    PackageManifest, Provenance, Target,
};
pub use values::{
    EmptyText, NonEmptyText, RelativePath, RelativePathError, SchemaVersion, SchemaVersionError,
    Sha256Digest, Sha256Error,
};

/// A failure that stops a package before any of its code runs.
///
/// `SPEC.md` §9.1 requires a failure report to carry a lifecycle phase and
/// redacted detail, and §9.2 requires the dependency-graph outcomes to be named
/// distinctly before world access. This trait is how the crate that formats
/// those reports gets all three from any package failure without matching on
/// each error type, and it is the reason every message here names a file rather
/// than a path.
pub trait PackageFailure: std::error::Error {
    /// The `SPEC.md` §9.2 outcome this failure is, when it is one of them.
    ///
    /// That list describes the dependency graph. A manifest that will not parse
    /// or a file that does not match its digest fails earlier than the graph,
    /// so it honestly reports no kind rather than borrowing one.
    fn failure_kind(&self) -> Option<FailureKind>;

    /// The state a package is in when this failure is raised.
    ///
    /// Everything this crate checks is what `SPEC.md` §14 requires before
    /// [`LifecycleState::Validated`], so a package that fails any of it is
    /// still [`LifecycleState::Discovered`] and has run no code.
    fn lifecycle_state(&self) -> LifecycleState {
        LifecycleState::Discovered
    }

    /// The failure text, ready to publish.
    ///
    /// Every message in this crate is written to hold no filesystem path and no
    /// credential, so the text is tagged [`Redaction::Applied`] rather than
    /// being handed to a policy that would have nothing to remove.
    fn diagnostic(&self) -> DiagnosticMessage {
        DiagnosticMessage {
            text: self.to_string(),
            redaction: Redaction::Applied,
        }
    }
}
