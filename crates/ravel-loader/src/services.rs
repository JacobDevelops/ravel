use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use ravel_core::{FailureKind, LifecycleState, ModId, StructuredFailure, precedence};
use ravel_host_api::{
    HostError, HostMod, ServiceCallFailed, ServiceDeclaration, ServiceDirectory, ServiceId,
    ServiceRequest, ServiceResponse, ServiceUnavailable,
};

use crate::lifecycle::LifecycleMachine;

/// The order the set's services were opened in.
///
/// Returned rather than discarded because "in dependency order" is the property
/// worth checking, and a caller cannot check an order it was not told.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceOrder {
    order: Vec<ModId>,
}

impl ServiceOrder {
    /// Returns every mod that was offered its services, in the order it was.
    #[must_use]
    pub fn order(&self) -> &[ModId] {
        &self.order
    }
}

/// Opens every mod's declared services, providers before consumers.
///
/// A service is answered by exactly one mod — its name says which — so this is a
/// dependency graph over identities and not a search. The order is the
/// lexicographically smallest topological order of that graph, which makes it a
/// property of the declarations alone rather than of the order the caller supplied
/// the mods in.
///
/// Each mod is handed a directory over the mods **already** started, so a call
/// cannot reach a provider that has not run. That is the ordering guarantee as a
/// structural fact rather than a rule a mod is trusted to follow: there is no
/// value in the directory for a mod later in the order, so no call to one can be
/// written.
///
/// A mod the machine parks on this side takes no part: it answers nothing, and a
/// consumer that requires one of its services is refused with
/// [`ServiceRefused::NoProvider`] rather than left waiting for a mod that will
/// never run. That is `SPEC.md` §14's side rule reaching the service graph.
///
/// # Optional services
///
/// A service a mod declared it *may use* orders exactly as a required one when
/// something running answers it, and refuses nothing when nothing does: the set
/// loads and the consumer's call reports [`ravel_host_api::ServiceUnavailable::Absent`].
/// Absence is decided once, here, by the same graph that decides the order — so a
/// consumer never sees "not installed" for a mod that is installed and merely
/// ordered later. A parked provider is absent for the same reason it is a missing
/// one for a required service, and a mod parked on this side is offered nothing
/// whether its needs are optional or not.
///
/// # Failure closure
///
/// Nothing is partially wired. A refusal stops the walk, so every mod after the
/// offender in the order is never offered its services, and the refusal names the
/// mod that caused it. A caller that gets an error must not start the set: a
/// consumer that could not reach what it declared has not started, and running it
/// anyway is running a mod with half its dependencies.
///
/// # Errors
///
/// Returns [`ServiceRefused::NoProvider`] when nothing in the running set answers
/// a service a mod requires, [`ServiceRefused::Cycle`] naming every mod in a
/// requirement cycle, [`ServiceRefused::ForeignDeclaration`] when a mod returns a
/// declaration owned by another identity, and [`ServiceRefused::Consumer`] when a
/// mod refused the services it was offered.
pub fn open_services(
    machine: &LifecycleMachine,
    mods: &mut [&mut dyn HostMod],
) -> Result<ServiceOrder, ServiceRefused> {
    let declarations = declared(machine, mods)?;
    let providers = providers(&declarations);
    let order = dependency_order(&declarations, &providers)?;

    arrange(mods, &order);
    for position in 0..mods.len() {
        let (started, remaining) = mods.split_at_mut(position);
        let consumer = &mut remaining[0];
        let Some(declaration) = declarations.get(consumer.id()) else {
            continue;
        };
        if declaration.requires().next().is_none() && declaration.may_use().next().is_none() {
            continue;
        }

        let mut directory = Started {
            caller: consumer.id().clone(),
            declaration,
            mods: started,
        };
        consumer
            .use_services(&mut directory)
            .map_err(|source| ServiceRefused::Consumer {
                mod_id: source.mod_id().clone(),
                source,
            })?;
    }

    Ok(ServiceOrder { order })
}

/// Collects the declaration of every mod that runs on this side.
///
/// A declaration names its own owner, and this is where that name is checked
/// against the mod that returned it. Every other boundary in this design refuses
/// at the boundary — `NotOurs`, `RequiresItself` and `Undeclared` all do — and
/// without this one a mod handing back a declaration built for another identity
/// would have its edges recorded against itself and surface the mismatch far
/// later, as a provider nothing can find at call time.
///
/// # Errors
///
/// Returns [`ServiceRefused::ForeignDeclaration`] naming both identities.
fn declared(
    machine: &LifecycleMachine,
    mods: &[&mut dyn HostMod],
) -> Result<BTreeMap<ModId, ServiceDeclaration>, ServiceRefused> {
    let mut declarations = BTreeMap::new();
    for host in mods {
        let loads_here = machine
            .entry(host.id())
            .is_ok_and(|entry| entry.sides.loads_on(machine.side()));
        if !loads_here {
            continue;
        }
        let Some(declaration) = host.service_declaration() else {
            continue;
        };
        if declaration.owner() != host.id() {
            return Err(ServiceRefused::ForeignDeclaration {
                mod_id: host.id().clone(),
                owner: declaration.owner().clone(),
            });
        }
        declarations.insert(host.id().clone(), declaration);
    }
    Ok(declarations)
}

