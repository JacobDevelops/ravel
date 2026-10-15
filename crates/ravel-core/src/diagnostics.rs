use semver::Version;
use serde::{Deserialize, Serialize};

use crate::identity::ModId;
use crate::states::LifecycleState;

/// A failure with a graph outcome, lifecycle phase, and diagnostic detail.
pub trait StructuredFailure: std::error::Error {
    /// Returns the dependency-graph outcome, or `None` for other failures.
    fn failure_kind(&self) -> Option<FailureKind>;

    /// Returns the failure phase, defaulting to pre-validation discovery.
    fn lifecycle_state(&self) -> LifecycleState {
        LifecycleState::Discovered
    }

    /// Returns whether the message quotes text not authored by Ravel.
    fn quotes_untrusted_text(&self) -> bool {
        false
    }

    /// Returns raw failure text; publishing requires a [`RedactionPolicy`].
    fn diagnostic(&self) -> DiagnosticMessage {
        DiagnosticMessage::raw(self.to_string())
    }
}

/// Redacts secrets and identifying paths before diagnostic publication.
pub trait RedactionPolicy {
    /// Returns `text` with everything the policy removes taken out.
    fn apply(&self, text: &str) -> String;
}

/// A dependency, side, or registry failure category.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FailureKind {
    /// A required dependency is absent from the installed mod set.
    MissingRequiredDependency,
    /// A dependency is installed at a version outside the declared range.
    IncompatibleVersion,
    /// The dependency graph contains a cycle.
    DependencyCycle,
    /// A mod was asked to load on a side it does not declare.
    SideMismatch,
    /// Two packages claim the same mod id.
    DuplicateIdentity,
    /// Two mods claim the same registry identifier.
    ConflictingRegistryClaim,
}

impl FailureKind {
    /// Every failure kind in declaration order.
    pub const ALL: [Self; 6] = [
        Self::MissingRequiredDependency,
        Self::IncompatibleVersion,
        Self::DependencyCycle,
        Self::SideMismatch,
        Self::DuplicateIdentity,
        Self::ConflictingRegistryClaim,
    ];

    /// Returns the failure kind's canonical serialized name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::MissingRequiredDependency => "missing-required-dependency",
            Self::IncompatibleVersion => "incompatible-version",
            Self::DependencyCycle => "dependency-cycle",
            Self::SideMismatch => "side-mismatch",
            Self::DuplicateIdentity => "duplicate-identity",
            Self::ConflictingRegistryClaim => "conflicting-registry-claim",
        }
    }
}

/// Whether a redaction policy has run over a diagnostic message.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Redaction {
    /// No policy has run; the text may still hold secrets or identifying paths.
    Raw,
    /// A redaction policy has run and the text is safe to publish.
    Applied,
}

/// Diagnostic text whose publishable state can only be earned by running a policy.
///
/// Serialization is output-only so documents cannot claim that redaction ran.
///
/// # Examples
///
/// ```
/// use ravel_core::{DiagnosticMessage, Redaction, RedactionPolicy};
///
/// struct DropEverything;
///
/// impl RedactionPolicy for DropEverything {
///     fn apply(&self, _: &str) -> String {
///         "[redacted]".to_owned()
///     }
/// }
///
/// let raw = DiagnosticMessage::raw("could not read /home/player/token.txt");
/// assert_eq!(raw.redaction(), Redaction::Raw);
/// assert!(!raw.is_publishable());
///
/// let published = DiagnosticMessage::redacted(&DropEverything, raw.text());
/// assert_eq!(published.text(), "[redacted]");
/// assert!(published.is_publishable());
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct DiagnosticMessage {
    text: String,
    redaction: Redaction,
}

impl DiagnosticMessage {
    /// Records text no policy has run over.
    #[must_use]
    pub fn raw(text: impl Into<String>) -> Self {
        Self {
            text: text.into(),
            redaction: Redaction::Raw,
        }
    }

    /// Runs `policy` over `text` and records the result as publishable.
    #[must_use]
    pub fn redacted<P>(policy: &P, text: &str) -> Self
    where
        P: RedactionPolicy + ?Sized,
    {
        Self {
            text: policy.apply(text),
            redaction: Redaction::Applied,
        }
    }

