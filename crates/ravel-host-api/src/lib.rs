#![forbid(unsafe_code)]

//! Provisional host contract whose mechanism is selected by ADG-2.

use std::fmt;

use ravel_core::{FailureKind, LifecycleState, ModId, RegistryKey, StructuredFailure};

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

    /// [`Self::Unsupported`] is written entirely by this crate. [`Self::Refused`]
    /// interpolates a sentence a mod wrote, and a mod can put a filesystem path
    /// or a token in it, which is the only message in Ravel's whole pre-world
    /// path that nobody in this workspace authored. Both are published through a
    /// redaction policy either way; saying which is which is how a report can
    /// tell a player that the words came from their mod rather than from Ravel.
    fn quotes_untrusted_text(&self) -> bool {
        matches!(self, Self::Refused { .. })
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

    /// Claims the registry identifiers this mod owns.
    ///
    /// The loader calls this once, while the mod is
    /// [`LifecycleState::Constructed`] and before the set advances to
    /// [`LifecycleState::Registered`]. That is the whole registration window:
    /// `SPEC.md` §14 makes identifiers immutable after one barrier for the load,
    /// so a mod that claims nothing here claims nothing ever.
    ///
    /// Registration is a separate method rather than a `Constructed` case
    /// inside [`HostMod::enter`] because it is the only step that hands the mod
    /// something. The phases differ in what they offer, and a signature that
    /// passed every phase a capability it cannot use would be describing a
    /// contract Ravel does not have.
    ///
    /// The default claims nothing, which is what a mod that registers no
    /// content does.
    ///
    /// # Errors
    ///
    /// Returns an error when the mod declines to register. A refused claim
    /// reaches the mod as a [`RegistrationRefused`] and it is the mod's to
    /// convert: a mod that treats a conflict as fatal returns
    /// [`HostError::Refused`], and a mod that has an alternative may carry on.
    fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), HostError> {
        let _ = registrar;
        Ok(())
    }
}

/// What the loader offers a mod during the registration window.
///
/// This is the mod-facing half of the registration barrier. The loader-facing
/// half is `ravel-loader`'s lifecycle machine, which owns the claim table and
/// decides every refusal; this trait exists so a mod can reach that decision
/// without naming the loader, which `docs/architecture/dependency-rules.md`
/// forbids it from depending on.
///
/// The trait is object-safe and takes `&mut dyn Registrar`, so the mechanism
/// that produced the mod value is still ADG-2's question and still unanswered
/// here.
pub trait Registrar {
    /// Claims `key` for the mod currently registering.
    ///
    /// # Errors
    ///
    /// Returns [`RegistrationRefused`] when the identifier is already owned, or
    /// when the barrier has closed.
    fn claim(&mut self, key: RegistryKey) -> Result<(), RegistrationRefused>;

    /// Returns the identifiers this mod has already claimed, in claim order.
    fn claimed(&self) -> &[RegistryKey];
}

/// Why the loader refused a registration claim.
///
/// The loader's own error type carries more than a mod can act on — which mod
/// owns the conflicting identifier, and what lifecycle state the claimant was
/// in. This is the part a mod can do something about, so it is what crosses the
/// boundary.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum RegistrationRefused {
    /// Another mod already owns the identifier.
    Conflict {
        /// The contested identifier.
        key: RegistryKey,
        /// The mod that claimed it first.
        owner: ModId,
    },
    /// The registration window is not open, so no claim can be accepted.
    ///
    /// A mod reaching this has called [`Registrar::claim`] outside the call the
    /// loader made to [`HostMod::register`], by holding the registrar past its
    /// window or by claiming after the set closed the barrier.
    Closed {
        /// The identifier that arrived too late.
        key: RegistryKey,
    },
}

impl RegistrationRefused {
    /// The identifier the refusal is about.
    #[must_use]
    pub const fn key(&self) -> &RegistryKey {
        match self {
            Self::Conflict { key, .. } | Self::Closed { key } => key,
        }
    }
}

impl fmt::Display for RegistrationRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Conflict { key, owner } => {
                write!(formatter, "`{key}` is already registered by `{owner}`")
            }
            Self::Closed { key } => write!(
                formatter,
                "`{key}` arrived after the registration barrier closed"
            ),
        }
    }
}

impl std::error::Error for RegistrationRefused {}

impl StructuredFailure for RegistrationRefused {
    /// A contested identifier is `SPEC.md` §9.2's conflicting registry claim.
    /// A late claim is not: nothing is contested, the window is simply shut, so
    /// it reports no kind rather than borrowing one.
    fn failure_kind(&self) -> Option<FailureKind> {
        match self {
            Self::Conflict { .. } => Some(FailureKind::ConflictingRegistryClaim),
            Self::Closed { .. } => None,
        }
    }

    fn lifecycle_state(&self) -> LifecycleState {
        LifecycleState::Constructed
    }