/// Maps every answered service to the mod that answers it.
fn providers(declarations: &BTreeMap<ModId, ServiceDeclaration>) -> BTreeMap<ServiceId, ModId> {
    declarations
        .iter()
        .flat_map(|(mod_id, declaration)| {
            declaration
                .provides()
                .map(move |service| (service.clone(), mod_id.clone()))
        })
        .collect()
}

/// Returns the mods in provider-before-consumer order.
///
/// The order itself is [`ravel_core::precedence`], which every surface that
/// orders mods among each other shares, so a service graph and a drawing stage
/// cannot disagree about what "after" means. What stays here is the part that is
/// about services: turning a required service into an edge, and refusing a
/// service nothing provides.
///
/// # Optional edges
///
/// A service a mod declared it *may use* becomes an ordinary edge when something
/// running answers it, and no edge at all when nothing does. Presence is a fact
/// about the resolved set rather than about iteration order, so the graph — and
/// therefore the order — is the same however the caller supplied the mods.
///
/// A present optional edge is a **real** edge, including for cycle detection. The
/// alternative was to drop optional edges to break a cycle, and it was rejected:
/// dropping one runs the consumer before a provider that *is* installed, so its
/// call would have to report absence for a mod the player can see in the list.
/// "Is it installed" would then be answered by the shape of the graph rather than
/// by the install, and there is no non-arbitrary rule for which edge to drop —
/// the same reason [`ServiceRefused::Cycle`] names every participant instead of
/// picking one. So an optional declaration changes exactly one thing: whether a
/// missing provider refuses the set.
fn dependency_order(
    declarations: &BTreeMap<ModId, ServiceDeclaration>,
    providers: &BTreeMap<ServiceId, ModId>,
) -> Result<Vec<ModId>, ServiceRefused> {
    let mut consumers: BTreeMap<ModId, BTreeSet<ModId>> = BTreeMap::new();

    for (mod_id, declaration) in declarations {
        for service in declaration.requires() {
            let provider = providers
                .get(service)
                .ok_or_else(|| ServiceRefused::NoProvider {
                    consumer: mod_id.clone(),
                    service: service.clone(),
                })?;
            consumers
                .entry(provider.clone())
                .or_default()
                .insert(mod_id.clone());
        }
        for service in declaration.may_use() {
            let Some(provider) = providers.get(service) else {
                continue;
            };
            consumers
                .entry(provider.clone())
                .or_default()
                .insert(mod_id.clone());
        }
    }

    precedence(declarations.keys().cloned(), &consumers).map_err(|cycle| ServiceRefused::Cycle {
        participants: cycle.participants().to_vec(),
    })
}

/// Rearranges `mods` so the ones `order` names come first, in that order.
///
/// A mod with no declaration keeps its relative position after them: it takes no
/// part in the graph, so nothing about the order is about it.
fn arrange(mods: &mut [&mut dyn HostMod], order: &[ModId]) {
    let mut placed = 0;
    for mod_id in order {
        if let Some(found) = mods[placed..].iter().position(|host| host.id() == mod_id) {
            mods.swap(placed, placed + found);
            placed += 1;
        }
    }
}

/// A directory over the mods already started.
struct Started<'a, 'm> {
    caller: ModId,
    declaration: &'a ServiceDeclaration,
    mods: &'a mut [&'m mut dyn HostMod],
}

impl ServiceDirectory for Started<'_, '_> {
    fn call(&mut self, request: &ServiceRequest) -> Result<ServiceResponse, ServiceCallFailed> {
        if !self.declaration.calls(&request.id) {
            return Err(ServiceCallFailed::Unavailable(
                ServiceUnavailable::Undeclared {
                    caller: self.caller.clone(),
                    service: request.id.clone(),
                },
            ));
        }
        // Nothing before this mod in the order answers it. For a required service
        // that is a set the loader should already have refused; for an optional one
        // it is the answer — nothing running provides it, because a provider that
        // did would have been ordered ahead of this call.
        let Some(provider) = self
            .mods
            .iter_mut()
            .find(|host| host.id() == request.id.owner())
        else {
            let service = request.id.clone();
            return Err(ServiceCallFailed::Unavailable(
                if self.declaration.may_do_without(&service) {
                    ServiceUnavailable::Absent { service }
                } else {
                    ServiceUnavailable::NoProvider { service }
                },
            ));
        };
        provider
            .answer_service(request)
            .map_err(ServiceCallFailed::Refused)
    }
}

