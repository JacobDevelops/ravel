use std::fmt;

use ravel_core::{FailureEnvironment, FailureKind, LifecycleState, ModId, StructuredFailure};
use ravel_host_api::{
    AdmittedPoints, HostError, InterceptionDeclaration, InterceptionRefused, Mod, Occasion, Point,
    Points, Round,
};

use crate::loaded::LoadedMods;

/// Collects and admits interception declarations once, before world access.
///
/// Optional declaration failures and conflicts remove their owners and retry admission.
/// All claimants of a conflict receive attribution, including required owners.
///
/// # Errors
///
/// Returns the typed declaration or admission refusal when a required owner fails.
pub fn open_interceptions<H: std::ops::DerefMut<Target: Mod>>(
    loaded: &mut LoadedMods<'_, H>,
    environment: &FailureEnvironment,
) -> Result<AdmittedPoints, InterceptRefused> {
    let declarations = declarations(loaded, environment)?;
    let (machine, _) = loaded.parts();
    loop {
        let mut points = Points::default();
        for declaration in &declarations {
            if machine.loads(declaration.owner()) {
                points.declare(declaration);
            }
        }
        match points.admit() {
            Ok(admitted) => return Ok(admitted),
            Err(refused) => {
                let mut stop = false;
                for owner in refused.claimants() {
                    let source = crate::owner_refusal(machine, owner, &refused)
                        .map_err(InterceptRefused::Lifecycle)?;
                    stop |= machine
                        .fail_setup_callback(&source, environment)
                        .map_err(InterceptRefused::Lifecycle)?;
                }
                if stop || refused.claimants().is_empty() {
                    return Err(InterceptRefused::Refused(refused));
                }
            }
        }
    }
}

fn declarations<H: std::ops::DerefMut<Target: Mod>>(
    loaded: &mut LoadedMods<'_, H>,
    environment: &FailureEnvironment,
) -> Result<Vec<InterceptionDeclaration>, InterceptRefused> {
    let (machine, mods) = loaded.parts();
    if let Some(by) = machine.halted_by() {
        return Err(InterceptRefused::Halted { by: by.clone() });
    }
    let eligible: Vec<_> = machine
        .eligible()
        .filter(|(_, state)| crate::executes_mod_code(*state))
        .map(|(entry, state)| (entry.mod_id.clone(), state))
        .collect();
    let mut declarations = Vec::with_capacity(eligible.len());
    for (mod_id, state) in eligible {
        let instance = crate::loaded::instance(mods, &mod_id);
        let declaration =
            match crate::callback(&mod_id, state, || instance.interception_declaration()) {
                Ok(Some(declaration)) => declaration,
                Ok(None) => continue,
                Err(source) => {
                    declaration_failure(machine, environment, source)?;
                    continue;
                }
            };
        if declaration.owner() != &mod_id {
            let refused = InterceptRefused::ForeignDeclaration {
                mod_id: mod_id.clone(),
                owner: declaration.owner().clone(),
            };
            let source = crate::owner_refusal(machine, &mod_id, &refused)
                .map_err(InterceptRefused::Lifecycle)?;
            if machine
                .fail_setup_callback(&source, environment)
                .map_err(InterceptRefused::Lifecycle)?
            {
                return Err(refused);
            }
            continue;
        }
        declarations.push(declaration);
    }
    Ok(declarations)
}

