use std::fmt;

use ravel_core::{FailureKind, LifecycleState, ModId, StructuredFailure};
use ravel_host_api::{HostError, HostMod, InterceptionRefused, Occasion, Point, Points, Round};

use crate::lifecycle::LifecycleMachine;

/// Asks eligible mods at `point` in declared order, skipping parked or inactive mods.
///
/// A mod's answering failure ends the round and is retained in [`Round::failure`].
///
/// # Errors
///
/// Returns [`InterceptionError`] for foreign declarations or a point set that cannot be asked.
///
/// # Panics
///
/// Never: ordered owners were found in `mods` while collecting declarations.
pub fn intercept(
    machine: &LifecycleMachine,
    mods: &mut [&mut (dyn HostMod + '_)],
    point: Point,
) -> Result<Round, InterceptionError> {
    let mut points = Points::default();
    for mod_id in machine.order() {
        let Ok(state) = machine.state(mod_id) else {
            continue;
        };
        if !crate::executes_mod_code(state) {
            continue;
        }
        let Some(host) = mods.iter().find(|host| host.id() == mod_id) else {
            continue;
        };
        let Some(declaration) =
            host.interception_declaration()
                .map_err(|source| InterceptionError::Declaration {
                    mod_id: mod_id.clone(),
                    source,
                })?
        else {
            continue;
        };
        if declaration.owner() != mod_id {
            return Err(InterceptionError::Disowned {
                declared: declaration.owner().clone(),
                by: mod_id.clone(),
            });
        }
        points.declare(&declaration);
    }

    let mut asked = |owner: &ModId, occasion: Occasion| {
        let host = mods
            .iter_mut()
            .find(|host| host.id() == owner)
            .expect("only mods that were found above are ordered");
        host.intercept(occasion)
    };
    points
        .ask(point.as_str(), machine.side(), &mut asked)
        .map_err(InterceptionError::Refused)
}

/// Why a point could not be asked at all.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InterceptionError {
    /// A mod refused to declare its interceptions.
    Declaration {
        /// The mod asked for its declaration.
        mod_id: ModId,
        /// The declaration hook's refusal.
        source: HostError,
    },
    /// A mod handed over a declaration belonging to another mod.
    Disowned {
        /// The owner the declaration named.
        declared: ModId,
        /// The mod that produced it.
        by: ModId,
    },
    /// The declared set cannot be asked.
    Refused(InterceptionRefused),
}

impl fmt::Display for InterceptionError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Declaration { source, .. } => source.fmt(formatter),
            Self::Disowned { declared, by } => write!(
                formatter,
                "`{by}` declared interceptions owned by `{declared}`, and a mod answers only for \
                 itself"
            ),
            Self::Refused(refused) => write!(formatter, "{refused}"),
        }
    }
}

impl std::error::Error for InterceptionError {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Declaration { source, .. } => Some(source),
            Self::Disowned { .. } => None,
            Self::Refused(refused) => Some(refused),
        }
    }
}

impl StructuredFailure for InterceptionError {
    fn failure_kind(&self) -> Option<FailureKind> {
        None
    }

    fn lifecycle_state(&self) -> LifecycleState {
        LifecycleState::Running
    }