/// Why a set's services did not open.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServiceRefused {
    /// Nothing running in this set answers a service a mod requires.
    NoProvider {
        /// The mod that requires it.
        consumer: ModId,
        /// The service nothing answers.
        service: ServiceId,
    },
    /// Two or more mods require each other's services.
    ///
    /// Every mod in the cycle is named, in identity order, because a cycle has no
    /// first offender: naming one of them would be picking an arbitrary member and
    /// telling the player to fix that one.
    Cycle {
        /// Every mod the cycle holds, in identity order.
        participants: Vec<ModId>,
    },
    /// A mod returned a declaration built for another identity.
    ForeignDeclaration {
        /// The mod that returned it.
        mod_id: ModId,
        /// The identity the declaration names as its owner.
        owner: ModId,
    },
    /// A mod refused the services it was offered.
    Consumer {
        /// The mod that refused.
        mod_id: ModId,
        /// What it said.
        source: HostError,
    },
}

impl fmt::Display for ServiceRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NoProvider { consumer, service } => write!(
                formatter,
                "`{consumer}` requires `{service}` and nothing running answers it"
            ),
            Self::Cycle { participants } => {
                formatter.write_str("these mods require each other's services: ")?;
                for (position, mod_id) in participants.iter().enumerate() {
                    if position > 0 {
                        formatter.write_str(", ")?;
                    }
                    write!(formatter, "`{mod_id}`")?;
                }
                Ok(())
            }
            Self::ForeignDeclaration { mod_id, owner } => write!(
                formatter,
                "`{mod_id}` returned a service declaration owned by `{owner}`, and a mod declares \
                 only its own services"
            ),
            Self::Consumer { mod_id, source } => {
                write!(formatter, "`{mod_id}` refused its services: {source}")
            }
        }
    }
}

impl std::error::Error for ServiceRefused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::NoProvider { .. } | Self::Cycle { .. } | Self::ForeignDeclaration { .. } => None,
            Self::Consumer { source, .. } => Some(source),
        }
    }
}

impl StructuredFailure for ServiceRefused {
    /// A missing provider and a cycle are the service graph's own forms of
    /// `SPEC.md` §9.2's dependency-graph outcomes, so they report them: a set
    /// whose services do not resolve fails closed the way one whose packages do
    /// not resolve does. A consumer's own refusal is not a graph outcome — the
    /// graph resolved and the mod declined — so it borrows no kind.
    fn failure_kind(&self) -> Option<FailureKind> {
        match self {
            Self::NoProvider { .. } => Some(FailureKind::MissingRequiredDependency),
            Self::Cycle { .. } => Some(FailureKind::DependencyCycle),
            // A foreign declaration is not a graph outcome either: the graph was
            // never built, because the mod answered for somebody else.
            Self::Consumer { .. } | Self::ForeignDeclaration { .. } => None,
        }
    }

    fn lifecycle_state(&self) -> LifecycleState {
        match self {
            Self::NoProvider { .. } | Self::Cycle { .. } | Self::ForeignDeclaration { .. } => {
                LifecycleState::Resolved
            }
            Self::Consumer { .. } => LifecycleState::Constructed,
        }
    }

