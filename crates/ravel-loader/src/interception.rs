use std::fmt;

use ravel_core::{FailureKind, LifecycleState, ModId, StructuredFailure};
use ravel_host_api::{HostError, HostMod, InterceptionRefused, Occasion, Point, Points, Round};

use crate::loaded::LoadedMods;

/// Asks eligible mods at `point` in declared order, skipping parked or inactive mods.
///
/// A mod's answering failure ends the round and is retained in [`Round::failure`].
///
/// # Errors
///
/// Returns [`InterceptRefused`] for a halted set, foreign declarations, or an invalid point set.
///
/// # Panics
///
/// Never: ordered owners were found in `mods` while collecting declarations.
pub fn intercept<H: std::ops::DerefMut<Target: HostMod>>(
    loaded: &mut LoadedMods<'_, H>,
    point: Point,
) -> Result<Round, InterceptRefused> {
    let (machine, mods) = loaded.parts();
    crate::events::ensure_active(machine).map_err(InterceptRefused::Inactive)?;
    let mut points = Points::default();
    for mod_id in machine.order() {
        let Ok(state) = machine.state(mod_id) else {
            continue;
        };
        if !machine
            .entry(mod_id)
            .is_ok_and(|entry| entry.sides.loads_on(machine.side()))
            || !crate::executes_mod_code(state)
        {
            continue;
        }
        let host = mods
            .iter()
            .find(|host| host.id() == mod_id)
            .expect("LoadedMods checked every loading owner");
        let Some(declaration) = crate::callback(mod_id, state, || host.interception_declaration())
            .map_err(|source| InterceptRefused::Mod {
                mod_id: mod_id.clone(),
                source,
            })?
        else {
            continue;
        };
        if declaration.owner() != mod_id {
            return Err(InterceptRefused::ForeignDeclaration {
                mod_id: mod_id.clone(),
                owner: declaration.owner().clone(),
            });
        }
        points.declare(&declaration);
    }

    let mut asked = |owner: &ModId, occasion: Occasion| {
        let host = mods
            .iter_mut()
            .find(|host| host.id() == owner)
            .expect("only mods that were found above are ordered");
        let state = machine
            .state(owner)
            .expect("only eligible mods are ordered");
        crate::callback(owner, state, || host.intercept(occasion))
    };
    points
        .ask(point, machine.side(), &mut asked)
        .map_err(InterceptRefused::Refused)
}

/// Why a point could not be asked at all.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InterceptRefused {
    /// The set has halted and cannot ask any mod.
    Inactive(crate::DispatchRefused),
    /// A mod refused to declare its interceptions.
    Mod {
        /// The mod asked for its declaration.
        mod_id: ModId,
        /// The declaration hook's refusal.
        source: HostError,
    },
    /// A mod handed over a declaration belonging to another mod.
    ForeignDeclaration {
        /// The mod that produced the declaration.
        mod_id: ModId,
        /// The foreign owner the declaration named.
        owner: ModId,
    },
    /// The declared set cannot be asked.
    Refused(InterceptionRefused),
}

impl InterceptRefused {
    /// Returns the declaring mod responsible for this refusal, if there is one.
    #[must_use]
    pub const fn owner(&self) -> Option<&ModId> {
        match self {
            Self::Mod { mod_id, .. } | Self::ForeignDeclaration { mod_id, .. } => Some(mod_id),
            Self::Refused(_) | Self::Inactive(_) => None,
        }
    }
}

impl fmt::Display for InterceptRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Mod { source, .. } => source.fmt(formatter),
            Self::Inactive(refused) => refused.fmt(formatter),
            Self::ForeignDeclaration { mod_id, owner } => write!(
                formatter,
                "`{mod_id}` declared interceptions owned by `{owner}`, and a mod answers only for \
                 itself"
            ),
            Self::Refused(refused) => write!(formatter, "{refused}"),
        }
    }
}

impl std::error::Error for InterceptRefused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Mod { source, .. } => Some(source),
            Self::Inactive(refused) => Some(refused),
            Self::ForeignDeclaration { .. } => None,
            Self::Refused(refused) => Some(refused),
        }
    }
}

impl StructuredFailure for InterceptRefused {
    fn failure_kind(&self) -> Option<FailureKind> {
        None
    }

    fn lifecycle_state(&self) -> LifecycleState {
        match self {
            Self::Mod { source, .. } => source.lifecycle_state(),
            Self::Inactive(refused) => refused.lifecycle_state(),
            Self::ForeignDeclaration { .. } | Self::Refused(_) => LifecycleState::Running,
        }
    }

