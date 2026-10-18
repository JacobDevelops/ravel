use std::fmt;

use ravel_core::{
    ContentKind, FailureKind, LifecycleState, ModId, RegistryKey, Side, StructuredFailure,
};

/// Why the lifecycle refused an operation.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum LifecycleError {
    /// Two entries of one plan claim the same mod id.
    DuplicateIdentity {
        /// The contested identity.
        mod_id: ModId,
    },
    /// The plan holds no such mod.
    UnknownMod {
        /// The requested identity.
        mod_id: ModId,
    },
    /// The `SPEC.md` §14 table refuses this pair of states.
    IllegalTransition {
        /// The mod that would have moved.
        mod_id: ModId,
        /// The state it is in.
        from: LifecycleState,
        /// The state it was asked to enter.
        to: LifecycleState,
    },
    /// A move to `failed` arrived without structured attribution.
    AttributionRequired,
    /// The set halted after a failure that followed world mutation.
    Halted {
        /// The mod that would have moved.
        mod_id: ModId,
        /// The state it is in.
        from: LifecycleState,
        /// The state it was asked to enter.
        to: LifecycleState,
    },
    /// Mod code would have run on a side the mod does not declare.
    SideMismatch {
        /// The mod that would have run.
        mod_id: ModId,
        /// The state it is in.
        from: LifecycleState,
        /// The side the set is running on.
        side: Side,
    },
    /// A mod would have started running before the set finished registering.
    RegistrationBarrierOpen {
        /// The mod that would have started.
        mod_id: ModId,
        /// The mod still holding the barrier open.
        blocking: ModId,
    },
    /// A registry identifier was claimed after the set closed the barrier.
    RegistrationBarrierClosed {
        /// The claiming mod.
        mod_id: ModId,
        /// The identifier it claimed.
        identifier: RegistryKey,
    },
    /// A registry identifier was claimed by a mod that is not constructed.
    RegistrationWindow {
        /// The claiming mod.
        mod_id: ModId,
        /// The identifier it claimed.
        identifier: RegistryKey,
        /// The state the claim arrived in.
        state: LifecycleState,
    },
    /// A mod claimed a registry key in another identity's namespace.
    ForeignRegistryClaim {
        /// The claimed key.
        key: RegistryKey,
        /// The claiming mod.
        mod_id: ModId,
    },
    /// Two mods claim one registry identifier.
    ConflictingRegistryClaim {
        /// The contested identifier.
        identifier: RegistryKey,
        /// The mod that already owns it.
        owner: ModId,
        /// The mod that claimed it second.
        claimant: ModId,
    },
    /// A mod described claimed content as a kind the identifier does not name.
    MismatchedContentBehaviour {
        /// The identifier that was described.
        identifier: RegistryKey,
        /// The kind the description describes.
        described: ContentKind,
    },
    /// A mod is not running, so the set cannot open a world.
    WorldAccessBlocked {
        /// The mod that is not running.
        mod_id: ModId,
        /// The state it is in.
        state: LifecycleState,
    },
    /// The set failed after mutating the world, so it never regains access.
    WorldAccessHalted {
        /// The mod whose failure halted the set.
        mod_id: ModId,
    },
    /// A save was requested after a failure that followed world mutation.
    PartialStateRefused {
        /// The mod whose failure halted the set.
        mod_id: ModId,
    },
}

impl fmt::Display for LifecycleError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::DuplicateIdentity { mod_id } => write!(
                formatter,
                "two packages claim the mod id `{mod_id}`, so nothing can own its registrations \
                 or saved data"
            ),
            Self::UnknownMod { mod_id } => {
                write!(formatter, "the load plan holds no mod `{mod_id}`")
            }
            Self::IllegalTransition { mod_id, from, to } => {
                fmt_illegal_transition(formatter, mod_id, *from, *to)
            }
            Self::AttributionRequired => formatter.write_str(
                "a move to `failed` carries structured attribution, so it is recorded by `fail`",
            ),
            Self::Halted { mod_id, from, to } => fmt_halted(formatter, mod_id, *from, *to),
            Self::SideMismatch { mod_id, from, side } => {
                fmt_side_mismatch(formatter, mod_id, *from, *side)
            }
            Self::RegistrationBarrierOpen { mod_id, blocking } => write!(
                formatter,
                "`{mod_id}` cannot start running until the set has registered, and `{blocking}` \
                 has not"
            ),
            Self::RegistrationBarrierClosed { mod_id, identifier } => write!(
                formatter,
                "the set closed the registration barrier, so `{mod_id}` cannot claim \
                 `{identifier}`"
            ),
            Self::RegistrationWindow {
                mod_id,
                identifier,
                state,
            } => write!(
                formatter,
                "`{mod_id}` can claim `{identifier}` only while constructed, but it is `{}`",
                state.as_str()
            ),
            Self::ForeignRegistryClaim { key, mod_id } => fmt_foreign_claim(formatter, key, mod_id),
            Self::ConflictingRegistryClaim {
                identifier,
                owner,
                claimant,
            } => fmt_registry_conflict(formatter, identifier, owner, claimant),
            Self::MismatchedContentBehaviour {
                identifier,
                described,
            } => write!(
                formatter,
                "`{identifier}` was described as {described} content, which is not the kind it \
                 names"
            ),
            Self::WorldAccessBlocked { mod_id, state } => write!(
                formatter,
                "`{mod_id}` is `{}` rather than running, so the set cannot open a world",
                state.as_str()
            ),
            Self::WorldAccessHalted { mod_id } => write!(
                formatter,
                "`{mod_id}` failed after the world was mutated, so the set cannot open a world"
            ),
            Self::PartialStateRefused { mod_id } => write!(
                formatter,
                "`{mod_id}` failed after the world was mutated, so partial state is not saved"
            ),
        }
    }
}