    /// A consumer's refusal carries the sentence that mod wrote.
    fn quotes_untrusted_text(&self) -> bool {
        matches!(self, Self::Consumer { .. })
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use ravel_core::{ContentId, LifecycleState, ModId, Side, SideSet};
    use ravel_host_api::{
        HostError, HostMod, ServiceDeclaration, ServiceDirectory, ServiceId, ServiceRequest,
        ServiceResponse,
    };
    use serde_json::json;

    use super::{ServiceRefused, open_services};
    use crate::lifecycle::{LifecycleEntry, LifecycleMachine, Requirement};

    fn id(value: &str) -> ModId {
        value.parse().expect("a legal mod id")
    }

    fn service(owner: &str, path: &str) -> ServiceId {
        ServiceId::new(ContentId::new(id(owner), path).expect("a legal content path"))
    }

    /// A mod that answers with its own identity and records what it reached.
    struct Wired {
        id: ModId,
        declaration: ServiceDeclaration,
        answered: Vec<ServiceId>,
        reached: Vec<String>,
        refuses: bool,
    }

    impl Wired {
        fn new(name: &str, provides: &[ServiceId], requires: &[ServiceId]) -> Self {
            Self {
                id: id(name),
                declaration: ServiceDeclaration::new(
                    id(name),
                    provides.to_vec(),
                    requires.to_vec(),
                )
                .expect("a legal declaration"),
                answered: Vec::new(),
                reached: Vec::new(),
                refuses: false,
            }
        }

        fn refusing(mut self) -> Self {
            self.refuses = true;
            self
        }

        /// Hands back a declaration built for `owner` rather than for this mod.
        fn answering_for(mut self, owner: &str, provides: &[ServiceId]) -> Self {
            self.declaration = ServiceDeclaration::new(id(owner), provides.to_vec(), Vec::new())
                .expect("a legal declaration for the other identity");
            self
        }
    }

    impl HostMod for Wired {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
            Ok(())
        }

        fn service_declaration(&self) -> Option<ServiceDeclaration> {
            Some(self.declaration.clone())
        }

        fn answer_service(
            &mut self,
            request: &ServiceRequest,
        ) -> Result<ServiceResponse, HostError> {
            self.answered.push(request.id.clone());
            Ok(ServiceResponse::new(json!(self.id.as_str())))
        }

        fn use_services(&mut self, directory: &mut dyn ServiceDirectory) -> Result<(), HostError> {
            if self.refuses {
                return Err(HostError::Refused {
                    mod_id: self.id.clone(),
                    state: LifecycleState::Constructed,
                    detail: "the answer was not one this mod accepts".to_owned(),
                });
            }
            let requested: Vec<ServiceId> = self.declaration.requires().cloned().collect();
            for wanted in requested {
                let answer = directory
                    .call(&ServiceRequest::new(wanted, json!(null)))
                    .map_err(|failed| HostError::Refused {
                        mod_id: self.id.clone(),
                        state: LifecycleState::Constructed,
                        detail: failed.to_string(),
                    })?;
                self.reached
                    .push(answer.value().as_str().unwrap_or_default().to_owned());
            }
            Ok(())
        }
    }

    fn entry(name: &str, sides: &[Side]) -> LifecycleEntry {
        LifecycleEntry {
            mod_id: id(name),
            sides: SideSet::new(sides.iter().copied()).expect("a non-empty side set"),
            requirement: Requirement::Required,
            saved_data_versions: BTreeSet::new(),
        }
    }

    fn machine(entries: Vec<LifecycleEntry>) -> LifecycleMachine {
        LifecycleMachine::new(Side::DedicatedServer, entries).expect("a legal plan")
    }

    #[test]
    fn a_provider_is_offered_its_services_before_the_mod_that_requires_them() {
        let mut consumer = Wired::new("consumer", &[], &[service("base", "prices")]);
        let mut base = Wired::new("base", &[service("base", "prices")], &[]);
        let machine = machine(vec![
            entry("consumer", &[Side::DedicatedServer]),
            entry("base", &[Side::DedicatedServer]),
        ]);

        // The caller supplies the consumer first, so an order that simply used the
        // slice would have called it before its provider existed.
        let opened =
            open_services(&machine, &mut [&mut consumer, &mut base]).expect("the graph resolves");

        assert_eq!(opened.order(), [id("base"), id("consumer")]);
        assert_eq!(consumer.reached, ["base"]);
        assert_eq!(base.answered, [service("base", "prices")]);
    }

    #[test]
    fn a_mod_answering_for_another_identity_is_refused_before_any_mod_is_offered_anything() {
        // `thief` hands back `base`'s declaration. Without the owner check its
        // edges are recorded against `thief`, and the consumer's call for
        // `base:prices` then finds no provider at all — a refusal one step from
        // where the mistake was made and naming nobody who can fix it.
        let prices = service("base", "prices");
        let mut thief =
            Wired::new("thief", &[], &[]).answering_for("base", std::slice::from_ref(&prices));
        let mut consumer = Wired::new("consumer", &[], std::slice::from_ref(&prices));
        let machine = machine(vec![
            entry("thief", &[Side::DedicatedServer]),
            entry("consumer", &[Side::DedicatedServer]),
        ]);

        let refused = open_services(&machine, &mut [&mut thief, &mut consumer])
            .expect_err("a mod declares only its own services");

        assert_eq!(
            refused,
            ServiceRefused::ForeignDeclaration {
                mod_id: id("thief"),
                owner: id("base"),
            }
        );
        let text = refused.to_string();
        assert!(text.contains("thief"), "{text}");
        assert!(text.contains("base"), "{text}");
        assert!(
            consumer.reached.is_empty() && thief.answered.is_empty(),
            "the refusal came before anything was wired"
        );
    }

