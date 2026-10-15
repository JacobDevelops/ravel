#![forbid(unsafe_code)]

//! Repository automation for Ravel.

mod error;
mod evidence;
pub mod fixture;
mod gate_versions;
pub mod host;
mod instance_scan;
mod inventory;
mod java_shape;
mod json;
mod package_digests;
mod scaffold;
mod spikes;
mod tracked;
mod version_boundary;

#[cfg(test)]
mod test_support;

pub use error::FixtureError;
pub use evidence::manifest::{verify_evidence_manifest, verify_milestone};
pub use evidence::records::verify_evidence_records;
pub use gate_versions::verify_gate_versions;
pub use instance_scan::{Finding, InstanceScan, scan_instance};
pub use package_digests::{DigestOutcome, StaleDigest, refresh_digests};
pub use scaffold::{required_scaffold_paths, verify_scaffold};
pub use spikes::verify_spikes;
pub use tracked::{verify_tracked_contents, verify_tracked_paths};

const CRATE_NAMES: &[&str] = &[
    "ravel-core",
    "ravel-package",
    "ravel-resolve",
    "ravel-loader",
    "ravel-sdk",
    "ravel-sdk-macros",
    "ravel-host-api",
    "ravel-host-runtime",
    "ravel-bootstrap",
    "ravel-composed-host",
    "ravel-build",
    "ravel-minecraft",
    "ravel-minecraft-adapter",
    "ravel-config",
    "ravel-data",
    "ravel-save",
    "ravel-network",
    "ravel-observability",
    "ravel-pack",
    "ravel-acceptance",
    "ravel-testkit",
];

const TEST_MOD_NAMES: &[&str] = &[
    "audio",
    "client-only",
    "commands",
    "configuration",
    "dependency-base",
    "dependency-consumer",
    "events",
    "failure-cases",
    "inter-mod-service",
    "interception",
    "networking",
    "noop",
    "optional-service",
    "provision",
    "registries",
    "render-stages",
    "resources",
    "saved-data",
    "server-data",
    "server-only",
    "waystone-ledger",
    "world-shaping",
];

const REGISTRY_MOD_NAMES: &[&str] = &[
    "rvl-002-counterpart",
    "rvl-025-counterpart",
    "rvl-042-counterpart",
    "rvl-060-counterpart",
    "rvl-086-counterpart",
    "rvl-120-counterpart",
    "rvl-132-counterpart",
    "rvl-146-counterpart",
    "rvl-180-counterpart",
    "rvl-210-counterpart",
    "rvl-287-counterpart",
    "rvl-321-counterpart",
];
