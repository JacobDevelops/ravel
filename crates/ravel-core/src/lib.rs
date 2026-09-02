#![forbid(unsafe_code)]

//! Version-neutral identity, version, path, side, lifecycle, and diagnostic
//! types.

mod content;
mod diagnostics;
mod identity;
mod paths;
mod precedence;
mod sides;
mod states;

use serde::{Deserialize, Serialize};

pub use content::{
    BlockBehaviour, ContentBehaviour, ContentId, ContentIdError, ContentKind, ContentKindError,
    ItemBehaviour, RegistryKey, RegistryKeyError,
};
pub use diagnostics::{
    DiagnosticMessage, FailureAttribution, FailureEnvironment, FailureKind, FailureSurface,
    Redaction, RedactionPolicy, StructuredFailure,
};
pub use identity::{ModId, ModIdError};
pub use paths::{RelativePath, RelativePathError};
pub use precedence::{Cycle, precedence};
/// An exact released version of a mod or of Ravel itself.
///
/// The grammar is `schemas/package.schema.json`'s `version` pattern, which is
/// semantic versioning, so `semver::Version` implements it and Ravel defines no
/// version grammar of its own. A Minecraft version is deliberately not this
/// type: the schema treats `compatibility.minecraft` as a list of exact opaque
/// strings, because a Minecraft release is not required to be semver-shaped.
pub use semver::Version;
/// A compatibility range over [`Version`].
///
/// `schemas/package.schema.json` leaves the range grammar to the loader, and
/// the grammar Ravel adopts is Cargo's `semver::VersionReq`: comma-separated
/// comparators built from `^`, `~`, `=`, `>`, `>=`, `<`, `<=`, and `*`, where a
/// bare version means `^`. It covers both fields that need a range — a
/// package's `compatibility.ravel` and a dependency's `version` — and nothing
/// else needs one, because Minecraft compatibility is an exact-string list.
pub use semver::VersionReq as VersionRange;
pub use sides::{ClientOnly, EmptySideSet, Side, SideSet, WrongSide};
pub use states::LifecycleState;

/// Names which tracked Minecraft version a build generates its bindings from.
///
/// `docs/decisions/version-selection-is-a-build-time-choice.md` made the
/// selection an input of a build rather than a pin in the tree, so more than one
/// thing has to spell it: the adapter's build script reads it, and a build Ravel
/// itself launches has to set it rather than hope it was inherited. The name
/// lives here because two crates that spelled it separately would be one rule
/// written twice, and the one that drifted would silently build a version
/// nobody chose.
pub const MINECRAFT_VERSION_SELECTION: &str = "RAVEL_MINECRAFT_VERSION";

/// How a declared dependency constrains the load graph (`SPEC.md` §3.4).
///
/// These are the four values of the `relationship` field in
/// `schemas/package.schema.json`, and `SPEC.md` §9.2 requires each outcome to
/// be reported distinctly before world access.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, Ord, PartialEq, PartialOrd, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum DependencyRelationship {
    /// The dependant cannot load without it, and its absence fails closed.
    Required,
    /// The dependant loads with or without it, and orders after it when present.
    Optional,
    /// The two mods cannot load together.
    Incompatible,
    /// The two mods may load together, but the combination is reported.
    Discouraged,
}

#[cfg(test)]
mod tests {
    use super::{
        DependencyRelationship, LifecycleState, ModId, Side, SideSet, Version, VersionRange,
    };

    const RELATIONSHIPS: [DependencyRelationship; 4] = [
        DependencyRelationship::Required,
        DependencyRelationship::Optional,
        DependencyRelationship::Incompatible,
        DependencyRelationship::Discouraged,
    ];

    #[test]
    fn every_schema_relationship_round_trips_as_a_kebab_case_name() {
        for relationship in RELATIONSHIPS {
            let encoded = serde_json::to_string(&relationship).expect("should serialize");
            assert_eq!(
                serde_json::from_str::<DependencyRelationship>(&encoded)
                    .expect("should deserialize"),
                relationship
            );
        }
        assert_eq!(
            serde_json::to_string(&DependencyRelationship::Incompatible).expect("should serialize"),
            "\"incompatible\""
        );
    }

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

    #[test]
    fn the_core_vocabulary_composes_into_one_declaration() {
        let id: ModId = "example_mod".parse().expect("valid id");
        let sides = SideSet::new([Side::Client, Side::IntegratedServer]).expect("two sides");
        let supported: VersionRange = ">=1.2, <2.0".parse().expect("valid range");

        assert_eq!(id.as_str(), "example_mod");
        assert!(supported.matches(&"1.4.0".parse().expect("valid version")));
        assert!(!sides.loads_on(Side::DedicatedServer));
        assert!(LifecycleState::Discovered.can_transition_to(LifecycleState::Validated));
    }
}
