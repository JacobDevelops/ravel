#![forbid(unsafe_code)]

//! Repository automation for Ravel.

mod evidence;
pub mod fixture;
mod gate_versions;
mod hooks;
pub mod host;
mod instance_scan;
mod json;
mod package_digests;
mod scaffold;
mod text;
mod tracked;
mod version_boundary;

#[cfg(test)]
mod test_support;

pub use evidence::manifest::{verify_evidence_manifest, verify_evidence_records, verify_milestone};
pub use evidence::spikes::verify_spikes;
pub use fixture::FixtureError;
pub use gate_versions::{GateVersionsReport, verify_gate_versions};
pub use instance_scan::{Finding, InstanceScan, scan_instance};
pub use package_digests::{StaleDigest, refresh_digests, stale_digests};
pub use scaffold::{required_scaffold_paths, verify_scaffold};
pub use tracked::{verify_tracked_contents, verify_tracked_paths};
pub use version_boundary::matrix_eligibility;

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