    #[test]
    fn a_chain_of_three_is_opened_from_the_end_nothing_depends_on() {
        let mut top = Wired::new("top", &[], &[service("middle", "sum")]);
        let mut middle = Wired::new(
            "middle",
            &[service("middle", "sum")],
            &[service("base", "prices")],
        );
        let mut base = Wired::new("base", &[service("base", "prices")], &[]);
        let machine = machine(vec![
            entry("top", &[Side::DedicatedServer]),
            entry("middle", &[Side::DedicatedServer]),
            entry("base", &[Side::DedicatedServer]),
        ]);

        let opened = open_services(&machine, &mut [&mut top, &mut middle, &mut base])
            .expect("the chain resolves");

        assert_eq!(opened.order(), [id("base"), id("middle"), id("top")]);
        assert_eq!(middle.reached, ["base"]);
        assert_eq!(top.reached, ["middle"]);
    }

    #[test]
    fn a_requirement_cycle_names_every_mod_in_it_and_starts_none_of_them() {
        let mut first = Wired::new(
            "first",
            &[service("first", "left")],
            &[service("second", "right")],
        );
        let mut second = Wired::new(
            "second",
            &[service("second", "right")],
            &[service("first", "left")],
        );
        let machine = machine(vec![
            entry("first", &[Side::DedicatedServer]),
            entry("second", &[Side::DedicatedServer]),
        ]);

        let refusal = open_services(&machine, &mut [&mut first, &mut second])
            .expect_err("no order starts either");
        assert_eq!(
            refusal,
            ServiceRefused::Cycle {
                participants: vec![id("first"), id("second")],
            }
        );
        for name in ["first", "second"] {
            assert!(refusal.to_string().contains(name), "{refusal}");
        }
        assert!(first.reached.is_empty() && second.reached.is_empty());
        assert!(first.answered.is_empty() && second.answered.is_empty());
    }

    #[test]
    fn a_missing_provider_names_the_mod_that_wanted_it_and_the_service() {
        let mut consumer = Wired::new("consumer", &[], &[service("absent", "prices")]);
        let machine = machine(vec![entry("consumer", &[Side::DedicatedServer])]);

        let refusal = open_services(&machine, &mut [&mut consumer])
            .expect_err("nothing answers `absent:prices`");
        assert_eq!(
            refusal,
            ServiceRefused::NoProvider {
                consumer: id("consumer"),
                service: service("absent", "prices"),
            }
        );
        assert!(refusal.to_string().contains("absent:prices"), "{refusal}");
        assert!(consumer.reached.is_empty());
    }

    #[test]
    fn a_provider_this_side_parks_is_a_missing_provider_rather_than_a_wait() {
        // The provider is installed and declares the client alone, so it never
        // runs here. A consumer that required it must be refused now: waiting for
        // a mod that will not run is the deadlock this rule avoids.
        let mut consumer = Wired::new("consumer", &[], &[service("base", "prices")]);
        let mut base = Wired::new("base", &[service("base", "prices")], &[]);
        let machine = machine(vec![
            entry("consumer", &[Side::DedicatedServer]),
            entry("base", &[Side::Client]),
        ]);

        assert_eq!(
            open_services(&machine, &mut [&mut consumer, &mut base]),
            Err(ServiceRefused::NoProvider {
                consumer: id("consumer"),
                service: service("base", "prices"),
            })
        );
        assert!(base.answered.is_empty(), "a parked mod answered a call");
    }

    #[test]
    fn a_consumer_that_refuses_stops_the_walk_and_leaves_the_rest_unwired() {
        // `middle` refuses, so `top` — which comes after it in the order — is never
        // offered its services at all. Half a wired set is what this is about: the
        // caller has to be able to refuse the whole thing.
        let mut top = Wired::new("top", &[], &[service("middle", "sum")]);
        let mut middle = Wired::new(
            "middle",
            &[service("middle", "sum")],
            &[service("base", "prices")],
        )
        .refusing();
        let mut base = Wired::new("base", &[service("base", "prices")], &[]);
        let machine = machine(vec![
            entry("top", &[Side::DedicatedServer]),
            entry("middle", &[Side::DedicatedServer]),
            entry("base", &[Side::DedicatedServer]),
        ]);

        let refusal = open_services(&machine, &mut [&mut top, &mut middle, &mut base])
            .expect_err("the middle mod refuses");
        assert!(matches!(
            refusal,
            ServiceRefused::Consumer { ref mod_id, .. } if mod_id == &id("middle")
        ));
        assert!(refusal.to_string().contains("middle"), "{refusal}");
        assert!(
            top.reached.is_empty(),
            "a mod after the refusal was wired anyway"
        );
        assert!(
            middle.answered.is_empty(),
            "the refusing mod answered a call it never got to make"
        );
    }