/// Asks currently eligible owners from a previously admitted point set.
///
/// Callback failures are recorded before returning the round; declarations are not recalled.
///
/// # Errors
///
/// Returns [`InterceptRefused`] if the set is halted or attribution fails.
pub fn intercept<H: std::ops::DerefMut<Target: Mod>>(
    loaded: &mut LoadedMods<'_, H>,
    points: &AdmittedPoints,
    point: Point,
    environment: &FailureEnvironment,
) -> Result<Round, InterceptRefused> {
    let (machine, mods) = loaded.parts();
    if let Some(by) = machine.halted_by() {
        return Err(InterceptRefused::Halted { by: by.clone() });
    }
    let side = if point == Point::WorldIsSaved && machine.side() == ravel_core::Side::Client {
        ravel_core::Side::IntegratedServer
    } else {
        machine.side()
    };
    let states: std::collections::BTreeMap<_, _> = machine
        .tracked_entries()
        .filter(|(entry, state)| entry.sides.loads_on(side) && crate::executes_mod_code(*state))
        .map(|(entry, state)| (entry.mod_id.clone(), state))
        .collect();
    let mut asked = |owner: &ModId, occasion: Occasion| {
        let instance = crate::loaded::instance_mut(mods, owner);
        crate::callback(owner, states[owner], || instance.intercept(occasion))
    };
    let round = points.ask(point, side, |owner| states.contains_key(owner), &mut asked);
    if let Some(error) = round.failure() {
        machine
            .fail_callback(error, environment, None)
            .map_err(InterceptRefused::Lifecycle)?;
    }
    Ok(round)
}

fn declaration_failure(
    machine: &mut crate::LifecycleMachine,
    environment: &FailureEnvironment,
    source: HostError,
) -> Result<(), InterceptRefused> {
    if machine
        .fail_setup_callback(&source, environment)
        .map_err(InterceptRefused::Lifecycle)?
    {
        return Err(InterceptRefused::Mod {
            mod_id: source.mod_id().clone(),
            source,
        });
    }
    Ok(())
}

/// Why a point could not be asked at all.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum InterceptRefused {
    /// The lifecycle refused failure attribution.
    Lifecycle(crate::LifecycleError),
    /// The set has halted and cannot ask any mod.
    Halted {
        /// The mod whose failure halted the set.
        by: ModId,
    },
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
    pub const fn mod_id(&self) -> Option<&ModId> {
        match self {
            Self::Mod { mod_id, .. } | Self::ForeignDeclaration { mod_id, .. } => Some(mod_id),
            Self::Refused(_) | Self::Halted { .. } | Self::Lifecycle(_) => None,
        }
    }
}

impl fmt::Display for InterceptRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lifecycle(error) => error.fmt(formatter),
            Self::Mod { source, .. } => source.fmt(formatter),
            Self::Halted { by } => write!(formatter, "the mod set halted after `{by}` failed"),
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
            Self::Lifecycle(error) => Some(error),
            Self::Mod { source, .. } => Some(source),
            Self::Halted { .. } | Self::ForeignDeclaration { .. } => None,
            Self::Refused(refused) => Some(refused),
        }
    }
}

impl StructuredFailure for InterceptRefused {
    fn failure_kind(&self) -> Option<FailureKind> {
        match self {
            Self::Mod { source, .. } => source.failure_kind(),
            Self::Lifecycle(error) => error.failure_kind(),
            Self::Halted { .. } | Self::ForeignDeclaration { .. } | Self::Refused(_) => None,
        }
    }

    fn lifecycle_state(&self) -> LifecycleState {
        match self {
            Self::Lifecycle(error) => error.lifecycle_state(),
            Self::Mod { source, .. } => source.lifecycle_state(),
            Self::Halted { .. } => LifecycleState::Failed,
            Self::ForeignDeclaration { .. } | Self::Refused(_) => LifecycleState::Running,
        }
    }

