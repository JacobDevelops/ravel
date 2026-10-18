use std::fmt;

use ravel_core::{FailureKind, LifecycleState, ModId, RegistryKey, Side, StructuredFailure};
use ravel_host_api::GameEvent;
use ravel_network::{Direction, PacketId};

/// Why an event or a command reached no mod.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum DispatchRefused {
    /// The lifecycle refused callback failure attribution.
    Lifecycle(crate::LifecycleError),
    /// A mod failure halted the set.
    Halted {
        /// The owner whose failure halted the set.
        by: ModId,
    },
    /// The executing event side cannot belong to this runtime.
    EventForeignSide {
        /// The executing side requested by the caller.
        claimed: Side,
        /// The side the set is running on.
        running: Side,
    },
    /// The command's claimed side disagrees with the runtime's world side.
    CommandForeignSide {
        /// The command that was refused.
        key: RegistryKey,
        /// The execution side claimed by the caller.
        claimed: Side,
        /// The runtime's world execution side.
        running: Side,
    },
    /// The event cannot happen on the executing side.
    EventWrongSide {
        /// The event requested by the caller.
        event: GameEvent,
        /// The executing side requested by the caller.
        side: Side,
    },
    /// The set is not in a state that reacts.
    NotReacting {
        /// The first non-reacting audience member in resolved order.
        mod_id: ModId,
        /// The state it is in.
        state: LifecycleState,
    },
    /// No mod claimed the command's root literal.
    Unclaimed {
        /// The key nobody owns.
        key: RegistryKey,
    },
    /// The supplied key identifies content other than a command.
    NotACommand {
        /// The non-command key that cannot be routed.
        key: RegistryKey,
    },
    /// The delivery's owner is not in a state that reacts.
    OwnerNotReacting {
        /// The owner the delivery was routed to.
        mod_id: ModId,
        /// The state it is in.
        state: LifecycleState,
    },
    /// The owner's declared sides do not include the executing side.
    OwnerWrongSide {
        /// The owner the delivery was routed to.
        mod_id: ModId,
    },
    /// The delivery's own side field disagrees with the side the set runs on.
    PacketForeignSide {
        /// The contract that was refused.
        id: PacketId,
        /// The side the delivery claimed to arrive on.
        claimed: Side,
        /// The side the set is running on.
        running: Side,
    },
    /// The packet travels a way this side is not an end of.
    PacketWrongSide {
        /// The contract that was refused.
        id: PacketId,
        /// The way it travels.
        direction: Direction,
        /// The side it arrived on.
        side: Side,
    },
    /// The requested owner is not a member of the lifecycle plan.
    Absent {
        /// The owner absent from the lifecycle plan.
        mod_id: ModId,
    },
}

impl DispatchRefused {
    /// Returns the identified owner of the refused operation.
    #[must_use]
    pub fn mod_id(&self) -> Option<&ModId> {
        match self {
            Self::Lifecycle(error) => error.mod_id(),
            Self::Halted { by } => Some(by),
            Self::NotReacting { mod_id, .. }
            | Self::OwnerNotReacting { mod_id, .. }
            | Self::OwnerWrongSide { mod_id }
            | Self::Absent { mod_id } => Some(mod_id),
            Self::Unclaimed { key }
            | Self::NotACommand { key }
            | Self::CommandForeignSide { key, .. } => Some(key.id().namespace()),
            Self::PacketForeignSide { id, .. } | Self::PacketWrongSide { id, .. } => {
                Some(id.owner())
            }
            Self::EventForeignSide { .. } | Self::EventWrongSide { .. } => None,
        }
    }

    fn lifecycle_error(&self) -> Option<&crate::LifecycleError> {
        match self {
            Self::Lifecycle(error) => Some(error),
            Self::Halted { .. }
            | Self::EventForeignSide { .. }
            | Self::CommandForeignSide { .. }
            | Self::EventWrongSide { .. }
            | Self::NotReacting { .. }
            | Self::Unclaimed { .. }
            | Self::NotACommand { .. }
            | Self::OwnerNotReacting { .. }
            | Self::OwnerWrongSide { .. }
            | Self::PacketForeignSide { .. }
            | Self::PacketWrongSide { .. }
            | Self::Absent { .. } => None,
        }
    }
}

impl fmt::Display for DispatchRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lifecycle(error) => error.fmt(formatter),
            Self::Halted { by } => write!(formatter, "the set halted after `{by}` failed"),
            Self::EventForeignSide { claimed, running } => write!(
                formatter,
                "event execution claimed the `{}` side and the set is running as `{}`",
                claimed.as_str(),
                running.as_str()
            ),
            Self::CommandForeignSide {
                key,
                claimed,
                running,
            } => write!(
                formatter,
                "`{key}` claimed the `{}` side and commands execute as `{}`",
                claimed.as_str(),
                running.as_str()
            ),
            Self::EventWrongSide { event, side } => {
                write!(formatter, "{event:?} does not happen on {side:?}")
            }
            Self::NotReacting { mod_id, state } => write!(
                formatter,
                "the set is `{}` at `{mod_id}`, which does not react",
                state.as_str()
            ),
            Self::Unclaimed { key } => write!(formatter, "no mod claimed `{key}`"),
            Self::NotACommand { key } => write!(formatter, "`{key}` is not a command"),
            Self::OwnerNotReacting { mod_id, state } => write!(
                formatter,
                "`{mod_id}` owns this delivery and is `{}`, which does not react",
                state.as_str()
            ),
            Self::OwnerWrongSide { mod_id } => {
                write!(formatter, "`{mod_id}` does not load on this side")
            }
            Self::PacketForeignSide {
                id,
                claimed,
                running,
            } => write!(
                formatter,
                "`{id}` arrived claiming the `{}` side and the set is running as `{}`",
                claimed.as_str(),
                running.as_str()
            ),
            Self::PacketWrongSide {
                id,
                direction,
                side,
            } => write!(
                formatter,
                "`{id}` travels {direction} and the `{}` side is not an end of that",
                side.as_str()
            ),
            Self::Absent { mod_id } => {
                write!(formatter, "the lifecycle plan has no owner `{mod_id}`")
            }
        }
    }
}

impl std::error::Error for DispatchRefused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        self.lifecycle_error()
            .map(|error| -> &(dyn std::error::Error + 'static) { error })
    }
}

impl StructuredFailure for DispatchRefused {
    /// Dispatch refusals are not dependency-graph failures.
    fn failure_kind(&self) -> Option<FailureKind> {
        self.lifecycle_error()
            .and_then(StructuredFailure::failure_kind)
    }

    fn lifecycle_state(&self) -> LifecycleState {
        match self {
            Self::Lifecycle(error) => error.lifecycle_state(),
            Self::Halted { .. } => LifecycleState::Failed,
            Self::NotReacting { state, .. } | Self::OwnerNotReacting { state, .. } => *state,
            Self::Unclaimed { .. }
            | Self::EventForeignSide { .. }
            | Self::CommandForeignSide { .. }
            | Self::EventWrongSide { .. }
            | Self::NotACommand { .. }
            | Self::OwnerWrongSide { .. }
            | Self::PacketForeignSide { .. }
            | Self::PacketWrongSide { .. }
            | Self::Absent { .. } => LifecycleState::Running,
        }
    }

    /// Messages contain only validated identifiers and loader-owned text.
    fn quotes_untrusted_text(&self) -> bool {
        matches!(self, Self::Lifecycle(error) if error.quotes_untrusted_text())
    }
}
