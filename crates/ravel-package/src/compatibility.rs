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
    /// Each declared entry is matched in one of two ways, and the author
    /// chooses which by how they write it:
    ///
    /// - **Exact string.** An entry that is not a version range matches the
    ///   running version by exact string equality. A Minecraft release is not
    ///   required to be semver-shaped (snapshots such as `24w45a` are not), and
    ///   for those the loader still refuses to order two strings — it only
    ///   accepts the ones the author wrote.
    /// - **Explicit range.** An entry that parses as a [`VersionRange`] (for
    ///   example `>=26.1, <26.2`) matches when the running version parses as a
    ///   semver [`Version`] and falls inside it. This is first-class multi-
    ///   version support: a mod that genuinely runs across a span of releases
    ///   declares that span once rather than enumerating every patch. The range
    ///   is the *author's* tested claim, so the loader is not inferring that an
    ///   undeclared release works — it is honouring a declared one. A range is
    ///   only ever evaluated against a semver-shaped running version; against a
    ///   non-semver running string a range entry simply does not match, and an
    ///   exact entry is still required.
    ///
    /// # Errors
    ///
    /// Returns [`CompatibilityError::Minecraft`] when the running version is
    /// neither one of the declared exact strings nor inside any declared range.
    pub fn check_minecraft(&self, running: &str) -> Result<(), CompatibilityError> {
        let running_version: Option<Version> = running.parse().ok();
        for declared in &self.minecraft {
            let text = declared.as_str();
            // Exact-string membership always applies.
            if text == running {
                return Ok(());
            }
            // A range entry applies only when both it and the running version
            // are semver-shaped, so no non-semver string is ever ordered.
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

/// Whether a declared Minecraft entry is a version range rather than an exact
/// version. A range carries a comparator (`>=`, `<`, `<=`, `>`, `=`, `^`, `~`,
/// `*`) or a comma; an exact version like `26.1.2` carries none of these, so it
/// is never treated as a range even though [`VersionRange`] would parse it.
fn is_range(text: &str) -> bool {
    text.contains(',')
        || text.contains('*')
        || text
            .chars()
            .any(|c| matches!(c, '>' | '<' | '=' | '^' | '~'))
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
    fn an_undeclared_minecraft_version_is_refused_when_only_exact_versions_are_declared() {
        // Exact entries are still matched exactly: a range would accept 26.0.1
        // but the author declared only the exact string 26.0.0.
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
        // First-class multi-version support: one declared span, many releases.
        let manifest = manifest("*", r#"[">=26.1, <26.2"]"#);
        for running in ["26.1.0", "26.1.2", "26.1.9"] {
            assert_eq!(
                manifest.compatibility.check_minecraft(running),
                Ok(()),
                "{running}"
            );
        }
        // Outside the range is refused.
        for running in ["26.0.9", "26.2.0"] {
            assert!(
                manifest.compatibility.check_minecraft(running).is_err(),
                "{running}"
            );
        }
    }

    #[test]
    fn exact_and_range_entries_mix_in_one_declaration() {
        // A range for the current line plus an exact pin for an older release.
        let manifest = manifest("*", r#"[">=26.1, <26.2", "25.4.7"]"#);
        assert_eq!(manifest.compatibility.check_minecraft("26.1.5"), Ok(()));
        assert_eq!(manifest.compatibility.check_minecraft("25.4.7"), Ok(()));
        assert!(manifest.compatibility.check_minecraft("25.4.6").is_err());
    }

    #[test]
    fn a_range_never_matches_a_non_semver_running_version() {
        // A snapshot string is not orderable, so a range entry does not apply
        // to it; only an exact entry could accept it.
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
    }
}
