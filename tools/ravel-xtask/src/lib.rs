#![forbid(unsafe_code)]

//! Repository automation for Ravel.

mod error;
mod evidence;
pub mod fixture;
mod gate_versions;
mod hooks;
pub mod host;
mod instance_scan;
mod inventory;
mod java_shape;
mod json;
mod package_digests;
mod scaffold;
mod tracked;
mod version_boundary;

#[cfg(test)]
mod test_support;

pub use error::FixtureError;
pub use evidence::manifest::{verify_evidence_manifest, verify_milestone};
pub use evidence::records::verify_evidence_records;
pub use evidence::spikes::verify_spikes;
pub use gate_versions::{GateVersionsReport, verify_gate_versions};
pub use instance_scan::{Finding, InstanceScan, scan_instance};
pub use package_digests::{DigestOutcome, StaleDigest, refresh_digests};
pub use scaffold::{required_scaffold_paths, verify_scaffold};
pub use tracked::{verify_tracked_contents, verify_tracked_paths};
pub use version_boundary::version_matrix;

const EXAMPLE_NAMES: &[&str] = &[
    "content-mod",
    "guide-snippets",
    "minimal-mod",
    "stateful-mod",
];

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
    "ravel-minecraft-generate",
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
    "events",
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

/// Encodes digest bytes without hashing them a second time.
pub(crate) fn hex_digest(digest: &[u8]) -> String {
    use std::fmt::Write as _;

    let mut output = String::with_capacity(digest.len() * 2);
    for byte in digest {
        write!(&mut output, "{byte:02x}").expect("writing to a String cannot fail");
    }
    output
}

#[cfg(test)]
mod tests {
    #[test]
    fn digest_encoding_preserves_leading_zeroes() {
        assert_eq!(super::hex_digest(&[0, 1, 15, 16, 255]), "00010f10ff");
        assert_eq!(super::hex_digest(&[]), "");
    }
}