    fn quotes_untrusted_text(&self) -> bool {
        match self {
            Self::Mod { source, .. } => source.quotes_untrusted_text(),
            Self::ForeignDeclaration { .. } | Self::Inactive(_) => false,
            Self::Refused(refused) => {
                matches!(refused, InterceptionRefused::UnpublishedPoint { .. })
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{LifecycleState, ModId, Side, StructuredFailure};
    use ravel_host_api::{
        HostError, HostMod, InterceptionDeclaration, InterceptionRefused, ModError, Occasion,
        Outcome, Point,
    };

    use super::{InterceptRefused, intercept};
    use crate::lifecycle::LifecycleMachine;
    use crate::loaded::LoadedMods;
    use crate::tests::entry;

    // Reverse lexical order ensures ordering assertions depend on the declared edge.
    const GUARD: &str = "zephyr_guard";
    const LEDGER: &str = "alpha_ledger";

    fn id(name: &str) -> ModId {
        name.parse().expect("a legal mod id")
    }

    #[derive(Clone, Copy, Eq, PartialEq)]
    enum PanicAt {
        Never,
        Declaration,
        Answer,
    }

    struct Answering {
        id: ModId,
        point: Point,
        answer: Outcome,
        after: Option<ModId>,
        owner_override: Option<ModId>,
        fails: bool,
        refuses_declaration: bool,
        panic_at: PanicAt,
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
                panic_at: PanicAt::Never,
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

        fn enter(&mut self, _: LifecycleState) -> Result<(), ModError> {
            Ok(())
        }

        fn interception_declaration(&self) -> Result<Option<InterceptionDeclaration>, ModError> {
            assert!(self.panic_at != PanicAt::Declaration, "declaration panic");
            if self.refuses_declaration {
                return Err(ModError::Unsupported);
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

        fn intercept(&mut self, occasion: Occasion) -> Result<Outcome, ModError> {
            assert!(self.panic_at != PanicAt::Answer, "answer panic");
            self.seen.push(occasion);
            if self.fails {
                return Err(ModError::refused("the ledger is closed"));
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

        fn enter(&mut self, _: LifecycleState) -> Result<(), ModError> {
            Ok(())
        }
    }

    fn running(names: &[&str]) -> LifecycleMachine {
        let mut machine = LifecycleMachine::new(
            Side::IntegratedServer,
            names
                .iter()
                .map(|name| entry(name, &[Side::IntegratedServer, Side::DedicatedServer]))
                .collect(),
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
    fn halted_sets_refuse_interception_before_any_mod_callback() {
        let mut ledger = Quiet { id: id(LEDGER) };
        let mut guard = Answering::new(GUARD, Point::WorldIsSaved, Outcome::Proceed);
        guard.panic_at = PanicAt::Declaration;
        let mut machine = running(&[GUARD, LEDGER]);
        machine
            .record_world_mutation()
            .expect("a running set can mutate the world");
        machine
            .fail(
                &id(LEDGER),
                ravel_core::FailureAttribution {
                    mod_id: Some(id(LEDGER)),
                    version: None,
                    dependency_chain: Vec::new(),
                    state: LifecycleState::Running,
                    kind: None,
                    surface: None,
                    environment: ravel_core::FailureEnvironment {
                        ravel: "0.1.0".parse().expect("valid version"),
                        minecraft: "test".to_owned(),
                        platform: "test".to_owned(),
                        architecture: "test".to_owned(),
                    },
                    message: ravel_observability::redact("required mod failed"),
                },
            )
            .expect("a running required mod can fail");
        let refusal = intercept(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut guard, &mut ledger])
                .expect("all active hosts are supplied"),
            Point::WorldIsSaved,
        )
        .expect_err("a halted set asks no callbacks");
        assert_eq!(
            refusal,
            InterceptRefused::Inactive(crate::DispatchRefused::NotReacting {
                mod_id: id(LEDGER),
                state: LifecycleState::Failed,
            })
        );
        assert_eq!(refusal.lifecycle_state(), LifecycleState::Failed);
        assert!(!refusal.quotes_untrusted_text());
        assert_eq!(guard.seen, []);
    }

    #[test]
    fn second_answer_panics_name_the_second_asked_mod() {
        let mut guard = Answering::new(GUARD, Point::WorldIsSaved, Outcome::Proceed);
        let mut ledger = Answering::new(LEDGER, Point::WorldIsSaved, Outcome::Proceed).after(GUARD);
        ledger.panic_at = PanicAt::Answer;
        let round = intercept(
            &mut LoadedMods::<&mut dyn HostMod>::new(
                &mut running(&[GUARD, LEDGER]),
                &mut [&mut ledger, &mut guard],
            )
            .expect("all active hosts are supplied"),
            Point::WorldIsSaved,
        )
        .expect("declarations are legal");
        assert_eq!(round.outcome(), Outcome::Refuse);
        assert_eq!(round.failed_by(), Some(&id(LEDGER)));
        assert_eq!(round.failure().map(HostError::mod_id), Some(&id(LEDGER)));
        assert_eq!(guard.seen.len(), 1);
        assert_eq!(ledger.seen, []);
    }

    #[test]
    fn interception_declaration_refusal_is_not_absence() {
        let mut host = Answering::new(GUARD, Point::WorldIsSaved, Outcome::Proceed);
        host.refuses_declaration = true;
        assert_eq!(
            intercept(
                &mut LoadedMods::<&mut dyn HostMod>::new(&mut running(&[GUARD]), &mut [&mut host])
                    .expect("all active hosts are supplied"),
                Point::WorldIsSaved
            ),
            Err(InterceptRefused::Mod {
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
        let mut machine = running(&[GUARD, LEDGER]);

        let round = intercept(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            Point::WorldIsSaved,
        )
        .expect("a legal set");

        assert_eq!(round.asked(), [id(GUARD), id(LEDGER)]);
        assert_eq!(round.outcome(), Outcome::Proceed);
        assert_eq!(round.point(), Point::WorldIsSaved);
    }

    #[test]
    fn a_refusal_ends_the_round_before_the_later_mod_is_asked() {
        let mut guard = Answering::new(GUARD, Point::WorldIsSaved, Outcome::Refuse);
        let mut ledger = Answering::new(LEDGER, Point::WorldIsSaved, Outcome::Proceed).after(GUARD);
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut ledger, &mut guard];
        let mut machine = running(&[GUARD, LEDGER]);

        let round = intercept(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            Point::WorldIsSaved,
        )
        .expect("a legal set");

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
        let mut machine = running(&[GUARD, LEDGER]);

        let round = intercept(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            Point::WorldIsSaved,
        )
        .expect("a legal set");

        assert_eq!(round.asked(), [id(LEDGER)]);
        assert_eq!(round.outcome(), Outcome::Proceed);
        assert_eq!(elsewhere.seen, Vec::<Occasion>::new());
    }

    #[test]
    fn a_mod_that_intercepts_nothing_is_not_asked() {
        let mut quiet = Quiet { id: id(GUARD) };
        let mut answering = Answering::new(LEDGER, Point::WorldIsSaved, Outcome::Proceed);
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut quiet, &mut answering];
        let mut machine = running(&[GUARD, LEDGER]);

        let round = intercept(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            Point::WorldIsSaved,
        )
        .expect("a legal set");

        assert_eq!(round.asked(), [id(LEDGER)]);
    }

    #[test]
    fn a_mod_that_is_not_running_is_not_asked() {
        let mut guard = Answering::new(GUARD, Point::WorldIsSaved, Outcome::Refuse);
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut guard];
        let mut machine = LifecycleMachine::new(
            Side::IntegratedServer,
            vec![entry(
                GUARD,
                &[Side::IntegratedServer, Side::DedicatedServer],
            )],
        )
        .expect("a legal plan");
        machine
            .advance_all(LifecycleState::Validated)
            .expect("a legal step");

        let round = intercept(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            Point::WorldIsSaved,
        )
        .expect("a legal set");

        assert_eq!(round.asked(), []);
        assert_eq!(round.outcome(), Outcome::Proceed);
        assert_eq!(guard.seen, Vec::<Occasion>::new());
    }

    #[test]
    fn a_mod_declaring_interceptions_for_another_mod_is_refused() {
        let mut liar = Answering::new(GUARD, Point::WorldIsSaved, Outcome::Refuse).owned_by(LEDGER);
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut liar];
        let mut machine = running(&[GUARD]);

        let refused = intercept(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            Point::WorldIsSaved,
        )
        .expect_err("a mod answers for itself");

        assert_eq!(
            refused,
            InterceptRefused::ForeignDeclaration {
                mod_id: id(GUARD),
                owner: id(LEDGER),
            }
        );
        assert!(refused.to_string().contains(LEDGER));
        assert!(refused.to_string().contains(GUARD));
        assert_eq!(refused.lifecycle_state(), LifecycleState::Running);
        assert_eq!(refused.failure_kind(), None);
        assert!(!refused.quotes_untrusted_text());
        assert_eq!(refused.owner(), Some(&id(GUARD)));
    }

    #[test]
    fn a_mod_that_fails_while_answering_ends_the_round_and_is_attributed() {
        let mut guard = Answering::new(GUARD, Point::WorldIsSaved, Outcome::Proceed).failing();
        let mut ledger = Answering::new(LEDGER, Point::WorldIsSaved, Outcome::Proceed).after(GUARD);
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut ledger, &mut guard];
        let mut machine = running(&[GUARD, LEDGER]);

        let round = intercept(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            Point::WorldIsSaved,
        )
        .expect("a legal set");

        assert_eq!(
            round.outcome(),
            Outcome::Refuse,
            "a host that could not finish asking was told it may proceed"
        );
        let failure = round.failure().expect("the failure is carried out");
        assert_eq!(failure.mod_id(), &id(GUARD));
        assert_eq!(round.failed_by(), Some(&id(GUARD)));
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

            fn enter(&mut self, _: LifecycleState) -> Result<(), ModError> {
                Ok(())
            }

            fn interception_declaration(
                &self,
            ) -> Result<Option<InterceptionDeclaration>, ModError> {
                let mut declared = InterceptionDeclaration::new(self.id.clone());
                declared.answer_at(Point::WorldIsSaved.as_str());
                Ok(Some(declared))
            }
        }

        let mut silent = Silent { id: id(GUARD) };
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut silent];
        let mut machine = running(&[GUARD]);

        let round = intercept(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            Point::WorldIsSaved,
        )
        .expect("a legal set");

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

            fn enter(&mut self, _: LifecycleState) -> Result<(), ModError> {
                Ok(())
            }

            fn interception_declaration(
                &self,
            ) -> Result<Option<InterceptionDeclaration>, ModError> {
                let mut declared = InterceptionDeclaration::new(self.id.clone());
                declared.replace_at(Point::WorldIsSaved.as_str());
                Ok(Some(declared))
            }

            fn intercept(&mut self, _: Occasion) -> Result<Outcome, ModError> {
                Ok(Outcome::Proceed)
            }
        }

        let mut first = Replacing { id: id(GUARD) };
        let mut second = Replacing { id: id(LEDGER) };
        let mut mods: Vec<&mut dyn HostMod> = vec![&mut first, &mut second];
        let mut machine = running(&[GUARD, LEDGER]);

        let refused = intercept(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            Point::WorldIsSaved,
        )
        .expect_err("two replacements are a conflict");

        assert!(
            matches!(
                refused,
                InterceptRefused::Refused(InterceptionRefused::ContestedPoint { .. })
            ),
            "{refused:?}"
        );
        assert!(std::error::Error::source(&refused).is_some());
    }

    #[test]
    fn declaration_panics_are_attributed_to_the_declaring_host() {
        let mut host = Answering::new(GUARD, Point::WorldIsSaved, Outcome::Proceed);
        host.panic_at = PanicAt::Declaration;
        let refused = intercept(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut running(&[GUARD]), &mut [&mut host])
                .expect("all active hosts are supplied"),
            Point::WorldIsSaved,
        )
        .expect_err("a declaration panic refuses admission");
        assert_eq!(refused.owner(), Some(&id(GUARD)));
        let InterceptRefused::Mod { source, .. } = refused else {
            panic!("expected an attributed callback failure");
        };
        assert_eq!(source.mod_id(), &id(GUARD));
        assert!(source.to_string().contains("panicked"));
    }

    #[test]
    fn answer_panics_stop_the_round_and_record_the_real_owner() {
        let mut guard = Answering::new(GUARD, Point::WorldIsSaved, Outcome::Proceed);
        guard.panic_at = PanicAt::Answer;
        let mut ledger = Answering::new(LEDGER, Point::WorldIsSaved, Outcome::Proceed).after(GUARD);
        let round = intercept(
            &mut LoadedMods::<&mut dyn HostMod>::new(
                &mut running(&[GUARD, LEDGER]),
                &mut [&mut guard, &mut ledger],
            )
            .expect("all active hosts are supplied"),
            Point::WorldIsSaved,
        )
        .expect("declarations are legal");
        assert_eq!(round.outcome(), Outcome::Refuse);
        assert_eq!(round.failed_by(), Some(&id(GUARD)));
        assert_eq!(
            round.failure().expect("panic retained").mod_id(),
            &id(GUARD)
        );
        assert_eq!(ledger.seen, []);
    }

    #[test]
    fn point_set_refusals_do_not_identify_a_single_failing_owner() {
        let refused = InterceptRefused::Refused(InterceptionRefused::UnpublishedPoint {
            name: "unpublished".to_owned(),
            claimants: vec![id(GUARD)],
        });
        assert_eq!(refused.owner(), None);
    }
}