    #[test]
    fn a_mod_that_calls_a_service_it_did_not_declare_is_refused_by_the_directory() {
        // The directory checks the caller's own declaration, so the graph the order
        // was built from is the graph that is actually enforced at call time.
        struct Sneaking {
            id: ModId,
        }

        impl HostMod for Sneaking {
            fn id(&self) -> &ModId {
                &self.id
            }

            fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
                Ok(())
            }

            fn service_declaration(&self) -> Option<ServiceDeclaration> {
                ServiceDeclaration::new(
                    self.id.clone(),
                    Vec::new(),
                    vec![service("base", "prices")],
                )
                .ok()
            }

            fn use_services(
                &mut self,
                directory: &mut dyn ServiceDirectory,
            ) -> Result<(), HostError> {
                let failed = directory
                    .call(&ServiceRequest::new(service("base", "stock"), json!(null)))
                    .expect_err("undeclared");
                Err(HostError::Refused {
                    mod_id: self.id.clone(),
                    state: LifecycleState::Constructed,
                    detail: failed.to_string(),
                })
            }
        }

        let mut sneaking = Sneaking { id: id("consumer") };
        let mut base = Wired::new(
            "base",
            &[service("base", "prices"), service("base", "stock")],
            &[],
        );
        let machine = machine(vec![
            entry("consumer", &[Side::DedicatedServer]),
            entry("base", &[Side::DedicatedServer]),
        ]);