    fn quotes_untrusted_text(&self) -> bool {
        match self {
            Self::Mod { source, .. } => source.quotes_untrusted_text(),
            Self::Lifecycle(error) => error.quotes_untrusted_text(),
            Self::Halted { .. } | Self::ForeignDeclaration { .. } | Self::Refused(_) => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{LifecycleState, ModId, Side, StructuredFailure};
    use ravel_host_api::{
        HostError, InterceptionDeclaration, InterceptionRefused, Mod, ModError, Occasion, Outcome,
        Point,
    };

    use super::InterceptRefused;

    fn intercept<H: std::ops::DerefMut<Target: Mod>>(
        loaded: &mut LoadedMods<'_, H>,
        point: Point,
    ) -> Result<ravel_host_api::Round, InterceptRefused> {
        let points = super::open_interceptions(loaded, &crate::tests::environment())?;
        super::intercept(loaded, &points, point, &crate::tests::environment())
    }
    use crate::lifecycle::LifecycleMachine;
    use crate::loaded::LoadedMods;
    use crate::tests::{advance_through, entry};

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
        declaration_calls: std::cell::Cell<usize>,
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
                declaration_calls: std::cell::Cell::new(0),
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

    impl Mod for Answering {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _: LifecycleState) -> Result<(), ModError> {
            Ok(())
        }

        fn interception_declaration(&self) -> Result<Option<InterceptionDeclaration>, ModError> {
            self.declaration_calls.set(self.declaration_calls.get() + 1);
            assert!(self.panic_at != PanicAt::Declaration, "declaration panic");
            if self.refuses_declaration {
                return Err(ModError::Unsupported);
            }
            let owner = self
                .owner_override
                .clone()
                .unwrap_or_else(|| self.id.clone());
            let mut declared = InterceptionDeclaration::new(owner);
            declared.answer_at(self.point);
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

    impl Mod for Quiet {
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
        advance_through(&mut machine, LifecycleState::Running);
        machine
    }

    #[test]
    fn unknown_owners_are_attributed_and_optional_claimants_isolated() {
        for requirement in [crate::Requirement::Optional, crate::Requirement::Required] {
            let mut guard =
                Answering::new(GUARD, Point::WorldIsSaved, Outcome::Proceed).after("missing");
            let mut ledger = Answering::new(LEDGER, Point::WorldIsSaved, Outcome::Proceed);
            let mut guard_entry = entry(GUARD, &[Side::IntegratedServer]);
            guard_entry.requirement = requirement;
            let mut machine = LifecycleMachine::new(
                Side::IntegratedServer,
                vec![guard_entry, entry(LEDGER, &[Side::IntegratedServer])],
            )
            .expect("plan");
            advance_through(&mut machine, LifecycleState::Running);
            let admitted = super::open_interceptions(
                &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut guard, &mut ledger])
                    .expect("paired owners"),
                &crate::tests::environment(),
            );
            assert_eq!(machine.state(&id(GUARD)), Ok(LifecycleState::Failed));
            assert!(machine.failure(&id(GUARD)).expect("tracked").is_some());
            assert_eq!(machine.state(&id(LEDGER)), Ok(LifecycleState::Running));
            if requirement == crate::Requirement::Required {
                assert_eq!(
                    admitted.expect_err("required invalid owner refuses admission"),
                    InterceptRefused::Refused(InterceptionRefused::UnknownOwners {
                        owners: vec![id("missing")],
                        claimants: vec![id(GUARD)],
                    })
                );
            } else {
                let points = admitted.expect("optional invalid claimant is isolated");
                let round = super::intercept(
                    &mut LoadedMods::<&mut dyn Mod>::new(
                        &mut machine,
                        &mut [&mut guard, &mut ledger],
                    )
                    .expect("paired owners"),
                    &points,
                    Point::WorldIsSaved,
                    &crate::tests::environment(),
                )
                .expect("surviving owner can answer");
                assert_eq!(round.asked(), [id(LEDGER)]);
                assert_eq!(guard.declaration_calls.get(), 1);
            }
            assert_eq!(guard.seen, []);
        }
    }

    #[test]
    fn client_world_save_asks_only_owners_enabled_on_the_integrated_side() {
        let mut guard = Answering::new(GUARD, Point::WorldIsSaved, Outcome::Refuse);
        let mut ledger = Answering::new(LEDGER, Point::WorldIsSaved, Outcome::Proceed);
        let mut machine = LifecycleMachine::new(
            Side::Client,
            vec![
                entry(GUARD, &[Side::Client]),
                entry(LEDGER, &[Side::Client, Side::IntegratedServer]),
            ],
        )
        .expect("client plan");
        advance_through(&mut machine, LifecycleState::Running);
        let round = intercept(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut guard, &mut ledger])
                .expect("paired owners"),
            Point::WorldIsSaved,
        )
        .expect("integrated round");
        assert_eq!(round.asked(), [id(LEDGER)]);
        assert_eq!(ledger.seen[0].side(), Side::IntegratedServer);
        assert_eq!(guard.seen, []);
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
                    message: ravel_core::DiagnosticMessage::redacted("required mod failed"),
                },
            )
            .expect("a running required mod can fail");
        let refusal = intercept(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut guard, &mut ledger])
                .expect("all active hosts are supplied"),
            Point::WorldIsSaved,
        )
        .expect_err("a halted set asks no callbacks");
        assert_eq!(refusal, InterceptRefused::Halted { by: id(LEDGER) });
        assert_eq!(refusal.lifecycle_state(), LifecycleState::Failed);
        assert!(!refusal.quotes_untrusted_text());
        let mut points = ravel_host_api::Points::default();
        points.answer_at(&id(GUARD), Point::WorldIsSaved);
        let points = points.admit().expect("cached legal declaration");
        assert_eq!(
            super::intercept(
                &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut guard, &mut ledger],)
                    .expect("paired owners"),
                &points,
                Point::WorldIsSaved,
                &crate::tests::environment(),
            ),
            Err(InterceptRefused::Halted { by: id(LEDGER) })
        );
        assert_eq!(guard.seen, []);
    }

    #[test]
    fn second_answer_panics_name_the_second_asked_mod() {
        let mut guard = Answering::new(GUARD, Point::WorldIsSaved, Outcome::Proceed);
        let mut ledger = Answering::new(LEDGER, Point::WorldIsSaved, Outcome::Proceed).after(GUARD);
        ledger.panic_at = PanicAt::Answer;
        let round = intercept(
            &mut LoadedMods::<&mut dyn Mod>::new(
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
                &mut LoadedMods::<&mut dyn Mod>::new(&mut running(&[GUARD]), &mut [&mut host])
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
        let mut mods: Vec<&mut dyn Mod> = vec![&mut ledger, &mut guard];
        let mut machine = running(&[GUARD, LEDGER]);

        let round = intercept(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut mods)
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
        let mut mods: Vec<&mut dyn Mod> = vec![&mut ledger, &mut guard];
        let mut machine = running(&[GUARD, LEDGER]);

        let round = intercept(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut mods)
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
        let mut mods: Vec<&mut dyn Mod> = vec![&mut elsewhere, &mut here];
        let mut machine = running(&[GUARD, LEDGER]);

        let round = intercept(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut mods)
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
        let mut mods: Vec<&mut dyn Mod> = vec![&mut quiet, &mut answering];
        let mut machine = running(&[GUARD, LEDGER]);

        let round = intercept(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            Point::WorldIsSaved,
        )
        .expect("a legal set");

        assert_eq!(round.asked(), [id(LEDGER)]);
    }

    #[test]
    fn a_mod_that_is_not_running_is_not_asked() {
        let mut guard = Answering::new(GUARD, Point::WorldIsSaved, Outcome::Refuse);
        let mut mods: Vec<&mut dyn Mod> = vec![&mut guard];
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
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut mods)
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
        let mut mods: Vec<&mut dyn Mod> = vec![&mut liar];
        let mut machine = running(&[GUARD]);

        let refused = intercept(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut mods)
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
        assert_eq!(refused.mod_id(), Some(&id(GUARD)));
    }

    #[test]
    fn a_mod_that_fails_while_answering_ends_the_round_and_is_attributed() {
        let mut guard = Answering::new(GUARD, Point::WorldIsSaved, Outcome::Proceed).failing();
        let mut ledger = Answering::new(LEDGER, Point::WorldIsSaved, Outcome::Proceed).after(GUARD);
        let mut mods: Vec<&mut dyn Mod> = vec![&mut ledger, &mut guard];
        let mut machine = running(&[GUARD, LEDGER]);

        let round = intercept(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut mods)
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

        impl Mod for Silent {
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
                declared.answer_at(Point::WorldIsSaved);
                Ok(Some(declared))
            }
        }

        let mut silent = Silent { id: id(GUARD) };
        let mut mods: Vec<&mut dyn Mod> = vec![&mut silent];
        let mut machine = running(&[GUARD]);

        let round = intercept(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut mods)
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

        impl Mod for Replacing {
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
                declared.replace_at(Point::WorldIsSaved);
                Ok(Some(declared))
            }

            fn intercept(&mut self, _: Occasion) -> Result<Outcome, ModError> {
                Ok(Outcome::Proceed)
            }
        }

        let mut first = Replacing { id: id(GUARD) };
        let mut second = Replacing { id: id(LEDGER) };
        let mut mods: Vec<&mut dyn Mod> = vec![&mut first, &mut second];
        let mut machine = running(&[GUARD, LEDGER]);

        let refused = intercept(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut mods)
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
        for owner in [id(GUARD), id(LEDGER)] {
            assert_eq!(machine.state(&owner), Ok(LifecycleState::Failed));
            let failure = machine
                .failure(&owner)
                .expect("tracked")
                .expect("attributed");
            assert_eq!(failure.mod_id.as_ref(), Some(&owner));
            assert_eq!(failure.environment, crate::tests::environment());
        }
    }

    #[test]
    fn declaration_panics_are_attributed_to_the_declaring_host() {
        let mut host = Answering::new(GUARD, Point::WorldIsSaved, Outcome::Proceed);
        host.panic_at = PanicAt::Declaration;
        let refused = intercept(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut running(&[GUARD]), &mut [&mut host])
                .expect("all active hosts are supplied"),
            Point::WorldIsSaved,
        )
        .expect_err("a declaration panic refuses admission");
        assert_eq!(refused.mod_id(), Some(&id(GUARD)));
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
        let mut machine = running(&[GUARD, LEDGER]);
        let round = intercept(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut guard, &mut ledger])
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
        assert_eq!(machine.state(&id(GUARD)), Ok(LifecycleState::Failed));
        let attribution = machine
            .failure(&id(GUARD))
            .expect("tracked")
            .expect("recorded");
        assert_eq!(attribution.environment, crate::tests::environment());
        assert_eq!(attribution.mod_id.as_ref(), Some(&id(GUARD)));
    }

    #[test]
    fn optional_cycles_are_attributed_then_removed_before_admission() {
        let mut guard = Answering::new(GUARD, Point::WorldIsSaved, Outcome::Proceed).after(LEDGER);
        let mut ledger = Answering::new(LEDGER, Point::WorldIsSaved, Outcome::Proceed).after(GUARD);
        let mut guard_entry = entry(GUARD, &[Side::IntegratedServer]);
        guard_entry.requirement = crate::Requirement::Optional;
        let mut ledger_entry = entry(LEDGER, &[Side::IntegratedServer]);
        ledger_entry.requirement = crate::Requirement::Optional;
        let mut machine =
            LifecycleMachine::new(Side::IntegratedServer, vec![guard_entry, ledger_entry])
                .expect("plan");
        advance_through(&mut machine, LifecycleState::Running);
        let points = super::open_interceptions(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut guard, &mut ledger])
                .expect("paired owners"),
            &crate::tests::environment(),
        )
        .expect("optional cycle owners leave the admitted set");
        for owner in [id(GUARD), id(LEDGER)] {
            assert_eq!(machine.state(&owner), Ok(LifecycleState::Failed));
            let failure = machine
                .failure(&owner)
                .expect("tracked")
                .expect("attributed");
            assert_eq!(failure.mod_id.as_ref(), Some(&owner));
            assert_eq!(failure.environment, crate::tests::environment());
            assert!(!machine.loads(&owner));
        }
        assert!(machine.halted_by().is_none());
        let round = super::intercept(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut guard, &mut ledger])
                .expect("paired owners"),
            &points,
            Point::WorldIsSaved,
            &crate::tests::environment(),
        )
        .expect("empty admitted point is safe");
        assert_eq!(round.outcome(), Outcome::Proceed);
        let empty: &[Occasion] = &[];
        assert_eq!(guard.seen.as_slice(), empty);
        assert_eq!(ledger.seen.as_slice(), empty);
        assert_eq!(guard.declaration_calls.get(), 1);
        assert_eq!(ledger.declaration_calls.get(), 1);
    }

    #[test]
    fn cycles_with_a_required_owner_refuse_admission_after_attributing_every_owner() {
        for guard_requirement in [crate::Requirement::Optional, crate::Requirement::Required] {
            let mut guard =
                Answering::new(GUARD, Point::WorldIsSaved, Outcome::Proceed).after(LEDGER);
            let mut ledger =
                Answering::new(LEDGER, Point::WorldIsSaved, Outcome::Proceed).after(GUARD);
            let mut guard_entry = entry(GUARD, &[Side::IntegratedServer]);
            guard_entry.requirement = guard_requirement;
            let mut machine = LifecycleMachine::new(
                Side::IntegratedServer,
                vec![guard_entry, entry(LEDGER, &[Side::IntegratedServer])],
            )
            .expect("plan");
            advance_through(&mut machine, LifecycleState::Running);
            let refused = super::open_interceptions(
                &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut guard, &mut ledger])
                    .expect("paired owners"),
                &crate::tests::environment(),
            )
            .expect_err("a required cycle owner prevents admission");
            let InterceptRefused::Refused(InterceptionRefused::Circular { point, claimants }) =
                refused
            else {
                panic!("expected a typed circular refusal, got {refused:?}");
            };
            assert_eq!(point, Point::WorldIsSaved);
            assert_eq!(claimants.len(), 2);
            assert!(claimants.contains(&id(GUARD)));
            assert!(claimants.contains(&id(LEDGER)));
            for owner in [id(GUARD), id(LEDGER)] {
                assert_eq!(machine.state(&owner), Ok(LifecycleState::Failed));
                let failure = machine
                    .failure(&owner)
                    .expect("tracked")
                    .expect("attributed");
                assert_eq!(failure.mod_id.as_ref(), Some(&owner));
                assert_eq!(failure.environment, crate::tests::environment());
            }
            assert_eq!(guard.seen, Vec::<Occasion>::new());
            assert_eq!(ledger.seen, Vec::<Occasion>::new());
        }
    }

    #[test]
    fn cached_admission_skips_later_failures_without_recalling_declarations() {
        let mut guard = Answering::new(GUARD, Point::WorldIsSaved, Outcome::Proceed);
        let mut ledger = Answering::new(LEDGER, Point::WorldIsSaved, Outcome::Proceed).after(GUARD);
        let mut optional = entry(GUARD, &[Side::IntegratedServer]);
        optional.requirement = crate::Requirement::Optional;
        let mut machine = LifecycleMachine::new(
            Side::IntegratedServer,
            vec![optional, entry(LEDGER, &[Side::IntegratedServer])],
        )
        .expect("plan");
        advance_through(&mut machine, LifecycleState::Running);
        let points = super::open_interceptions(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut guard, &mut ledger])
                .expect("paired owners"),
            &crate::tests::environment(),
        )
        .expect("admitted declarations");
        let source = HostError::Unsupported {
            mod_id: id(GUARD),
            state: LifecycleState::Running,
        };
        // A provider callback can fail after point admission and before this round.
        machine
            .fail_callback(&source, &crate::tests::environment(), None)
            .expect("attributed");
        let round = super::intercept(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut guard, &mut ledger])
                .expect("paired owners"),
            &points,
            Point::WorldIsSaved,
            &crate::tests::environment(),
        )
        .expect("optional failure does not halt the cached round");
        assert_eq!(round.outcome(), Outcome::Proceed);
        assert_eq!(guard.declaration_calls.get(), 1);
        assert_eq!(ledger.declaration_calls.get(), 1);
        let empty: &[Occasion] = &[];
        assert_eq!(guard.seen.as_slice(), empty);
        assert_eq!(ledger.seen.len(), 1);
    }

    #[test]
    fn point_set_refusals_do_not_identify_a_single_failing_owner() {
        let refused = InterceptRefused::Refused(InterceptionRefused::ContestedPoint {
            point: Point::WorldIsSaved,
            claimants: vec![id(GUARD), id(LEDGER)],
        });
        assert_eq!(refused.mod_id(), None);
    }
}
