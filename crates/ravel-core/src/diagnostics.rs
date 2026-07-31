use serde::{Deserialize, Serialize};

use crate::{LifecycleState, ModId, Version};

/// A failure any Ravel crate can hand to the surface that reports it.
///
/// `SPEC.md` §9.1 requires a failure report to carry a lifecycle phase and
/// redacted detail, and §9.2 requires the dependency-graph outcomes to be named
/// distinctly before world access. This trait is how the crate that formats
/// those reports gets all three from any error without matching on each error
/// type, and it lives in `ravel-core` because more than one crate raises
/// failures that a single pre-world report has to render together.
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

    /// The failure text, ready to publish.
    ///
    /// The default tags the rendered message [`Redaction::Applied`], which an
    /// implementation may only rely on when every one of its messages is
    /// written to hold no filesystem path and no credential. Anything that
    /// interpolates untrusted text overrides this and tags it
    /// [`Redaction::Raw`] for a policy to handle.
    fn diagnostic(&self) -> DiagnosticMessage {
        DiagnosticMessage {
            text: self.to_string(),
            redaction: Redaction::Applied,
        }
    }
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

/// Whether a redaction policy has run over a diagnostic message.
///
/// `SPEC.md` §9.1 requires reports to redact authentication tokens, secrets,
/// and unnecessarily identifying filesystem paths. `ravel-core` carries the tag
/// and deliberately owns none of the policy: the rules live with the crate that
/// formats reports, and the tag is what lets a publishing surface refuse text
/// that nothing has redacted.
#[derive(Clone, Copy, Debug, Deserialize, Eq, Hash, PartialEq, Serialize)]
#[serde(rename_all = "kebab-case")]
pub enum Redaction {
    /// No policy has run; the text may still hold secrets or identifying paths.
    Raw,
    /// A redaction policy has run and the text is safe to publish.
    Applied,
}

/// Human-readable failure detail together with its redaction state.
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct DiagnosticMessage {
    /// The message text.
    pub text: String,
    /// Whether a redaction policy has been applied to `text`.
    pub redaction: Redaction,
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
///     ModId, Redaction,
/// };
///
/// let failure = FailureAttribution {
///     mod_id: "example_mod".parse()?,
///     version: Some("1.4.0".parse()?),
///     dependency_chain: vec!["example_pack".parse()?],
///     state: LifecycleState::Resolved,
///     kind: FailureKind::MissingRequiredDependency,
///     surface: None,
///     environment: FailureEnvironment {
///         ravel: "0.1.0".parse()?,
///         minecraft: "26.0.0".to_owned(),
///         platform: "linux".to_owned(),
///         architecture: "x86_64".to_owned(),
///     },
///     message: DiagnosticMessage {
///         text: "requires `example_library`".to_owned(),
///         redaction: Redaction::Applied,
///     },
/// };
///
/// assert!(failure.state.can_transition_to(LifecycleState::Failed));
/// # Ok::<(), Box<dyn std::error::Error>>(())
/// ```
#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
pub struct FailureAttribution {
    /// The mod the failure is attributed to.
    pub mod_id: ModId,
    /// The version that mod declared, absent when the failure preceded metadata
    /// validation and no version was ever accepted.
    pub version: Option<Version>,
    /// The dependency path that reached the failing mod, ordered from the mod
    /// that required it. Empty when the failure was not dependency-mediated.
    pub dependency_chain: Vec<ModId>,
    /// The lifecycle state the mod was in when it failed.
    pub state: LifecycleState,
    /// What went wrong.
    pub kind: FailureKind,
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
        Redaction,
    };
    use crate::{LifecycleState, ModId, Version};

    const KINDS: [FailureKind; 7] = [
        FailureKind::MissingRequiredDependency,
        FailureKind::IncompatibleVersion,
        FailureKind::DependencyCycle,
        FailureKind::SideMismatch,
        FailureKind::DuplicateIdentity,
        FailureKind::UnsupportedTarget,
        FailureKind::ConflictingRegistryClaim,
    ];

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
            mod_id: "example_mod".parse().expect("valid id"),
            version,
            dependency_chain: vec!["example_pack".parse().expect("valid id")],
            state: LifecycleState::Resolved,
            kind: FailureKind::MissingRequiredDependency,
            surface: None,
            environment: environment(),
            message: DiagnosticMessage {
                text: "requires `example_library`".to_owned(),
                redaction: Redaction::Applied,
            },
        }
    }

    #[test]
    fn every_spec_failure_kind_round_trips_as_a_kebab_case_name() {
        for kind in KINDS {
            let encoded = serde_json::to_string(&kind).expect("kind should serialize");
            assert_eq!(
                serde_json::from_str::<FailureKind>(&encoded).expect("kind should deserialize"),
                kind
            );
        }
        assert_eq!(
            serde_json::to_string(&FailureKind::ConflictingRegistryClaim)
                .expect("kind should serialize"),
            "\"conflicting-registry-claim\""
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
    fn a_message_carries_whether_a_policy_has_redacted_it() {
        let raw = DiagnosticMessage {
            text: "/home/player/instance/mods".to_owned(),
            redaction: Redaction::Raw,
        };
        assert_ne!(raw.redaction, Redaction::Applied);

        let encoded = serde_json::to_string(&raw).expect("message should serialize");
        assert_eq!(
            encoded,
            "{\"text\":\"/home/player/instance/mods\",\"redaction\":\"raw\"}"
        );
        assert_eq!(
            serde_json::from_str::<DiagnosticMessage>(&encoded)
                .expect("message should deserialize"),
            raw
        );
    }

    #[test]
    fn an_attribution_round_trips_with_and_without_a_declared_version() {
        for version in [
            Some("1.4.0".parse::<Version>().expect("valid version")),
            None,
        ] {
            let failure = attribution(version);
            let encoded = serde_json::to_string(&failure).expect("failure should serialize");
            assert_eq!(
                serde_json::from_str::<FailureAttribution>(&encoded)
                    .expect("failure should deserialize"),
                failure
            );
        }
    }

    #[test]
    fn an_attribution_carries_every_facet_the_spec_requires() {
        let mut failure = attribution(Some("1.4.0".parse().expect("valid version")));
        failure.surface = Some(FailureSurface::Registry("example_mod:block".to_owned()));

        assert_eq!(
            failure.mod_id,
            "example_mod".parse::<ModId>().expect("valid id")
        );
        assert_eq!(failure.version, Some("1.4.0".parse().expect("valid")));
        assert_eq!(
            failure.dependency_chain,
            vec!["example_pack".parse::<ModId>().expect("valid id")]
        );
        assert_eq!(failure.state, LifecycleState::Resolved);
        assert!(failure.state.can_transition_to(LifecycleState::Failed));
        assert_eq!(failure.kind, FailureKind::MissingRequiredDependency);
        assert_eq!(
            failure.surface,
            Some(FailureSurface::Registry("example_mod:block".to_owned()))
        );
        assert_eq!(failure.environment, environment());
        assert_eq!(failure.message.redaction, Redaction::Applied);

        let encoded = serde_json::to_string(&failure).expect("failure should serialize");
        assert_eq!(
            serde_json::from_str::<FailureAttribution>(&encoded)
                .expect("failure should deserialize"),
            failure
        );
    }
}