    /// Returns the message text.
    #[must_use]
    pub fn text(&self) -> &str {
        &self.text
    }

    /// Returns whether a policy has run over the text.
    #[must_use]
    pub const fn redaction(&self) -> Redaction {
        self.redaction
    }

    /// Returns whether the text may be shown to a player.
    #[must_use]
    pub const fn is_publishable(&self) -> bool {
        matches!(self.redaction, Redaction::Applied)
    }
}

/// A named hook, event, registry, or packet involved in a failure.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum FailureSurface {
    /// A loader or game hook.
    Hook(String),
    /// An event dispatch.
    Event(String),
    /// A registry operation.
    Registry(String),
    /// A network packet.
    Packet(String),
}

/// The Ravel and host environment; Minecraft version and target names are opaque.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FailureEnvironment {
    /// The Ravel version that reported the failure.
    pub ravel: Version,
    /// The Minecraft version the loader was hosted by.
    pub minecraft: String,
    /// The running platform, matching the package manifest `platform` value.
    pub platform: String,
    /// The running architecture, matching the package manifest.
    pub architecture: String,
}

/// A failure's mod, dependency path, lifecycle phase, surface, environment, and detail.
///
/// # Examples
///
/// ```
/// use ravel_core::{
///     DiagnosticMessage, FailureAttribution, FailureEnvironment, FailureKind, LifecycleState,
///     ModId, RedactionPolicy,
/// };
///
/// struct KeepEverything;
///
/// impl RedactionPolicy for KeepEverything {
///     fn apply(&self, text: &str) -> String {
///         text.to_owned()
///     }
/// }
///
/// let failure = FailureAttribution {
///     mod_id: Some("example_mod".parse()?),
///     version: Some("1.4.0".parse()?),
///     dependency_chain: vec!["example_pack".parse()?],
///     state: LifecycleState::Resolved,
///     kind: Some(FailureKind::MissingRequiredDependency),
///     surface: None,
///     environment: FailureEnvironment {
///         ravel: "0.1.0".parse()?,
///         minecraft: "26.0.0".to_owned(),
///         platform: "linux".to_owned(),
///         architecture: "x86_64".to_owned(),
///     },
///     message: DiagnosticMessage::redacted(&KeepEverything, "requires `example_library`"),
/// };
///
/// assert!(failure.state.can_transition_to(LifecycleState::Failed));
/// assert!(failure.message.is_publishable());
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct FailureAttribution {
    /// The mod identity, absent if metadata validation accepted none.
    pub mod_id: Option<ModId>,
    /// The mod version, absent if metadata validation accepted none.
    pub version: Option<Version>,
    /// The dependency path that reached the failing mod, ordered from the mod
    /// that required it. Empty when the failure was not dependency-mediated.
    pub dependency_chain: Vec<ModId>,
    /// The lifecycle state the mod was in when it failed.
    pub state: LifecycleState,
    /// The dependency-graph outcome, absent for other failures.
    pub kind: Option<FailureKind>,
    /// The surface involved, absent for a failure that reached no surface.
    pub surface: Option<FailureSurface>,
    /// Where the loader was running.
    pub environment: FailureEnvironment,
    /// Redactable detail.
    pub message: DiagnosticMessage,
}

#[cfg(test)]
mod tests {
    use semver::Version;

    use super::{
        DiagnosticMessage, FailureAttribution, FailureEnvironment, FailureKind, FailureSurface,
        Redaction, RedactionPolicy, StructuredFailure,
    };
    use crate::states::LifecycleState;

    struct Verbatim;

    impl RedactionPolicy for Verbatim {
        fn apply(&self, text: &str) -> String {
            text.to_owned()
        }
    }

    #[derive(Debug)]
    struct Bare;

    impl std::fmt::Display for Bare {
        fn fmt(&self, formatter: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            formatter.write_str("nothing worked")
        }
    }

    impl std::error::Error for Bare {}

    impl StructuredFailure for Bare {
        fn failure_kind(&self) -> Option<FailureKind> {
            None
        }
    }

    fn environment() -> FailureEnvironment {
        FailureEnvironment {
            ravel: "0.1.0".parse().expect("valid version"),
            minecraft: "26.0.0".to_owned(),
            platform: "linux".to_owned(),
            architecture: "x86_64".to_owned(),
        }
    }

