#![forbid(unsafe_code)]

//! Version-neutral identity, version, path, side, lifecycle, and diagnostic
//! types.

mod content;
mod diagnostics;
mod identity;
pub mod paths;
mod precedence;
mod redaction;
mod sides;
mod states;

pub use content::{
    ContentId, ContentIdError, ContentKind, ContentKindError, RegistryKey, RegistryKeyError,
};
pub use diagnostics::{
    DiagnosticMessage, FailureAttribution, FailureEnvironment, FailureKind, FailureSurface,
    Redaction, StructuredFailure,
};
pub use identity::{ModId, ModIdError, check_identifier};
pub use paths::{RelativePath, RelativePathError};
pub use precedence::{Cycle, precedence};
pub use redaction::{CREDENTIAL_PREFIXES, CREDENTIAL_WORDS, contains_credential};
/// An exact semantic version of a mod or Ravel, not a Minecraft release.
pub use semver::Version;
/// A Cargo-style compatibility range over [`Version`]; bare versions mean `^`.
pub use semver::VersionReq as VersionRange;
pub use sides::{ClientOnly, ClientOnlyError, EmptySideSet, Side, SideSet};
pub use states::LifecycleState;

/// The environment variable selecting a build's tracked Minecraft version.
pub const MINECRAFT_VERSION_SELECTION: &str = "RAVEL_MINECRAFT_VERSION";

#[cfg(test)]
mod tests {
    use super::{Version, VersionRange};

    #[test]
    fn the_adopted_range_grammar_covers_every_comparator_it_documents() {
        for (grammar, candidate, expected) in [
            (">=1.2, <2.0", "1.4.0", true),
            (">=1.2, <2.0", "1.1.9", false),
            (">=1.2, <2.0", "2.0.0", false),
            ("^1.2.3", "1.9.0", true),
            ("^1.2.3", "2.0.0", false),
            ("~1.2.3", "1.2.9", true),
            ("~1.2.3", "1.3.0", false),
            ("=1.0.0", "1.0.0", true),
            ("=1.0.0", "1.0.1", false),
            (">1.0.0", "1.0.1", true),
            ("<=1.0.0", "1.0.0", true),
            ("*", "9.9.9", true),
            ("1.2.3", "1.4.0", true),
        ] {
            let range: VersionRange = grammar.parse().expect("valid range");
            let version: Version = candidate.parse().expect("valid version");
            assert_eq!(
                range.matches(&version),
                expected,
                "{grammar} vs {candidate}"
            );
        }
    }

    #[test]
    fn a_malformed_version_or_range_is_refused_rather_than_guessed() {
        assert!("not-a-version".parse::<Version>().is_err());
        assert!("1".parse::<Version>().is_err());
        assert!("1.2".parse::<Version>().is_err());
        assert!(">=".parse::<VersionRange>().is_err());
        assert!("!1.0.0".parse::<VersionRange>().is_err());
    }
}
