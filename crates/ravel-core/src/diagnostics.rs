use serde::{Deserialize, Serialize};

use crate::{LifecycleState, ModId, Version};

/// A failure any Ravel crate can hand to the surface that reports it.
///
/// `SPEC.md` §9.1 requires a failure report to carry a lifecycle phase and
/// redacted detail, and §9.2 requires the dependency-graph outcomes to be named
/// distinctly before world access. This trait is how the crate that formats
/// those reports gets all of that from any error without matching on each error
/// type, and it lives in `ravel-core` because every stage of the pipeline —
/// manifest, integrity, compatibility, target, resolution, lifecycle, and the
/// mod itself — raises failures that a single pre-world report has to render
/// together. One `match` arm covers all of them.
pub trait StructuredFailure: std::error::Error {
    /// The `SPEC.md` §9.2 outcome this failure is, when it is one of them.
    ///
    /// That list describes the dependency graph. A failure that happens earlier
    /// or later than the graph honestly reports no kind rather than borrowing
    /// one.
    fn failure_kind(&self) -> Option<FailureKind>;

    /// The state the subject is in when this failure is raised.
    ///
    /// The default is [`LifecycleState::Discovered`], which is the honest
    /// answer for anything raised before a package was admitted: nothing has
    /// run. A failure that happens later overrides it with the phase it
    /// actually reached.
    fn lifecycle_state(&self) -> LifecycleState {
        LifecycleState::Discovered
    }

    /// Whether the rendered text quotes words this project did not write.
    ///
    /// A report is published through a redaction policy either way, so this
    /// changes no rule the policy applies. It is the one fact about a message
    /// that the text itself does not show: whether the sentence a player is
    /// about to read came from Ravel or from a mod, which is what lets a report
    /// attribute it to the right author.
    fn quotes_untrusted_text(&self) -> bool {
        false
    }

    /// The failure text, before any policy has run over it.
    ///
    /// The message is [`Redaction::Raw`] because nothing has vetted it yet:
    /// [`Redaction::Applied`] is a claim only a [`RedactionPolicy`] can earn,
    /// and no implementation of this trait is allowed to make it on a policy's
    /// behalf.
    fn diagnostic(&self) -> DiagnosticMessage {
        DiagnosticMessage::raw(self.to_string())
    }
}

/// The rules a report's text is held to before it is published.
///
/// `SPEC.md` §9.1 requires reports to redact authentication tokens, secrets,
/// and unnecessarily identifying filesystem paths. `ravel-core` names that
/// obligation and deliberately owns none of the rules: it is the crate every
/// other one depends on, so a policy written here would be one nobody could
/// replace. The rules belong to the crate that publishes reports, and this
/// trait is how a [`DiagnosticMessage`] proves one of them ran.
pub trait RedactionPolicy {
    /// Returns `text` with everything the policy removes taken out.
    fn apply(&self, text: &str) -> String;
}

/// What went wrong, from the set `SPEC.md` §9.2 requires Ravel to report before
/// it grants world access.
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
    /// A package does not support the running platform or architecture.
    UnsupportedTarget,
    /// Two mods claim the same registry identifier.
    ConflictingRegistryClaim,
}

impl FailureKind {
    /// Every outcome, in the order `SPEC.md` §9.2 lists them.
    pub const ALL: [Self; 7] = [
        Self::MissingRequiredDependency,
        Self::IncompatibleVersion,
        Self::DependencyCycle,
        Self::SideMismatch,
        Self::DuplicateIdentity,
        Self::UnsupportedTarget,
        Self::ConflictingRegistryClaim,
    ];

    /// The name a report writes this outcome under, matching its serialized
    /// form.
    ///
    /// A pre-world report shows the outcome next to the message, and it uses
    /// this rather than the derived `Debug` spelling so that one outcome has
    /// one user-facing name.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::MissingRequiredDependency => "missing-required-dependency",
            Self::IncompatibleVersion => "incompatible-version",
            Self::DependencyCycle => "dependency-cycle",
            Self::SideMismatch => "side-mismatch",
            Self::DuplicateIdentity => "duplicate-identity",
            Self::UnsupportedTarget => "unsupported-target",
            Self::ConflictingRegistryClaim => "conflicting-registry-claim",
        }
    }
}

/// Whether a redaction policy has run over a diagnostic message.
///
/// `SPEC.md` §9.1 requires reports to redact authentication tokens, secrets,
/// and unnecessarily identifying filesystem paths. `ravel-core` carries the tag
/// and owns none of the policy: the rules live with the crate that formats
/// reports, and the tag is what lets a publishing surface refuse text that
/// nothing has redacted.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Redaction {
    /// No policy has run; the text may still hold secrets or identifying paths.
    Raw,
    /// A redaction policy has run and the text is safe to publish.
    Applied,
}