    fn attribution(version: Option<Version>) -> FailureAttribution {
        FailureAttribution {
            mod_id: Some("example_mod".parse().expect("valid id")),
            version,
            dependency_chain: vec!["example_pack".parse().expect("valid id")],
            state: LifecycleState::Resolved,
            kind: Some(FailureKind::MissingRequiredDependency),
            surface: None,
            environment: environment(),
            message: DiagnosticMessage::redacted(&Verbatim, "requires `example_library`"),
        }
    }

    #[test]
    fn every_spec_failure_kind_round_trips_as_its_reported_name() {
        assert_eq!(FailureKind::ALL.len(), 6);
        for kind in FailureKind::ALL {
            let encoded = serde_json::to_string(&kind).expect("kind should serialize");
            assert_eq!(encoded, format!("\"{}\"", kind.name()));
            assert_eq!(
                serde_json::from_str::<FailureKind>(&encoded).expect("kind should deserialize"),
                kind
            );
        }
        assert_eq!(
            FailureKind::ConflictingRegistryClaim.name(),
            "conflicting-registry-claim"
        );
    }

    #[test]
    fn every_spec_failure_surface_round_trips_with_the_name_it_carries() {
        for surface in [
            FailureSurface::Hook("world_load".to_owned()),
            FailureSurface::Event("player_tick".to_owned()),
            FailureSurface::Registry("example_mod:block".to_owned()),
            FailureSurface::Packet("example_mod:sync".to_owned()),
        ] {
            let encoded = serde_json::to_string(&surface).expect("surface should serialize");
            assert_eq!(
                serde_json::from_str::<FailureSurface>(&encoded)
                    .expect("surface should deserialize"),
                surface
            );
        }
        assert_eq!(
            serde_json::to_string(&FailureSurface::Registry("example_mod:block".to_owned()))
                .expect("surface should serialize"),
            "{\"registry\":\"example_mod:block\"}"
        );
    }

    #[test]
    fn a_message_is_raw_until_a_policy_has_run_over_it() {
        let raw = DiagnosticMessage::raw("/home/player/instance/mods");
        assert_eq!(raw.redaction(), Redaction::Raw);
        assert!(!raw.is_publishable());
        assert_eq!(raw.text(), "/home/player/instance/mods");
        assert_eq!(
            serde_json::to_string(&raw).expect("message should serialize"),
            "{\"text\":\"/home/player/instance/mods\",\"redaction\":\"raw\"}"
        );
    }

    #[test]
    fn only_running_a_policy_produces_a_publishable_message() {
        struct Truncating;

        impl RedactionPolicy for Truncating {
            fn apply(&self, text: &str) -> String {
                text.rsplit('/').next().unwrap_or_default().to_owned()
            }
        }

        let published = DiagnosticMessage::redacted(&Truncating, "/home/player/instance/mods");
        assert_eq!(published.text(), "mods");
        assert_eq!(published.redaction(), Redaction::Applied);
        assert!(published.is_publishable());
    }

    #[test]
    fn a_failure_reports_raw_text_and_no_borrowed_kind_by_default() {
        let bare = Bare;
        assert_eq!(bare.failure_kind(), None);
        assert_eq!(bare.lifecycle_state(), LifecycleState::Discovered);
        assert!(!bare.quotes_untrusted_text());
        assert_eq!(bare.diagnostic().text(), "nothing worked");
        assert_eq!(bare.diagnostic().redaction(), Redaction::Raw);
    }

    #[test]
    fn an_attribution_serializes_with_and_without_a_declared_version() {
        for version in [
            Some("1.4.0".parse::<Version>().expect("valid version")),
            None,
        ] {
            let encoded =
                serde_json::to_string(&attribution(version)).expect("failure should serialize");
            assert!(
                encoded.contains("\"missing-required-dependency\""),
                "{encoded}"
            );
            assert!(encoded.contains("\"redaction\":\"applied\""), "{encoded}");
        }
    }

    #[test]
    fn an_attribution_reports_no_kind_for_a_failure_that_is_not_a_graph_outcome() {
        let mut failure = attribution(None);
        failure.kind = None;

        let encoded = serde_json::to_string(&failure).expect("failure should serialize");
        assert!(encoded.contains("\"kind\":null"), "{encoded}");
    }
}