    fn quotes_untrusted_text(&self) -> bool {
        match self {
            Self::Declaration { source, .. } => source.quotes_untrusted_text(),
            Self::Disowned { .. } => false,
            Self::Refused(refused) => {
                matches!(refused, InterceptionRefused::UnpublishedPoint { .. })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use ravel_core::{LifecycleState, ModId, Side, SideSet, StructuredFailure};
    use ravel_host_api::{
        HostError, HostMod, InterceptionDeclaration, InterceptionRefused, Occasion, Outcome, Point,
    };

    use super::{InterceptionError, intercept};
    use crate::lifecycle::{LifecycleEntry, LifecycleMachine, Requirement};

    // Reverse lexical order ensures ordering assertions depend on the declared edge.
    const GUARD: &str = "zephyr_guard";
    const LEDGER: &str = "alpha_ledger";

    fn id(name: &str) -> ModId {
        name.parse().expect("a legal mod id")
    }

    fn entry(name: &str) -> LifecycleEntry {
        LifecycleEntry {
            mod_id: id(name),
            sides: SideSet::new([Side::IntegratedServer, Side::DedicatedServer])
                .expect("a non-empty side set"),
            requirement: Requirement::Required,
            saved_data_versions: BTreeSet::new(),
        }
    }

    struct Answering {
        id: ModId,
        point: Point,
        answer: Outcome,
        after: Option<ModId>,
        owner_override: Option<ModId>,
        fails: bool,
        refuses_declaration: bool,
        seen: Vec<Occasion>,
    }

    impl Answering {
        fn new(name: &str, point: Point, answer: Outcome) -> Self {
            Self {
                id: id(name),
                point,
                answer,
                after: None,
                owner_override: None,
                fails: false,
                refuses_declaration: false,
                seen: Vec::new(),
            }
        }

        fn after(mut self, earlier: &str) -> Self {
            self.after = Some(id(earlier));
            self
        }

        fn owned_by(mut self, other: &str) -> Self {
            self.owner_override = Some(id(other));
            self
        }

        fn failing(mut self) -> Self {
            self.fails = true;
            self
        }
    }

    impl HostMod for Answering {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _: LifecycleState) -> Result<(), HostError> {
            Ok(())
        }

        fn interception_declaration(&self) -> Result<Option<InterceptionDeclaration>, HostError> {
            if self.refuses_declaration {
                return Err(HostError::Unsupported {
                    mod_id: self.id.clone(),
                    state: LifecycleState::Running,
                });
            }
            let owner = self
                .owner_override
                .clone()
                .unwrap_or_else(|| self.id.clone());
            let mut declared = InterceptionDeclaration::new(owner);
            declared.answer_at(self.point.as_str());
            if let Some(earlier) = &self.after {
                declared.after(earlier);
            }
            Ok(Some(declared))
        }

        fn intercept(&mut self, occasion: Occasion) -> Result<Outcome, HostError> {
            self.seen.push(occasion);
            if self.fails {
                return Err(HostError::Refused {
                    mod_id: self.id.clone(),
                    state: LifecycleState::Running,
                    detail: "the ledger is closed".to_owned(),
                });
            }
            Ok(self.answer)
        }
    }

    struct Quiet {
        id: ModId,
    }

    impl HostMod for Quiet {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _: LifecycleState) -> Result<(), HostError> {
            Ok(())
        }
    }

    fn running(names: &[&str]) -> LifecycleMachine {
        let mut machine = LifecycleMachine::new(
            Side::IntegratedServer,
            names.iter().copied().map(entry).collect(),
        )
        .expect("a legal plan");
        for state in [
            LifecycleState::Validated,
            LifecycleState::Resolved,
            LifecycleState::Loaded,
            LifecycleState::Constructed,
            LifecycleState::Registered,
            LifecycleState::Running,
        ] {
            machine.advance_all(state).expect("a legal step");
        }
        machine
    }

    #[test]
    fn interception_declaration_refusal_is_not_absence() {
        let mut host = Answering::new(GUARD, Point::WorldIsSaved, Outcome::Proceed);
        host.refuses_declaration = true;
        assert_eq!(
            intercept(&running(&[GUARD]), &mut [&mut host], Point::WorldIsSaved),
            Err(InterceptionError::Declaration {
                mod_id: id(GUARD),
                source: HostError::Unsupported {
                    mod_id: id(GUARD),
                    state: LifecycleState::Running,
                },
            })
        );
        assert_eq!(host.seen, Vec::<Occasion>::new());
    }

    #[test]
    fn the_declared_edge_decides_who_is_asked_first() {
        let mut guard = Answering::new(GUARD, Point::WorldIsSaved, Outcome::Proceed);
        let mut ledger = Answering::new(LEDGER, Point::WorldIsSaved, Outcome::Proceed).after(GUARD);
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut ledger, &mut guard];
        let machine = running(&[GUARD, LEDGER]);

        let round = intercept(&machine, &mut mods, Point::WorldIsSaved).expect("a legal set");

        assert_eq!(round.asked(), [id(GUARD), id(LEDGER)]);
        assert_eq!(round.outcome(), Outcome::Proceed);
        assert_eq!(round.point(), Point::WorldIsSaved);
    }

    #[test]
    fn a_refusal_ends_the_round_before_the_later_mod_is_asked() {
        let mut guard = Answering::new(GUARD, Point::WorldIsSaved, Outcome::Refuse);
        let mut ledger = Answering::new(LEDGER, Point::WorldIsSaved, Outcome::Proceed).after(GUARD);
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut ledger, &mut guard];
        let machine = running(&[GUARD, LEDGER]);

        let round = intercept(&machine, &mut mods, Point::WorldIsSaved).expect("a legal set");

