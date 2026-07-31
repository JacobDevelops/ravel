#![forbid(unsafe_code)]

//! Provisional host contract whose mechanism is selected by ADG-2.

use std::fmt;

use ravel_core::{
    DiagnosticMessage, FailureKind, LifecycleState, ModId, Redaction, StructuredFailure,
};

/// Why a mod refused the lifecycle step the loader asked it to take.
///
/// The loader attributes a failure to a mod and a phase, which is what
/// `SPEC.md` §9.1 requires of every report, so both travel with the error. A
/// loader driving several mods through [`HostMod::enter`] in one pass has no
/// other way to say which of them refused.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HostError {
    /// The mod ran and declined, naming why.
    Refused {
        /// The mod that declined.
        mod_id: ModId,
        /// The state the mod was asked to enter.
        state: LifecycleState,
        /// What the mod said, in the mod's own words and so not yet redacted.
        detail: String,
    },
    /// The mod does not implement the step at all.
    Unsupported {
        /// The mod that has no implementation for the step.
        mod_id: ModId,
        /// The state the mod was asked to enter.
        state: LifecycleState,
    },
}

impl HostError {
    /// The mod this failure is attributed to.
    #[must_use]
    pub const fn mod_id(&self) -> &ModId {
        match self {
            Self::Refused { mod_id, .. } | Self::Unsupported { mod_id, .. } => mod_id,
        }
    }

    /// The state the mod was asked to enter when it failed.
    #[must_use]
    pub const fn state(&self) -> LifecycleState {
        match self {
            Self::Refused { state, .. } | Self::Unsupported { state, .. } => *state,
        }
    }
}

impl fmt::Display for HostError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Refused {
                mod_id,
                state,
                detail,
            } => write!(
                formatter,
                "`{mod_id}` refused to enter `{}`: {detail}",
                state.name()
            ),
            Self::Unsupported { mod_id, state } => write!(
                formatter,
                "`{mod_id}` does not implement `{}`",
                state.name()
            ),
        }
    }
}

impl std::error::Error for HostError {}

impl StructuredFailure for HostError {
    /// A mod declining a step is not one of `SPEC.md` §9.2's dependency-graph
    /// outcomes, so this reports no kind rather than borrowing one.
    fn failure_kind(&self) -> Option<FailureKind> {
        None
    }

    fn lifecycle_state(&self) -> LifecycleState {
        self.state()
    }

    /// [`Self::Unsupported`] is written entirely by this crate and holds no path
    /// and no secret, so it keeps the default [`Redaction::Applied`].
    /// [`Self::Refused`] interpolates a sentence a mod wrote, and a mod can put
    /// a filesystem path or a token in it, so it is tagged [`Redaction::Raw`]
    /// for a policy to handle. Claiming otherwise would be the tag lying about
    /// the one message nobody in this crate wrote.
    fn diagnostic(&self) -> DiagnosticMessage {
        DiagnosticMessage {
            text: self.to_string(),
            redaction: match self {
                Self::Refused { .. } => Redaction::Raw,
                Self::Unsupported { .. } => Redaction::Applied,
            },
        }
    }
}

/// What the loader needs from one installed mod.
///
/// This is the whole contract, deliberately. How a mod's code arrives in the
/// process is ADG-2's question and ADG-2 is deferred to M3, so nothing here
/// names a binary format, a dynamic library, a guest runtime, or a foreign
/// calling convention: a host mod is a safe Rust value the loader owns and
/// drives, and the mechanism that produces that value is decided later without
/// changing this trait.
pub trait HostMod: Send {
    /// The canonical identity this mod was validated under.
    fn id(&self) -> &ModId;

    /// Takes the mod into `state`.
    ///
    /// The loader calls this once per state, in lifecycle order, and stops at
    /// the first refusal.
    ///
    /// # Errors
    ///
    /// Returns an error when the mod declines the step or does not implement it.
    fn enter(&mut self, state: LifecycleState) -> Result<(), HostError>;
}

#[cfg(test)]
mod tests {
    use ravel_core::{LifecycleState, ModId, Redaction, StructuredFailure};

    use super::{HostError, HostMod};

    struct Counting {
        id: ModId,
        entered: Vec<LifecycleState>,
    }

    impl HostMod for Counting {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, state: LifecycleState) -> Result<(), HostError> {
            if state == LifecycleState::Registered {
                return Err(HostError::Refused {
                    mod_id: self.id.clone(),
                    state,
                    detail: "the registry barrier had already closed".to_owned(),
                });
            }
            self.entered.push(state);
            Ok(())
        }
    }

    fn counting() -> Counting {
        Counting {
            id: "example_mod".parse().expect("valid id"),
            entered: Vec::new(),
        }
    }

    #[test]
    fn the_loader_drives_a_mod_through_states_and_stops_at_the_first_refusal() {
        let mut host: Box<dyn HostMod> = Box::new(counting());

        let id: ModId = "example_mod".parse().expect("valid id");

        assert_eq!(host.id().as_str(), "example_mod");
        assert_eq!(host.enter(LifecycleState::Loaded), Ok(()));
        assert_eq!(
            host.enter(LifecycleState::Registered),
            Err(HostError::Refused {
                mod_id: id,
                state: LifecycleState::Registered,
                detail: "the registry barrier had already closed".to_owned(),
            })
        );
    }

    #[test]
    fn every_refusal_names_the_mod_and_the_phase_it_happened_in() {
        let id: ModId = "example_mod".parse().expect("valid id");
        let refused = HostError::Refused {
            mod_id: id.clone(),
            state: LifecycleState::Constructed,
            detail: "no service was declared".to_owned(),
        };
        let unsupported = HostError::Unsupported {
            mod_id: id.clone(),
            state: LifecycleState::Stopping,
        };

        assert!(refused.to_string().contains("no service was declared"));
        assert!(refused.to_string().contains("example_mod"));
        assert!(unsupported.to_string().contains("does not implement"));
        assert_eq!(refused.mod_id(), &id);
        assert_eq!(unsupported.state(), LifecycleState::Stopping);
        assert_ne!(refused, unsupported);
    }

    #[test]
    fn a_refusal_reports_the_phase_it_reached_and_no_dependency_kind() {
        let refused = HostError::Refused {
            mod_id: "example_mod".parse().expect("valid id"),
            state: LifecycleState::Constructed,
            detail: "no service was declared".to_owned(),
        };

        assert_eq!(refused.failure_kind(), None);
        assert_eq!(refused.lifecycle_state(), LifecycleState::Constructed);
        assert!(refused.diagnostic().text.contains("constructed"));
    }

    #[test]
    fn only_text_this_crate_wrote_is_reported_as_redacted() {
        let refused = HostError::Refused {
            mod_id: "example_mod".parse().expect("valid id"),
            state: LifecycleState::Loaded,
            detail: "could not open the world directory".to_owned(),
        };
        let unsupported = HostError::Unsupported {
            mod_id: "example_mod".parse().expect("valid id"),
            state: LifecycleState::Loaded,
        };

        // `detail` is a sentence a mod wrote, so nothing here can promise it
        // holds no path and no token; the tag has to say so.
        assert_eq!(refused.diagnostic().redaction, Redaction::Raw);
        assert_eq!(unsupported.diagnostic().redaction, Redaction::Applied);
        assert_eq!(unsupported.diagnostic().text, unsupported.to_string());
    }
}
