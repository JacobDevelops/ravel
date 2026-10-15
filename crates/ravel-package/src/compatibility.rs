use std::fmt;

use ravel_core::{FailureKind, StructuredFailure, Version, VersionRange};

use crate::manifest::Compatibility;

impl Compatibility {
    /// Checks the declared Ravel range against the running Ravel version.
    ///
    /// # Errors
    ///
    /// Returns [`CompatibilityError::Ravel`] when the running version is outside the range.
    pub fn check_ravel(&self, running: &Version) -> Result<(), CompatibilityError> {
        if self.ravel.matches(running) {
            return Ok(());
        }
        Err(CompatibilityError::Ravel {
            declared: self.ravel.clone(),
            running: running.clone(),
        })
    }

    /// Matches exact Minecraft version strings or explicit ranges against semver versions.
    ///
    /// # Errors
    ///
    /// Returns [`CompatibilityError::Minecraft`] when no declared entry matches.
    pub fn check_minecraft(&self, running: &str) -> Result<(), CompatibilityError> {
        let running_version: Option<Version> = running.parse().ok();
        for declared in &self.minecraft {
            let text = declared.as_str();
            if text == running {
                return Ok(());
            }
            if let (Some(running_version), Ok(range)) =
                (running_version.as_ref(), text.parse::<VersionRange>())
                && is_range(text)
                && range.matches(running_version)
            {
                return Ok(());
            }
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

// Bare versions stay exact even though VersionRange accepts them.
fn is_range(text: &str) -> bool {
    text.contains(',')
        || text.contains('*')
        || text
            .chars()
            .any(|c| matches!(c, '>' | '<' | '=' | '^' | '~'))
}

/// A package does not support the host it was asked to load on.
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
    fn failure_kind(&self) -> Option<FailureKind> {
        Some(FailureKind::IncompatibleVersion)
    }

    fn quotes_untrusted_text(&self) -> bool {
        matches!(self, Self::Minecraft { .. })
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{FailureKind, LifecycleState, Redaction, StructuredFailure, Version};

    use super::CompatibilityError;
    use crate::manifest::PackageManifest;
    use crate::test_support::{DIGEST, manifest_document};

    fn manifest(ravel: &str, minecraft: &str) -> PackageManifest {
        let document = manifest_document(ravel, minecraft, DIGEST, DIGEST);
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
    fn an_undeclared_minecraft_version_is_refused_when_only_exact_versions_are_declared() {
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
    fn a_declared_minecraft_range_accepts_every_semver_version_inside_it() {
        let manifest = manifest("*", r#"[">=26.1, <26.2"]"#);
        for running in ["26.1.0", "26.1.4", "26.1.9"] {
            assert_eq!(
                manifest.compatibility.check_minecraft(running),
                Ok(()),
                "{running}"
            );
        }
        for running in ["26.0.9", "26.2.0"] {
            assert!(
                manifest.compatibility.check_minecraft(running).is_err(),
                "{running}"
            );
        }
    }

    #[test]
    fn exact_and_range_entries_mix_in_one_declaration() {
        let manifest = manifest("*", r#"[">=26.1, <26.2", "25.4.7"]"#);
        assert_eq!(manifest.compatibility.check_minecraft("26.1.5"), Ok(()));
        assert_eq!(manifest.compatibility.check_minecraft("25.4.7"), Ok(()));
        assert!(manifest.compatibility.check_minecraft("25.4.6").is_err());
    }

    #[test]
    fn a_range_never_matches_a_non_semver_running_version() {
        let manifest = manifest("*", r#"[">=26.1, <26.2"]"#);
        assert!(manifest.compatibility.check_minecraft("24w45a").is_err());
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
        assert!(error.quotes_untrusted_text());
    }

    #[test]
    fn a_minecraft_rejection_identifies_free_form_manifest_text_as_quoted() {
        let manifest = manifest("*", r#"["words supplied by the package author"]"#);
        let error = manifest
            .compatibility
            .check_minecraft("26.1.5")
            .expect_err("free-form text does not name the running version");

        assert!(error.quotes_untrusted_text());
        assert!(
            error
                .diagnostic()
                .text()
                .contains("words supplied by the package author")
        );
    }
}