        assert_eq!(round.outcome(), Outcome::Refuse);
        assert_eq!(round.asked(), [id(GUARD)]);
        assert!(
            ledger.seen.is_empty(),
            "the ledger answered after a refusal"
        );
        assert_eq!(guard.seen.len(), 1);
        assert_eq!(guard.seen[0].point(), Point::WorldIsSaved);
        assert_eq!(guard.seen[0].side(), Side::IntegratedServer);
    }

    #[test]
    fn a_mod_that_declared_another_point_is_not_asked_here() {
        let mut elsewhere = Answering::new(GUARD, Point::BlockIsBroken, Outcome::Refuse);
        let mut here = Answering::new(LEDGER, Point::WorldIsSaved, Outcome::Proceed);
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut elsewhere, &mut here];
        let machine = running(&[GUARD, LEDGER]);

        let round = intercept(&machine, &mut mods, Point::WorldIsSaved).expect("a legal set");

        assert_eq!(round.asked(), [id(LEDGER)]);
        assert_eq!(round.outcome(), Outcome::Proceed);
        assert_eq!(elsewhere.seen, Vec::<Occasion>::new());
    }

    #[test]
    fn a_mod_that_intercepts_nothing_is_not_asked() {
        let mut quiet = Quiet { id: id(GUARD) };
        let mut answering = Answering::new(LEDGER, Point::WorldIsSaved, Outcome::Proceed);
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut quiet, &mut answering];
        let machine = running(&[GUARD, LEDGER]);

        let round = intercept(&machine, &mut mods, Point::WorldIsSaved).expect("a legal set");

        assert_eq!(round.asked(), [id(LEDGER)]);
    }

    #[test]
    fn a_mod_that_is_not_running_is_not_asked() {
        let mut guard = Answering::new(GUARD, Point::WorldIsSaved, Outcome::Refuse);
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut guard];
        let mut machine = LifecycleMachine::new(Side::IntegratedServer, vec![entry(GUARD)])
            .expect("a legal plan");
        machine
            .advance_all(LifecycleState::Validated)
            .expect("a legal step");

        let round = intercept(&machine, &mut mods, Point::WorldIsSaved).expect("a legal set");

        assert_eq!(round.asked(), []);
        assert_eq!(round.outcome(), Outcome::Proceed);
        assert_eq!(guard.seen, Vec::<Occasion>::new());
    }

    #[test]
    fn a_mod_declaring_interceptions_for_another_mod_is_refused() {
        let mut liar = Answering::new(GUARD, Point::WorldIsSaved, Outcome::Refuse).owned_by(LEDGER);
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut liar];
        let machine = running(&[GUARD]);

        let refused = intercept(&machine, &mut mods, Point::WorldIsSaved)
            .expect_err("a mod answers for itself");

        assert_eq!(
            refused,
            InterceptionError::Disowned {
                declared: id(LEDGER),
                by: id(GUARD),
            }
        );
        assert!(refused.to_string().contains(LEDGER));
        assert!(refused.to_string().contains(GUARD));
        assert_eq!(refused.lifecycle_state(), LifecycleState::Running);
        assert_eq!(refused.failure_kind(), None);
        assert!(!refused.quotes_untrusted_text());
    }

    #[test]
    fn a_mod_that_fails_while_answering_ends_the_round_and_is_attributed() {
        let mut guard = Answering::new(GUARD, Point::WorldIsSaved, Outcome::Proceed).failing();
        let mut ledger = Answering::new(LEDGER, Point::WorldIsSaved, Outcome::Proceed).after(GUARD);
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut ledger, &mut guard];
        let machine = running(&[GUARD, LEDGER]);

        let round = intercept(&machine, &mut mods, Point::WorldIsSaved).expect("a legal set");

        assert_eq!(
            round.outcome(),
            Outcome::Refuse,
            "a host that could not finish asking was told it may proceed"
        );
        let failure = round.failure().expect("the failure is carried out");
        assert_eq!(failure.mod_id(), &id(GUARD));
        assert!(ledger.seen.is_empty(), "the round continued past a failure");
    }

    #[test]
    fn a_declared_point_with_no_implementation_is_unsupported() {
        struct Silent {
            id: ModId,
        }

        impl HostMod for Silent {
            fn id(&self) -> &ModId {
                &self.id
            }

            fn enter(&mut self, _: LifecycleState) -> Result<(), HostError> {
                Ok(())
            }

            fn interception_declaration(
                &self,
            ) -> Result<Option<InterceptionDeclaration>, HostError> {
                let mut declared = InterceptionDeclaration::new(self.id.clone());
                declared.answer_at(Point::WorldIsSaved.as_str());
                Ok(Some(declared))
            }
        }

        let mut silent = Silent { id: id(GUARD) };
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut silent];
        let machine = running(&[GUARD]);

        let round = intercept(&machine, &mut mods, Point::WorldIsSaved).expect("a legal set");

        assert_eq!(round.outcome(), Outcome::Refuse);
        let failure = round.failure().expect("the default refuses loudly");
        assert!(
            matches!(failure, HostError::Unsupported { .. }),
            "{failure:?}"
        );
    }

    #[test]
    fn two_mods_replacing_one_outcome_refuse_the_round() {
        struct Replacing {
            id: ModId,
        }

        impl HostMod for Replacing {
            fn id(&self) -> &ModId {
                &self.id
            }

            fn enter(&mut self, _: LifecycleState) -> Result<(), HostError> {
                Ok(())
            }

            fn interception_declaration(
                &self,
            ) -> Result<Option<InterceptionDeclaration>, HostError> {
                let mut declared = InterceptionDeclaration::new(self.id.clone());
                declared.replace_at(Point::WorldIsSaved.as_str());
                Ok(Some(declared))
            }

            fn intercept(&mut self, _: Occasion) -> Result<Outcome, HostError> {
                Ok(Outcome::Proceed)
            }
        }

        let mut first = Replacing { id: id(GUARD) };
        let mut second = Replacing { id: id(LEDGER) };
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut first, &mut second];
        let machine = running(&[GUARD, LEDGER]);

        let refused = intercept(&machine, &mut mods, Point::WorldIsSaved)
            .expect_err("two replacements are a conflict");

        assert!(
            matches!(
                refused,
                InterceptionError::Refused(InterceptionRefused::ContestedPoint { .. })
            ),
            "{refused:?}"
        );
        assert!(std::error::Error::source(&refused).is_some());
    }
}
