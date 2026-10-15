use std::fmt;

use ravel_core::{FailureKind, LifecycleState, ModId, StructuredFailure};
use ravel_host_api::{HostMod, InterceptionRefused, Occasion, Point, Points, Round};

use crate::lifecycle::LifecycleMachine;

/// Asks every mod that answers at `point`, in the order the set resolves to.
///
/// This is the caller `ravel_host_api::Points` existed without. Until it was
/// here a mod could be given a way to declare an answer and nothing would ever
/// ask for one, which is the same shape as a packet nothing carries: the
/// declaration was real and the round was hypothetical.
///
/// Every step belongs to somebody else. Each mod says what it answers at, and
/// [`Points`] decides the order and refuses a set that cannot be asked; what
/// this function owns is which mods are eligible, and asking them.
///
/// # Which mods are asked
///
/// Only mods in a state that runs mod code, and only those that declared this
/// point. A mod parked on this side or not yet running is not asked and is not
/// an error: a point is a moment in a running game, not a barrier every mod must
/// reach.
///
/// # Errors
///
/// Returns [`InterceptionError::Disowned`] when a mod hands over a declaration
/// naming another mod, and [`InterceptionError::Refused`] when the set cannot be
/// asked at all — an unpublished point, two replacements, or a cycle.
///
/// A mod that *fails* while answering is not an error here. It ends the round
/// and is carried out in [`Round::failure`], because the mods before it were
/// asked and a report that lost that would describe a different run.
///
/// # Panics
///
/// Never. The order is built from the declarations of mods this function has
/// already found among `mods`, so the lookup while asking is asking about a mod
/// the first pass held.
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
        let Some(declaration) = host.interception_declaration() else {
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
            Self::Disowned { .. } => None,
            Self::Refused(refused) => Some(refused),
        }
    }
}

impl StructuredFailure for InterceptionError {
    /// Neither variant is one of `SPEC.md` §9.2's dependency-graph outcomes. A
    /// round runs long after the graph was accepted.
    fn failure_kind(&self) -> Option<FailureKind> {
        None
    }

    /// A round is asked while the set runs.
    fn lifecycle_state(&self) -> LifecycleState {
        LifecycleState::Running
    }

    /// Every sentence here is written in this workspace. A point name a mod
    /// wrote reaches the refusal `ravel-host-api` renders, which says so itself.
    fn quotes_untrusted_text(&self) -> bool {
        match self {
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

    /// The two fixture identities, chosen to sort against the declared order.
    ///
    /// `zephyr_guard` is declared to be asked *first* and sorts *last*; the
    /// order this module resolves is therefore only ever right because the
    /// declared edge was read. With the edge dropped, `precedence` falls back to
    /// the lexicographically smallest order and answers `alpha_ledger` first, so
    /// every ordering assertion below fails rather than passing by luck.
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

    /// A mod that answers at one point and records every occasion it saw.
    struct Answering {
        id: ModId,
        point: Point,
        answer: Outcome,
        after: Option<ModId>,
        owner_override: Option<ModId>,
        fails: bool,
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

        fn interception_declaration(&self) -> Option<InterceptionDeclaration> {
            let owner = self
                .owner_override
                .clone()
                .unwrap_or_else(|| self.id.clone());
            let mut declared = InterceptionDeclaration::new(owner);
            declared.answer_at(self.point.as_str());
            if let Some(earlier) = &self.after {
                declared.after(earlier);
            }
            Some(declared)
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

    /// A mod that intercepts nothing, which is what most mods are.
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

    /// Drives `names` to `Running` on an integrated server.
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

    /// The order is the declared one, not the one the names sort into.
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

    /// The first refusal ends the round, and the mod after it is never entered.
    ///
    /// Both halves are checked: the round says who it asked, and the mod itself
    /// says whether its own body ran. A round that reported correctly while
    /// entering everybody would pass the first and fail the second.
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

    /// A mod that declared another point is not asked at this one.
    #[test]
    fn a_mod_that_declared_another_point_is_not_asked_here() {
        let mut elsewhere = Answering::new(GUARD, Point::BlockIsBroken, Outcome::Refuse);
        let mut here = Answering::new(LEDGER, Point::WorldIsSaved, Outcome::Proceed);
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut elsewhere, &mut here];
        let machine = running(&[GUARD, LEDGER]);

        let round = intercept(&machine, &mut mods, Point::WorldIsSaved).expect("a legal set");

        assert_eq!(round.asked(), [id(LEDGER)]);
        assert_eq!(round.outcome(), Outcome::Proceed);
        assert_eq!(elsewhere.seen, [] as [ravel_host_api::Occasion; 0]);
    }

    /// A mod that declares nothing is neither asked nor an obstacle.
    #[test]
    fn a_mod_that_intercepts_nothing_is_not_asked() {
        let mut quiet = Quiet { id: id(GUARD) };
        let mut answering = Answering::new(LEDGER, Point::WorldIsSaved, Outcome::Proceed);
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut quiet, &mut answering];
        let machine = running(&[GUARD, LEDGER]);

        let round = intercept(&machine, &mut mods, Point::WorldIsSaved).expect("a legal set");

        assert_eq!(round.asked(), [id(LEDGER)]);
    }

    /// A mod that is not running is not asked, however it declared.
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
        assert_eq!(guard.seen, [] as [ravel_host_api::Occasion; 0]);
    }

    /// A declaration that names another mod is refused rather than believed.
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

    /// A mod that fails while answering ends the round and is named.
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

    /// A mod that declared a point and implements nothing is refused loudly.
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

            fn interception_declaration(&self) -> Option<InterceptionDeclaration> {
                let mut declared = InterceptionDeclaration::new(self.id.clone());
                declared.answer_at(Point::WorldIsSaved.as_str());
                Some(declared)
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

    /// A set that cannot be asked is refused rather than half asked.
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

            fn interception_declaration(&self) -> Option<InterceptionDeclaration> {
                let mut declared = InterceptionDeclaration::new(self.id.clone());
                declared.replace_at(Point::WorldIsSaved.as_str());
                Some(declared)
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