    /// Every word of both messages is written by this crate, and both
    /// interpolate identifiers that `ravel-core` has already validated against
    /// a charset holding no path separator and no whitespace.
    fn quotes_untrusted_text(&self) -> bool {
        false
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{
        ContentKind, FailureKind, LifecycleState, ModId, Redaction, RegistryKey, StructuredFailure,
    };

    use super::{HostError, HostMod, Registrar, RegistrationRefused};

    /// A registrar that accepts anything nobody has claimed yet.
    ///
    /// The real refusals belong to the loader's claim table. This one exists so
    /// the trait's own contract — that a refusal reaches the mod, and that the
    /// mod decides what it means — can be tested without a loader.
    #[derive(Default)]
    struct Table {
        claimed: Vec<RegistryKey>,
        owned: Vec<(RegistryKey, ModId)>,
        closed: bool,
    }

    impl Registrar for Table {
        fn claim(&mut self, key: RegistryKey) -> Result<(), RegistrationRefused> {
            if self.closed {
                return Err(RegistrationRefused::Closed { key });
            }
            if let Some((_, owner)) = self.owned.iter().find(|(owned, _)| owned == &key) {
                return Err(RegistrationRefused::Conflict {
                    key,
                    owner: owner.clone(),
                });
            }
            self.claimed.push(key);
            Ok(())
        }

        fn claimed(&self) -> &[RegistryKey] {
            &self.claimed
        }
    }

    fn key(kind: ContentKind, id: &str) -> RegistryKey {
        RegistryKey::new(kind, id.parse().expect("a legal content id"))
    }

    /// A mod that claims one block and one item, and gives up on a conflict.
    struct Registering {
        id: ModId,
    }

    impl HostMod for Registering {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
            Ok(())
        }

        fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), HostError> {
            for kind in [ContentKind::Block, ContentKind::Item] {
                registrar
                    .claim(key(kind, "example_mod:copper"))
                    .map_err(|refused| HostError::Refused {
                        mod_id: self.id.clone(),
                        state: LifecycleState::Constructed,
                        detail: refused.to_string(),
                    })?;
            }
            Ok(())
        }
    }

    fn registering() -> Registering {
        Registering {
            id: "example_mod".parse().expect("valid id"),
        }
    }

    #[test]
    fn a_mod_that_registers_nothing_needs_no_implementation() {
        let mut table = Table::default();
        let mut host = counting();

        host.register(&mut table)
            .expect("the default claims nothing");
        assert!(table.claimed().is_empty());
    }

    #[test]
    fn one_mod_claims_the_same_name_in_two_registries_without_conflicting() {
        let mut table = Table::default();

        registering()
            .register(&mut table)
            .expect("a block and an item are different claims");
        assert_eq!(
            table.claimed(),
            [
                key(ContentKind::Block, "example_mod:copper"),
                key(ContentKind::Item, "example_mod:copper"),
            ]
        );
    }

    #[test]
    fn a_contested_identifier_reaches_the_mod_naming_who_owns_it() {
        let owner: ModId = "other_mod".parse().expect("valid id");
        let mut table = Table {
            owned: vec![(key(ContentKind::Block, "example_mod:copper"), owner.clone())],
            ..Table::default()
        };

        let error = registering()
            .register(&mut table)
            .expect_err("the block is taken");
        assert_eq!(error.mod_id().as_str(), "example_mod");
        assert_eq!(error.state(), LifecycleState::Constructed);
        assert!(error.to_string().contains("other_mod"), "{error}");

        // The mod gave up on the first refusal, so it never reached the item.
        assert!(table.claimed().is_empty());
    }

    #[test]
    fn a_claim_after_the_barrier_is_refused_as_late_rather_than_as_contested() {
        let mut table = Table {
            closed: true,
            ..Table::default()
        };
        let contested = key(ContentKind::Block, "example_mod:copper");

        let refused = table
            .claim(contested.clone())
            .expect_err("the barrier shut");
        assert_eq!(refused, RegistrationRefused::Closed { key: contested });
        assert!(
            refused
                .to_string()
                .contains("after the registration barrier")
        );
    }

    #[test]
    fn only_a_contested_claim_reports_a_dependency_graph_outcome() {
        let contested = key(ContentKind::Recipe, "example_mod:smelting/copper");
        let conflict = RegistrationRefused::Conflict {
            key: contested.clone(),
            owner: "other_mod".parse().expect("valid id"),
        };
        let closed = RegistrationRefused::Closed {
            key: contested.clone(),
        };

        assert_eq!(
            conflict.failure_kind(),
            Some(FailureKind::ConflictingRegistryClaim)
        );
        assert_eq!(closed.failure_kind(), None);
        assert_eq!(conflict.key(), &contested);
        assert_eq!(closed.key(), &contested);
        assert_eq!(conflict.lifecycle_state(), LifecycleState::Constructed);

        // Both messages are this crate's own words about validated identifiers.
        assert!(!conflict.quotes_untrusted_text());
        assert!(!closed.quotes_untrusted_text());
        assert_eq!(conflict.diagnostic().redaction(), Redaction::Raw);
        assert_ne!(conflict, closed);
    }

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
        assert!(refused.diagnostic().text().contains("constructed"));
    }

    #[test]
    fn only_the_message_a_mod_wrote_is_reported_as_quoting_untrusted_text() {
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
        // holds no path and no token; the report has to say where it came from.
        assert!(refused.quotes_untrusted_text());
        assert!(!unsupported.quotes_untrusted_text());

        // Neither vouches for its own text: only a policy produces `applied`.
        assert_eq!(refused.diagnostic().redaction(), Redaction::Raw);
        assert_eq!(unsupported.diagnostic().redaction(), Redaction::Raw);
        assert_eq!(unsupported.diagnostic().text(), unsupported.to_string());
    }
}