fn fmt_registry_conflict(
    formatter: &mut fmt::Formatter<'_>,
    identifier: &RegistryKey,
    owner: &ModId,
    claimant: &ModId,
) -> fmt::Result {
    write!(
        formatter,
        "`{claimant}` claimed the registry identifier `{identifier}`, which `{owner}` already owns"
    )
}

fn fmt_foreign_claim(
    formatter: &mut fmt::Formatter<'_>,
    key: &RegistryKey,
    mod_id: &ModId,
) -> fmt::Result {
    write!(
        formatter,
        "`{mod_id}` cannot claim `{key}` in another mod's namespace"
    )
}

fn fmt_illegal_transition(
    formatter: &mut fmt::Formatter<'_>,
    mod_id: &ModId,
    from: LifecycleState,
    to: LifecycleState,
) -> fmt::Result {
    write!(
        formatter,
        "`{mod_id}` cannot move from `{}` to `{}`",
        from.as_str(),
        to.as_str()
    )
}

fn fmt_halted(
    formatter: &mut fmt::Formatter<'_>,
    mod_id: &ModId,
    from: LifecycleState,
    to: LifecycleState,
) -> fmt::Result {
    write!(
        formatter,
        "the set failed after mutating the world, so `{mod_id}` cannot move from `{}` to \
         `{}`; only stopping remains",
        from.as_str(),
        to.as_str()
    )
}

fn fmt_side_mismatch(
    formatter: &mut fmt::Formatter<'_>,
    mod_id: &ModId,
    from: LifecycleState,
    side: Side,
) -> fmt::Result {
    write!(
        formatter,
        "`{mod_id}` does not declare the `{}` side, so its code never loads here and it \
         stays `{}`",
        side.as_str(),
        from.as_str()
    )
}

impl std::error::Error for LifecycleError {}

impl StructuredFailure for LifecycleError {
    fn failure_kind(&self) -> Option<FailureKind> {
        match self {
            Self::DuplicateIdentity { .. } => Some(FailureKind::DuplicateIdentity),
            Self::SideMismatch { .. } => Some(FailureKind::SideMismatch),
            Self::ConflictingRegistryClaim { .. } => Some(FailureKind::ConflictingRegistryClaim),
            Self::UnknownMod { .. }
            | Self::IllegalTransition { .. }
            | Self::AttributionRequired
            | Self::Halted { .. }
            | Self::RegistrationBarrierOpen { .. }
            | Self::RegistrationBarrierClosed { .. }
            | Self::RegistrationWindow { .. }
            | Self::ForeignRegistryClaim { .. }
            | Self::MismatchedContentBehaviour { .. }
            | Self::WorldAccessBlocked { .. }
            | Self::WorldAccessHalted { .. }
            | Self::PartialStateRefused { .. } => None,
        }
    }

    fn lifecycle_state(&self) -> LifecycleState {
        match self {
            Self::IllegalTransition { from, .. }
            | Self::Halted { from, .. }
            | Self::SideMismatch { from, .. } => *from,
            Self::RegistrationWindow { state, .. } | Self::WorldAccessBlocked { state, .. } => {
                *state
            }
            Self::ConflictingRegistryClaim { .. }
            | Self::ForeignRegistryClaim { .. }
            | Self::MismatchedContentBehaviour { .. } => LifecycleState::Constructed,
            Self::RegistrationBarrierOpen { .. } | Self::RegistrationBarrierClosed { .. } => {
                LifecycleState::Registered
            }
            Self::WorldAccessHalted { .. } | Self::PartialStateRefused { .. } => {
                LifecycleState::Failed
            }
            Self::DuplicateIdentity { .. }
            | Self::UnknownMod { .. }
            | Self::AttributionRequired => LifecycleState::Discovered,
        }
    }

    fn quotes_untrusted_text(&self) -> bool {
        false
    }
}