        let refusal = open_services(&machine, &mut [&mut sneaking, &mut base])
            .expect_err("the call was not declared");
        assert!(
            refusal.to_string().contains("does not require"),
            "{refusal}"
        );
        assert!(
            base.answered.is_empty(),
            "an undeclared call reached the provider"
        );
    }

    #[test]
    fn a_set_that_declares_no_service_at_all_opens_and_names_nobody() {
        struct Plain {
            id: ModId,
        }

        impl HostMod for Plain {
            fn id(&self) -> &ModId {
                &self.id
            }

            fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
                Ok(())
            }
        }

        let mut plain = Plain { id: id("plain") };
        let machine = machine(vec![entry("plain", &[Side::DedicatedServer])]);
        let opened =
            open_services(&machine, &mut [&mut plain]).expect("nothing to resolve resolves");
        assert!(opened.order().is_empty());
    }

    /// A consumer that declares a service optional and carries on without it.
    ///
    /// Deliberately not a mode of [`Wired`]: the existing fixtures state the
    /// required contract, and a shared fixture taught to tolerate absence would
    /// stop stating it.
    struct Hedging {
        id: ModId,
        declaration: ServiceDeclaration,
        reached: Vec<String>,
        absent: Vec<ServiceId>,
    }

    impl Hedging {
        fn new(name: &str, provides: &[ServiceId], may_use: &[ServiceId]) -> Self {
            Self {
                id: id(name),
                declaration: ServiceDeclaration::with_optional(
                    id(name),
                    provides.to_vec(),
                    Vec::new(),
                    may_use.to_vec(),
                )
                .expect("a legal declaration"),
                reached: Vec::new(),
                absent: Vec::new(),
            }
        }
    }

    impl HostMod for Hedging {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
            Ok(())
        }

        fn service_declaration(&self) -> Option<ServiceDeclaration> {
            Some(self.declaration.clone())
        }

        fn answer_service(
            &mut self,
            _request: &ServiceRequest,
        ) -> Result<ServiceResponse, HostError> {
            Ok(ServiceResponse::new(json!(self.id.as_str())))
        }

        /// Carries on past absence and past nothing else.
        ///
        /// The branch is on the outcome's own shape rather than on its sentence,
        /// which is the property this fixture is here to exercise: a provider that
        /// was reached and refused still refuses this mod.
        fn use_services(&mut self, directory: &mut dyn ServiceDirectory) -> Result<(), HostError> {
            let optional: Vec<ServiceId> = self.declaration.may_use().cloned().collect();
            for wanted in optional {
                match directory.call(&ServiceRequest::new(wanted.clone(), json!(null))) {
                    Ok(answer) => self
                        .reached
                        .push(answer.value().as_str().unwrap_or_default().to_owned()),
                    Err(failed) if failed.is_absent() => self.absent.push(wanted),
                    Err(failed) => {
                        return Err(HostError::Refused {
                            mod_id: self.id.clone(),
                            state: LifecycleState::Constructed,
                            detail: failed.to_string(),
                        });
                    }
                }
            }
            Ok(())
        }
    }

    /// A provider that is installed, runs, and says no to every call.
    struct Grudging {
        id: ModId,
        declaration: ServiceDeclaration,
    }

    impl Grudging {
        fn new(name: &str, provides: &[ServiceId]) -> Self {
            Self {
                id: id(name),
                declaration: ServiceDeclaration::new(id(name), provides.to_vec(), [])
                    .expect("a legal declaration"),
            }
        }
    }

    impl HostMod for Grudging {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), HostError> {
            Ok(())
        }

        fn service_declaration(&self) -> Option<ServiceDeclaration> {
            Some(self.declaration.clone())
        }

        fn answer_service(
            &mut self,
            _request: &ServiceRequest,
        ) -> Result<ServiceResponse, HostError> {
            Err(HostError::Refused {
                mod_id: self.id.clone(),
                state: LifecycleState::Running,
                detail: "this provider is not answering today".to_owned(),
            })
        }
    }

    #[test]
    fn an_installed_optional_provider_is_composed_before_the_mod_that_may_use_it() {
        // The ordering half. A set that loads but wires the consumer first would
        // pass every "it still loads" assertion and be wrong only in a running game,
        // so the order and the answer are both checked here.
        let mut consumer = Hedging::new("consumer", &[], &[service("zenith", "prices")]);
        let mut base = Wired::new("zenith", &[service("zenith", "prices")], &[]);
        let machine = machine(vec![
            entry("consumer", &[Side::DedicatedServer]),
            entry("zenith", &[Side::DedicatedServer]),
        ]);

        // Supplied consumer-first, so a slice order would have called it too early.
        let opened =
            open_services(&machine, &mut [&mut consumer, &mut base]).expect("the graph resolves");

        assert_eq!(opened.order(), [id("zenith"), id("consumer")]);
        assert_eq!(consumer.reached, ["zenith"]);
        assert!(
            consumer.absent.is_empty(),
            "an installed provider read as absent"
        );
        assert_eq!(base.answered, [service("zenith", "prices")]);
    }

    #[test]
    fn a_set_whose_optional_provider_is_not_installed_loads_and_the_call_reports_absence() {
        // The same declaration as a requirement would refuse the whole set here.
        let mut consumer = Hedging::new("consumer", &[], &[service("absent", "prices")]);
        let machine = machine(vec![entry("consumer", &[Side::DedicatedServer])]);

        let opened =
            open_services(&machine, &mut [&mut consumer]).expect("an absent provider refuses none");

        assert_eq!(opened.order(), [id("consumer")]);
        assert_eq!(consumer.absent, [service("absent", "prices")]);
        assert!(consumer.reached.is_empty());
    }

    #[test]
    fn a_required_service_nothing_answers_still_refuses_the_set_beside_an_optional_one() {
        // The two sides in one declaration: the optional name is missing and says
        // nothing, the required name is missing and refuses.
        let mut consumer = Hedging::new("consumer", &[], &[service("absent", "prices")]);
        consumer.declaration = ServiceDeclaration::with_optional(
            id("consumer"),
            [],
            [service("gone", "clock")],
            [service("absent", "prices")],
        )
        .expect("a legal declaration");
        let machine = machine(vec![entry("consumer", &[Side::DedicatedServer])]);

        assert_eq!(
            open_services(&machine, &mut [&mut consumer]),
            Err(ServiceRefused::NoProvider {
                consumer: id("consumer"),
                service: service("gone", "clock"),
            })
        );
    }

    #[test]
    fn a_provider_that_answered_and_refused_is_not_absence() {
        // The distinction a consumer branches on. `base` is installed and running,
        // so its refusal is an answer about the call and not about the install —
        // and the consumer must not swallow it as "nobody there".
        let mut consumer = Hedging::new("consumer", &[], &[service("zenith", "prices")]);
        let mut base = Grudging::new("zenith", &[service("zenith", "prices")]);
        let machine = machine(vec![
            entry("consumer", &[Side::DedicatedServer]),
            entry("zenith", &[Side::DedicatedServer]),
        ]);

        let refusal = open_services(&machine, &mut [&mut consumer, &mut base])
            .expect_err("the provider said no");

        assert!(matches!(
            refusal,
            ServiceRefused::Consumer { ref mod_id, .. } if mod_id == &id("consumer")
        ));
        assert!(
            refusal.to_string().contains("not answering today"),
            "{refusal}"
        );
        assert!(
            consumer.absent.is_empty(),
            "a refusal from a running provider was read as absence"
        );
    }

    #[test]
    fn a_cycle_through_an_installed_optional_provider_is_refused_like_any_other() {
        // `first` may use `second`'s service and `second` requires `first`'s. Both
        // are installed, so the optional edge is real and there is no order. Dropping
        // it to break the cycle was rejected: it would run `first` before a `second`
        // the player has installed and tell it `second` is not there.
        let mut first = Hedging::new(
            "first",
            &[service("first", "left")],
            &[service("second", "right")],
        );
        let mut second = Wired::new(
            "second",
            &[service("second", "right")],
            &[service("first", "left")],
        );
        let machine = machine(vec![
            entry("first", &[Side::DedicatedServer]),
            entry("second", &[Side::DedicatedServer]),
        ]);

        let refusal = open_services(&machine, &mut [&mut first, &mut second])
            .expect_err("no order starts either");
        assert_eq!(
            refusal,
            ServiceRefused::Cycle {
                participants: vec![id("first"), id("second")],
            }
        );
        assert!(first.reached.is_empty() && first.absent.is_empty());
    }

    #[test]
    fn the_same_optional_edge_draws_no_cycle_when_its_provider_is_not_installed() {
        // The counterpart of the test above, and the row that stops "an optional edge
        // is a real edge" from being the same claim as "an optional edge always
        // exists": the identical declaration loads once `second` is gone.
        let mut first = Hedging::new(
            "first",
            &[service("first", "left")],
            &[service("second", "right")],
        );
        let machine = machine(vec![entry("first", &[Side::DedicatedServer])]);

        let opened = open_services(&machine, &mut [&mut first]).expect("no edge, no cycle");
        assert_eq!(opened.order(), [id("first")]);
        assert_eq!(first.absent, [service("second", "right")]);
    }

    #[test]
    fn an_optional_provider_this_side_parks_is_absent_rather_than_a_refusal_or_a_wait() {
        let mut consumer = Hedging::new("consumer", &[], &[service("zenith", "prices")]);
        let mut base = Wired::new("zenith", &[service("zenith", "prices")], &[]);
        let machine = machine(vec![
            entry("consumer", &[Side::DedicatedServer]),
            entry("zenith", &[Side::Client]),
        ]);

        let opened = open_services(&machine, &mut [&mut consumer, &mut base])
            .expect("a parked provider refuses no optional consumer");

        assert_eq!(opened.order(), [id("consumer")]);
        assert_eq!(consumer.absent, [service("zenith", "prices")]);
        assert!(base.answered.is_empty(), "a parked mod answered a call");
    }

    #[test]
    fn a_graph_holding_an_optional_edge_opens_in_the_same_order_whatever_order_it_arrives_in() {
        let build = || {
            (
                Wired::new("zenith", &[service("zenith", "prices")], &[]),
                Hedging::new("consumer", &[], &[service("zenith", "prices")]),
                Wired::new("other", &[service("other", "clock")], &[]),
            )
        };
        let entries = vec![
            entry("zenith", &[Side::DedicatedServer]),
            entry("consumer", &[Side::DedicatedServer]),
            entry("other", &[Side::DedicatedServer]),
        ];

        let (mut base, mut consumer, mut other) = build();
        let forward = open_services(
            &machine(entries.clone()),
            &mut [&mut base, &mut consumer, &mut other],
        )
        .expect("resolves");

        let (mut base, mut consumer, mut other) = build();
        let reversed = open_services(
            &machine(entries),
            &mut [&mut other, &mut consumer, &mut base],
        )
        .expect("resolves");

        assert_eq!(forward, reversed);
        assert_eq!(
            forward.order(),
            [id("other"), id("zenith"), id("consumer")],
            "the order is not the smallest topological one"
        );
        assert_eq!(consumer.reached, ["zenith"]);
    }

    #[test]
    fn a_mod_with_only_optional_services_is_still_offered_them() {
        // The guard that skips a mod with nothing to call reads both sides. Reading
        // only `requires` would silently never call an optional-only consumer, which
        // is indistinguishable from a working absent branch until a provider exists.
        let mut consumer = Hedging::new("consumer", &[], &[service("zenith", "prices")]);
        let mut base = Wired::new("zenith", &[service("zenith", "prices")], &[]);
        let machine = machine(vec![
            entry("consumer", &[Side::DedicatedServer]),
            entry("zenith", &[Side::DedicatedServer]),
        ]);

        open_services(&machine, &mut [&mut base, &mut consumer]).expect("resolves");
        assert_eq!(base.answered, [service("zenith", "prices")]);
        assert_eq!(consumer.reached, ["zenith"]);
    }

    #[test]
    fn the_same_declarations_open_in_the_same_order_whatever_order_they_arrive_in() {
        let build = || {
            (
                Wired::new("base", &[service("base", "prices")], &[]),
                Wired::new("consumer", &[], &[service("base", "prices")]),
                Wired::new("other", &[service("other", "clock")], &[]),
            )
        };
        let entries = vec![
            entry("base", &[Side::DedicatedServer]),
            entry("consumer", &[Side::DedicatedServer]),
            entry("other", &[Side::DedicatedServer]),
        ];

        let (mut base, mut consumer, mut other) = build();
        let forward = open_services(
            &machine(entries.clone()),
            &mut [&mut base, &mut consumer, &mut other],
        )
        .expect("resolves");

        let (mut base, mut consumer, mut other) = build();
        let reversed = open_services(
            &machine(entries),
            &mut [&mut other, &mut consumer, &mut base],
        )
        .expect("resolves");

        assert_eq!(forward, reversed);
        assert_eq!(
            forward.order(),
            [id("base"), id("consumer"), id("other")],
            "the order is not the smallest topological one"
        );
    }
}
