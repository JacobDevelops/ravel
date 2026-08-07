use std::fmt;

use ravel_core::{FailureKind, StructuredFailure, Version, VersionRange};

use crate::Compatibility;

impl Compatibility {
    /// Checks the declared Ravel range against the running Ravel version.
    ///
    /// `compatibility.ravel` is a range because Ravel versions are semver and
    /// a mod is written against a span of them.
    ///
    /// # Errors
    ///
    /// Returns [`CompatibilityError::Ravel`] when the running version falls
    /// outside the declared range.
    pub fn check_ravel(&self, running: &Version) -> Result<(), CompatibilityError> {
        if self.ravel.matches(running) {
            return Ok(());
        }
        Err(CompatibilityError::Ravel {
            declared: self.ravel.clone(),
            running: running.clone(),
        })
    }

    /// Checks the declared Minecraft versions against the running one.
    ///
    /// This is exact string membership and never a range. `SPEC.md` §3.7 and
    /// `schemas/package.schema.json` both treat a Minecraft version as an
    /// opaque string, a Minecraft release is not required to be semver-shaped,
    /// and a loader that ordered two of them would be claiming to know that a
    /// mod built against one release works against a later one. It does not.
    ///
    /// # Errors
    ///
    /// Returns [`CompatibilityError::Minecraft`] when the running version is
    /// not one of the declared strings.
    pub fn check_minecraft(&self, running: &str) -> Result<(), CompatibilityError> {
        if self
            .minecraft
            .iter()
            .any(|declared| declared.as_str() == running)
        {
            return Ok(());
        }
        Err(CompatibilityError::Minecraft {
            declared: self
                .minecraft
                .iter()
                .map(|declared| declared.as_str().to_owned())
                .collect(),
            running: running.to_owned(),
        })
    }
}

/// A package does not support the host it was asked to load on.
///
/// Both variants carry what the package declared and what is actually running,
/// because `SPEC.md` §9.1 requires a report to name the Minecraft and Ravel
/// versions involved and a player cannot act on "incompatible" alone.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum CompatibilityError {
    /// The running Ravel version is outside the declared range.
    Ravel {
        /// The range the package declared.
        declared: VersionRange,
        /// The Ravel version that is running.
        running: Version,
    },
    /// The running Minecraft version is not one the package declared.
    Minecraft {
        /// The exact versions the package declared.
        declared: Vec<String>,
        /// The Minecraft version that is running.
        running: String,
    },
}

impl fmt::Display for CompatibilityError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Ravel { declared, running } => write!(
                formatter,
                "the package supports Ravel `{declared}`, but Ravel {running} is running"
            ),
            Self::Minecraft { declared, running } => write!(
                formatter,
                "the package supports Minecraft {}, but Minecraft {running} is running",
                declared.join(", ")
            ),
        }
    }
}

impl std::error::Error for CompatibilityError {}

impl StructuredFailure for CompatibilityError {
    /// Both variants are the `SPEC.md` §9.2 incompatible-version outcome.
    fn failure_kind(&self) -> Option<FailureKind> {
        Some(FailureKind::IncompatibleVersion)
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{FailureKind, LifecycleState, Redaction, StructuredFailure, Version};

    use super::CompatibilityError;
    use crate::PackageManifest;

    const DIGEST: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn manifest(ravel: &str, minecraft: &str) -> PackageManifest {
        let document = format!(
            r#"{{
              "schemaVersion": 1,
              "id": "example_mod",
              "version": "1.4.0",
              "compatibility": {{ "ravel": "{ravel}", "minecraft": {minecraft} }},
              "sides": ["client"],
              "authors": [{{ "name": "Example" }}],
              "license": "MIT",
              "source": {{
                "repository": "https://example.invalid/example_mod",
                "files": [{{ "path": "src/lib.rs", "sha256": "{DIGEST}" }}]
              }},
              "provenance": {{ "record": "provenance.json", "sha256": "{DIGEST}" }}
            }}"#
        );
        PackageManifest::parse(&document).expect("a valid manifest")
    }

    fn version(text: &str) -> Version {
        text.parse().expect("a valid version")
    }

    #[test]
    fn a_ravel_version_inside_the_declared_range_is_accepted() {
        let manifest = manifest(">=0.1, <0.2", r#"["26.0.0"]"#);
        for running in ["0.1.0", "0.1.9", "0.1.0+build.1"] {
            assert_eq!(
                manifest.compatibility.check_ravel(&version(running)),
                Ok(()),
                "{running}"
            );
        }
    }

    #[test]
    fn a_ravel_version_outside_the_declared_range_names_both_sides() {
        let manifest = manifest(">=0.1, <0.2", r#"["26.0.0"]"#);
        let error = manifest
            .compatibility
            .check_ravel(&version("0.2.0"))
            .expect_err("outside the range");

        assert_eq!(
            error,
            CompatibilityError::Ravel {
                declared: ">=0.1, <0.2".parse().expect("a valid range"),
                running: version("0.2.0"),
            }
        );
        assert_eq!(
            error.to_string(),
            "the package supports Ravel `>=0.1, <0.2`, but Ravel 0.2.0 is running"
        );
        assert_eq!(error.failure_kind(), Some(FailureKind::IncompatibleVersion));
        assert_eq!(error.lifecycle_state(), LifecycleState::Discovered);
        assert_eq!(error.diagnostic().redaction(), Redaction::Raw);
        assert!(!error.quotes_untrusted_text());
    }

    #[test]
    fn a_declared_minecraft_version_is_matched_as_an_exact_string() {
        let manifest = manifest("*", r#"["26.0.0", "26.0.1"]"#);
        assert_eq!(manifest.compatibility.check_minecraft("26.0.0"), Ok(()));
        assert_eq!(manifest.compatibility.check_minecraft("26.0.1"), Ok(()));
    }

    #[test]
    fn an_undeclared_minecraft_version_is_refused_even_when_a_range_would_accept_it() {
        let manifest = manifest("*", r#"["26.0.0"]"#);
        for running in ["26.0.1", "26.0", "26.0.0-pre1", "26.0.0 "] {
            let error = manifest
                .compatibility
                .check_minecraft(running)
                .expect_err(running);
            assert_eq!(
                error,
                CompatibilityError::Minecraft {
                    declared: vec!["26.0.0".to_owned()],
                    running: running.to_owned(),
                }
            );
            assert_eq!(error.failure_kind(), Some(FailureKind::IncompatibleVersion));
        }
    }

    #[test]
    fn a_minecraft_rejection_lists_every_version_the_package_declared() {
        let manifest = manifest("*", r#"["26.0.0", "26.0.1"]"#);
        let error = manifest
            .compatibility
            .check_minecraft("25.0.0")
            .expect_err("undeclared");
        assert_eq!(
            error.to_string(),
            "the package supports Minecraft 26.0.0, 26.0.1, but Minecraft 25.0.0 is running"
        );
        assert_eq!(error.diagnostic().text(), error.to_string());
    }
}