/// Human-readable failure detail together with its redaction state.
///
/// The fields are private and the two constructors are the whole story:
/// [`DiagnosticMessage::raw`] records text nothing has vetted, and
/// [`DiagnosticMessage::redacted`] is the only way to obtain
/// [`Redaction::Applied`]. It earns the tag rather than accepting it, because
/// it runs the [`RedactionPolicy`] itself instead of being told one ran.
///
/// The type is serialized but never deserialized. A report is published, not
/// parsed, and reading text back out of a document would let the document
/// assert a policy had run over it.
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
    ///
    /// This is the only way to produce [`Redaction::Applied`] anywhere in the
    /// workspace, which is what makes the tag mean something: a caller cannot
    /// claim its text is safe, it can only hand the text to a policy and keep
    /// what comes back.
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

/// The Minecraft-facing surface a failure occurred on (`SPEC.md` §9.1).
///
/// Each variant carries the name of the specific surface, which is a mod-facing
/// identifier rather than anything a Minecraft version defines.
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

/// The Ravel and host environment a failure happened in (`SPEC.md` §9.1).
///
/// The Minecraft version, platform, and architecture are opaque strings because
/// that is what `schemas/package.schema.json` and `schemas/evidence.schema.json`
/// declare them to be. A Minecraft release is not required to be
/// semver-shaped, `compatibility.minecraft` is an exact-string list rather than
/// a range, and no version-neutral crate may encode a Minecraft version
/// grammar.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FailureEnvironment {
    /// The Ravel version that reported the failure.
    pub ravel: Version,
    /// The Minecraft version the loader was hosted by.
    pub minecraft: String,
    /// The running platform, matching the package manifest `platform` value.
    pub platform: String,
    /// The running architecture, matching the package manifest `architecture`
    /// value.
    pub architecture: String,
}

/// Structured attribution for one failure (`SPEC.md` §9.1).
///
/// The fields are exactly what §9.1 requires a report to attribute a failure
/// to: mod identity and version, dependency chain, lifecycle phase, the hook,
/// event, registry or packet involved, the Minecraft and Ravel versions, and
/// the platform and target architecture. The detail text carries its own
/// redaction state, so a publishing surface can refuse anything unredacted
/// without knowing the policy.
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
    /// The mod the failure is attributed to, absent when the failure preceded
    /// metadata validation and no identity was ever accepted.
    ///
    /// A manifest that will not parse claims nothing, so there is no identity
    /// to name. Naming one anyway would put a mod id in a report that no
    /// package on the player's disk ever declared.
    pub mod_id: Option<ModId>,
    /// The version that mod declared, absent when the failure preceded metadata
    /// validation and no version was ever accepted.
    pub version: Option<Version>,
    /// The dependency path that reached the failing mod, ordered from the mod
    /// that required it. Empty when the failure was not dependency-mediated.
    pub dependency_chain: Vec<ModId>,
    /// The lifecycle state the mod was in when it failed.
    pub state: LifecycleState,
    /// What went wrong, absent when the failure is not one of the `SPEC.md`
    /// §9.2 dependency-graph outcomes.
    ///
    /// This mirrors [`StructuredFailure::failure_kind`] exactly. A manifest
    /// that will not parse and a file that does not match its digest both fail
    /// earlier than the graph, and an attribution that had to name a kind would
    /// force them to borrow one that misdescribes them.
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
    use super::{
        DiagnosticMessage, FailureAttribution, FailureEnvironment, FailureKind, FailureSurface,
        Redaction, RedactionPolicy, StructuredFailure,
    };
    use crate::{LifecycleState, ModId, Version};

    /// A policy that removes nothing, so a test can produce a publishable
    /// message without depending on the crate that owns the real rules.
    struct Verbatim;

    impl RedactionPolicy for Verbatim {
        fn apply(&self, text: &str) -> String {
            text.to_owned()
        }
    }

    /// The smallest failure that exercises every trait default.
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
        assert_eq!(FailureKind::ALL.len(), 7);
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
    fn an_attribution_carries_every_facet_the_spec_requires() {
        let mut failure = attribution(Some("1.4.0".parse().expect("valid version")));
        failure.surface = Some(FailureSurface::Registry("example_mod:block".to_owned()));

        assert_eq!(
            failure.mod_id,
            Some("example_mod".parse::<ModId>().expect("valid id"))
        );
        assert_eq!(failure.version, Some("1.4.0".parse().expect("valid")));
        assert_eq!(
            failure.dependency_chain,
            vec!["example_pack".parse::<ModId>().expect("valid id")]
        );
        assert_eq!(failure.state, LifecycleState::Resolved);
        assert!(failure.state.can_transition_to(LifecycleState::Failed));
        assert_eq!(failure.kind, Some(FailureKind::MissingRequiredDependency));
        assert_eq!(
            failure.surface,
            Some(FailureSurface::Registry("example_mod:block".to_owned()))
        );
        assert_eq!(failure.environment, environment());
        assert!(failure.message.is_publishable());
    }

    #[test]
    fn an_attribution_reports_no_kind_for_a_failure_that_is_not_a_graph_outcome() {
        let mut failure = attribution(None);
        failure.kind = None;

        let encoded = serde_json::to_string(&failure).expect("failure should serialize");
        assert!(encoded.contains("\"kind\":null"), "{encoded}");
    }
}
