#![forbid(unsafe_code)]

//! Repository automation for Ravel.

mod evidence;
pub mod fixture;
mod gate_versions;
mod hooks;
mod instance_scan;
mod json;
mod package_digests;
mod scaffold;
mod text;
mod tracked;
mod vcs;
mod version_boundary;

#[cfg(test)]
mod test_support;

pub use evidence::manifest::{verify_evidence_manifest, verify_evidence_records, verify_milestone};
pub use evidence::spikes::verify_spikes;
pub use gate_versions::{GateVersionsReport, verify_gate_versions};
pub use instance_scan::{Finding, InstanceScan, Loader, MarkerKind, scan_instance};
pub use package_digests::{
    StaleDigest, maintained_package_directories, refresh_digests, stale_digests,
};
pub use scaffold::{required_scaffold_paths, verify_scaffold};
pub use tracked::{verify_tracked_contents, verify_tracked_paths};
pub use version_boundary::{MatrixEligibility, matrix_eligibility};
