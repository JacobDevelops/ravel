use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use ravel_core::{
    FailureEnvironment, FailureKind, LifecycleState, ModId, StructuredFailure, precedence,
};
use ravel_host_api::{
    HostError, HostMod, ServiceCallFailed, ServiceDeclaration, ServiceDirectory, ServiceId,
    ServiceRequest, ServiceResponse, ServiceUnavailable,
};

use crate::lifecycle::{LifecycleError, LifecycleMachine, Requirement};

/// The order the set's services were opened in.
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

/// Opens services in lexicographically smallest provider-before-consumer order.
///
/// Only nonfailed mods on this side participate; directories contain providers whose services were opened earlier.
/// Present optional services add dependency edges; absent ones do not refuse the set.
/// A refusal stops opening services, and the caller must not replay callbacks already completed.
/// The loader calls this after registration and before entering `running`.
///
/// # Errors
///
/// Returns [`ServiceRefused`] for missing required providers, cycles, foreign declarations, or callback failure.
pub fn open_services<H: std::ops::DerefMut<Target: HostMod>>(
    loaded: &mut crate::LoadedMods<'_, H>,
    environment: &FailureEnvironment,
) -> Result<ServiceOrder, ServiceRefused> {
    let (machine, mods) = loaded.parts();
    open_with_context(
        &mut ServiceContext {
            machine,
            environment,
        },
        mods,
    )
}

struct ServiceContext<'a> {
    machine: &'a mut LifecycleMachine,
    environment: &'a FailureEnvironment,
}

impl ServiceContext<'_> {
    fn machine(&self) -> &LifecycleMachine {
        self.machine
    }

    fn fail(&mut self, source: HostError) -> Result<(), ServiceRefused> {
        let requirement = self
            .machine
            .fail_callback(&source, self.environment, None)
            .map_err(ServiceRefused::Lifecycle)?;
        if requirement == Requirement::Optional {
            return Ok(());
        }
        Err(ServiceRefused::Consumer {
            mod_id: source.mod_id().clone(),
            source,
        })
    }
}

fn open_with_context<H: std::ops::DerefMut<Target: HostMod>>(
    context: &mut ServiceContext<'_>,
    mods: &mut [H],
) -> Result<ServiceOrder, ServiceRefused> {
    let declarations = declared(context, mods)?;
    let providers = providers(&declarations);
    let mut order = dependency_order(&declarations, &providers)?;

    crate::loaded::arrange(mods, &order);
    for position in 0..mods.len() {
        let (started, remaining) = mods.split_at_mut(position);
        let consumer = &mut remaining[0];
        let Some(declaration) = declarations.get(consumer.id()) else {
            continue;
        };
        if context.machine().state(consumer.id()) == Ok(LifecycleState::Failed) {
            continue;
        }
        if declaration.requires().next().is_none() && declaration.may_use().next().is_none() {
            continue;
        }

        let mod_id = consumer.id().clone();
        let Ok(state) = context.machine().state(&mod_id) else {
            continue;
        };
        let mut directory = Started {
            caller: mod_id.clone(),
            declaration,
            mods: started,
            context,
            failure: None,
        };
        let outcome = crate::callback(&mod_id, state, || consumer.use_services(&mut directory));
        if let Some(error) = directory.failure {
            return Err(error);
        }
        if let Err(source) = outcome {
            context.fail(source)?;
        }
    }

    order.retain(|owner| context.machine().state(owner) != Ok(LifecycleState::Failed));
    Ok(ServiceOrder { order })
}

/// Collects declarations from mods on this side, rejecting foreign owners.
///
/// # Errors
///
/// Returns [`ServiceRefused::ForeignDeclaration`] naming both identities.
fn declared<H: std::ops::DerefMut<Target: HostMod>>(
    context: &mut ServiceContext<'_>,
    mods: &[H],
) -> Result<BTreeMap<ModId, ServiceDeclaration>, ServiceRefused> {
    let mut declarations = BTreeMap::new();
    for host in mods {
        let machine = context.machine();
        let loads_here = machine
            .entry(host.id())
            .is_ok_and(|entry| entry.sides.loads_on(machine.side()));
        if !loads_here || machine.state(host.id()) == Ok(LifecycleState::Failed) {
            continue;
        }
        let mod_id = host.id();
        let Ok(state) = machine.state(mod_id) else {
            continue;
        };
        let declaration = match crate::callback(mod_id, state, || host.service_declaration()) {
            Ok(Some(declaration)) => declaration,
            Ok(None) => continue,
            Err(source) => {
                context.fail(source)?;
                continue;
            }
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

/// Orders required and present optional service edges, rejecting missing required providers.
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

/// A directory over the mods already started.
struct Started<'a, 'm, H: std::ops::DerefMut<Target: HostMod>> {
    caller: ModId,
    declaration: &'a ServiceDeclaration,
    mods: &'a mut [H],
    context: &'a mut ServiceContext<'m>,
    failure: Option<ServiceRefused>,
}

impl<H: std::ops::DerefMut<Target: HostMod>> ServiceDirectory for Started<'_, '_, H> {
    fn call(&mut self, request: &ServiceRequest) -> Result<ServiceResponse, ServiceCallFailed> {
        if !self.declaration.calls(&request.id) {
            return Err(ServiceCallFailed::Unavailable(
                ServiceUnavailable::Undeclared {
                    caller: self.caller.clone(),
                    service: request.id.clone(),
                },
            ));
        }
        let Some(provider) = self.mods.iter_mut().find(|host| {
            host.id() == request.id.owner()
                && self.context.machine().state(host.id()) != Ok(LifecycleState::Failed)
        }) else {
            return Err(unavailable(self.declaration, request));
        };
        let mod_id = provider.id().clone();
        let Ok(state) = self.context.machine().state(&mod_id) else {
            return Err(unavailable(self.declaration, request));
        };
        if self.failure.is_some() {
            return Err(unavailable(self.declaration, request));
        }
        match crate::callback(&mod_id, state, || provider.answer_service(request)) {
            Ok(response) => Ok(response),
            Err(source) => {
                if let Err(error) = self.context.fail(source.clone()) {
                    self.failure = Some(error);
                }
                Err(ServiceCallFailed::Refused(source))
            }
        }
    }
}

fn unavailable(declaration: &ServiceDeclaration, request: &ServiceRequest) -> ServiceCallFailed {
    let service = request.id.clone();
    ServiceCallFailed::Unavailable(if declaration.may_do_without(&service) {
        ServiceUnavailable::Absent { service }
    } else {
        ServiceUnavailable::NoProvider { service }
    })
}

/// Why a set's services did not open.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServiceRefused {
    /// The lifecycle refused failure attribution.
    Lifecycle(LifecycleError),
    /// Nothing running in this set answers a service a mod requires.
    NoProvider {
        /// The mod that requires it.
        consumer: ModId,
        /// The service nothing answers.
        service: ServiceId,
    },
    /// Two or more mods require each other's services.
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
    /// A mod's service callback refused or panicked.
    Consumer {
        /// The owner whose callback failed.
        mod_id: ModId,
        /// What it said.
        source: HostError,
    },
}

impl fmt::Display for ServiceRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lifecycle(error) => error.fmt(formatter),
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
            Self::Lifecycle(error) => Some(error),
            Self::NoProvider { .. } | Self::Cycle { .. } | Self::ForeignDeclaration { .. } => None,
            Self::Consumer { source, .. } => Some(source),
        }
    }
}

impl StructuredFailure for ServiceRefused {
    /// Only missing providers and cycles are dependency-graph failures.
    fn failure_kind(&self) -> Option<FailureKind> {
        match self {
            Self::Lifecycle(error) => error.failure_kind(),
            Self::NoProvider { .. } => Some(FailureKind::MissingRequiredDependency),
            Self::Cycle { .. } => Some(FailureKind::DependencyCycle),
            Self::Consumer { .. } | Self::ForeignDeclaration { .. } => None,
        }
    }

    fn lifecycle_state(&self) -> LifecycleState {
        match self {
            Self::Lifecycle(error) => error.lifecycle_state(),
            Self::NoProvider { .. } | Self::Cycle { .. } | Self::ForeignDeclaration { .. } => {
                LifecycleState::Resolved
            }
            Self::Consumer { source, .. } => source.state(),
        }
    }

    /// A consumer's refusal carries the sentence that mod wrote.
    fn quotes_untrusted_text(&self) -> bool {
        match self {
            Self::Consumer { .. } => true,
            Self::Lifecycle(error) => error.quotes_untrusted_text(),
            Self::NoProvider { .. } | Self::Cycle { .. } | Self::ForeignDeclaration { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{ContentId, LifecycleState, ModId, Side};
    use ravel_host_api::{
        HostError, HostMod, ModError, ServiceDeclaration, ServiceDirectory, ServiceId,
        ServiceRequest, ServiceResponse,
    };
    use serde_json::json;

    use super::{ServiceRefused, open_services};
    use crate::lifecycle::{LifecycleEntry, LifecycleMachine, Requirement};
    use crate::loaded::LoadedMods;
    use crate::tests::entry;

    fn environment() -> ravel_core::FailureEnvironment {
        ravel_core::FailureEnvironment {
            ravel: "0.1.0".parse().expect("valid version"),
            minecraft: "Synthetic26.1.5".to_owned(),
            platform: "test".to_owned(),
            architecture: "test".to_owned(),
        }
    }

    struct Failing {
        id: ModId,
        phase: &'static str,
    }

    impl HostMod for Failing {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
            Ok(())
        }

        fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
            assert_ne!(self.phase, "declaration", "declaration panic");
            Ok(Some(
                ServiceDeclaration::new(self.id.clone(), [], [service("provider", "answer")])
                    .expect("valid declaration"),
            ))
        }

        fn use_services(&mut self, _directory: &mut dyn ServiceDirectory) -> Result<(), ModError> {
            assert_ne!(self.phase, "consumer", "consumer panic");
            Err(ModError::Unsupported)
        }

        fn answer_service(
            &mut self,
            _request: &ServiceRequest,
        ) -> Result<ServiceResponse, ModError> {
            panic!("provider panic")
        }
    }

    #[test]
    fn service_callbacks_cannot_forge_the_consumer_or_unwind() {
        for phase in ["declaration", "consumer", "refusal"] {
            let mut host = Failing {
                id: id("actual"),
                phase,
            };
            let mut provider = Wired::new("provider", &[service("provider", "answer")], &[]);
            let mut machine = machine(vec![
                entry("actual", &[Side::DedicatedServer]),
                entry("provider", &[Side::DedicatedServer]),
            ]);
            let refusal = open_services(
                &mut LoadedMods::<&mut dyn HostMod>::new(
                    &mut machine,
                    &mut [&mut host, &mut provider],
                )
                .expect("all active hosts are supplied"),
                &environment(),
            )
            .expect_err("callback failure stays contained");
            let ServiceRefused::Consumer { mod_id, source } = refusal else {
                panic!("expected a consumer refusal");
            };
            assert_eq!(mod_id, id("actual"));
            assert_eq!(source.mod_id(), &id("actual"));
        }
    }

    #[test]
    fn provider_panics_are_contained_and_stamped_with_the_provider() {
        let mut provider = Failing {
            id: id("provider"),
            phase: "provider",
        };
        let declaration =
            ServiceDeclaration::new(id("consumer"), [], [service("provider", "answer")])
                .expect("valid declaration");
        let mut providers: [&mut dyn HostMod; 1] = [&mut provider];
        let mut machine = machine(vec![entry("provider", &[Side::DedicatedServer])]);
        let environment = environment();
        let mut context = super::ServiceContext {
            machine: &mut machine,
            environment: &environment,
        };
        let mut directory = super::Started {
            caller: id("consumer"),
            declaration: &declaration,
            mods: &mut providers,
            context: &mut context,
            failure: None,
        };
        let error = directory
            .call(&ServiceRequest::new(
                service("provider", "answer"),
                json!(null),
            ))
            .expect_err("provider refusal is contained");
        assert!(matches!(
            &error,
            ravel_host_api::ServiceCallFailed::Refused(source) if source.mod_id() == &id("provider")
        ));
        let Some(ServiceRefused::Consumer { source, .. }) = directory.failure else {
            panic!("expected provider attribution");
        };
        assert_eq!(source.mod_id(), &id("provider"));
    }

    #[test]
    fn continuing_after_a_refusal_never_replays_completed_consumers() {
        let mut provider = Wired::new("provider", &[service("provider", "answer")], &[]);
        let mut healthy = Wired::new("consumer", &[], &[service("provider", "answer")]);
        let mut refusing = Failing {
            id: id("zfail"),
            phase: "refusal",
        };
        let mut failed = entry("zfail", &[Side::DedicatedServer]);
        failed.requirement = Requirement::Optional;
        let mut machine = machine(vec![
            entry("provider", &[Side::DedicatedServer]),
            entry("consumer", &[Side::DedicatedServer]),
            failed,
        ]);
        open_services(
            &mut LoadedMods::<&mut dyn HostMod>::new(
                &mut machine,
                &mut [&mut healthy, &mut refusing, &mut provider],
            )
            .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect("an optional refusal permits continuation");
        assert_eq!(machine.state(&id("zfail")), Ok(LifecycleState::Failed));
        let failure = machine
            .failure(&id("zfail"))
            .expect("known owner")
            .expect("optional refusal is retained");
        assert_eq!(failure.mod_id, Some(id("zfail")));
        assert_eq!(healthy.reached, ["provider"]);
        assert_eq!(provider.answered, [service("provider", "answer")]);
    }

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
        refuses_declaration: bool,
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
                refuses_declaration: false,
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

        fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
            Ok(())
        }

        fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
            if self.refuses_declaration {
                return Err(ModError::Unsupported);
            }
            Ok(Some(self.declaration.clone()))
        }

        fn answer_service(
            &mut self,
            request: &ServiceRequest,
        ) -> Result<ServiceResponse, ModError> {
            self.answered.push(request.id.clone());
            Ok(ServiceResponse::new(json!(self.id.as_str())))
        }

        fn use_services(&mut self, directory: &mut dyn ServiceDirectory) -> Result<(), ModError> {
            if self.refuses {
                return Err(ModError::Refused {
                    detail: "the answer was not one this mod accepts".to_owned(),
                });
            }
            let requested: Vec<ServiceId> = self.declaration.requires().cloned().collect();
            for wanted in requested {
                let answer = directory
                    .call(&ServiceRequest::new(wanted, json!(null)))
                    .map_err(|failed| ModError::Refused {
                        detail: failed.to_string(),
                    })?;
                self.reached
                    .push(answer.value().as_str().unwrap_or_default().to_owned());
            }
            Ok(())
        }
    }

    fn machine(entries: Vec<LifecycleEntry>) -> LifecycleMachine {
        LifecycleMachine::new(Side::DedicatedServer, entries).expect("a legal plan")
    }

    #[test]
    fn service_declaration_refusal_is_not_absence() {
        let mut host = Wired::new("refusing_mod", &[], &[]);
        host.refuses_declaration = true;
        let mut machine = machine(vec![entry("refusing_mod", &[Side::DedicatedServer])]);
        assert_eq!(
            open_services(
                &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut host])
                    .expect("all active hosts are supplied"),
                &environment(),
            ),
            Err(ServiceRefused::Consumer {
                mod_id: id("refusing_mod"),
                source: HostError::Unsupported {
                    mod_id: id("refusing_mod"),
                    state: LifecycleState::Discovered,
                },
            })
        );
        assert_eq!(host.reached, Vec::<String>::new());
    }

    #[test]
    fn a_provider_is_offered_its_services_before_the_mod_that_requires_them() {
        let mut consumer = Wired::new("consumer", &[], &[service("warehouse", "prices")]);
        let mut warehouse = Wired::new("warehouse", &[service("warehouse", "prices")], &[]);
        let mut machine = machine(vec![
            entry("consumer", &[Side::DedicatedServer]),
            entry("warehouse", &[Side::DedicatedServer]),
        ]);

        // The caller supplies the consumer first, so an order that simply used the
        // slice would have called it before its provider existed. The names are
        // chosen the same way: `consumer` sorts before `warehouse`, so an
        // implementation that only sorted would fail here too. It is worth saying
        // because it was not true — the pair used to be `base` and `consumer`, and
        // deleting the ordering edge left this test passing.
        let mut mods: [&mut dyn HostMod; 2] = [&mut consumer, &mut warehouse];
        let opened = open_services(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect("the graph resolves");

        assert_eq!(opened.order(), [id("warehouse"), id("consumer")]);
        assert_eq!(
            mods.iter()
                .map(|host| host.id().clone())
                .collect::<Vec<_>>(),
            [id("warehouse"), id("consumer")]
        );
        assert_eq!(consumer.reached, ["warehouse"]);
        assert_eq!(warehouse.answered, [service("warehouse", "prices")]);
    }

    #[test]
    fn a_mod_answering_for_another_identity_is_refused_before_any_mod_is_offered_anything() {
        // `thief` hands back `warehouse`'s declaration. Without the owner check its
        // edges are recorded against `thief`, and the consumer's call for
        // `warehouse:prices` then finds no provider at all — a refusal one step from
        // where the mistake was made and naming nobody who can fix it.
        let prices = service("warehouse", "prices");
        let mut thief =
            Wired::new("thief", &[], &[]).answering_for("warehouse", std::slice::from_ref(&prices));
        let mut consumer = Wired::new("consumer", &[], std::slice::from_ref(&prices));
        let mut machine = machine(vec![
            entry("thief", &[Side::DedicatedServer]),
            entry("consumer", &[Side::DedicatedServer]),
        ]);

        let refused = open_services(
            &mut LoadedMods::<&mut dyn HostMod>::new(
                &mut machine,
                &mut [&mut thief, &mut consumer],
            )
            .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect_err("a mod declares only its own services");

        assert_eq!(
            refused,
            ServiceRefused::ForeignDeclaration {
                mod_id: id("thief"),
                owner: id("warehouse"),
            }
        );
        let text = refused.to_string();
        assert!(text.contains("thief"), "{text}");
        assert!(text.contains("warehouse"), "{text}");
        assert!(
            consumer.reached.is_empty() && thief.answered.is_empty(),
            "the refusal came before anything was wired"
        );
    }

    #[test]
    fn a_chain_of_three_is_opened_from_the_end_nothing_depends_on() {
        // The three names sort in exactly the reverse of the order the chain
        // requires, so the only thing that can produce the expected order is the
        // edges. A chain named `base`, `middle`, `top` sorts into the right answer
        // on its own and says nothing.
        let mut anvil = Wired::new("anvil", &[], &[service("mill", "sum")]);
        let mut mill = Wired::new(
            "mill",
            &[service("mill", "sum")],
            &[service("warehouse", "prices")],
        );
        let mut warehouse = Wired::new("warehouse", &[service("warehouse", "prices")], &[]);
        let mut machine = machine(vec![
            entry("anvil", &[Side::DedicatedServer]),
            entry("mill", &[Side::DedicatedServer]),
            entry("warehouse", &[Side::DedicatedServer]),
        ]);

        let mut mods: [&mut dyn HostMod; 3] = [&mut anvil, &mut mill, &mut warehouse];
        let opened = open_services(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect("the chain resolves");

        assert_eq!(opened.order(), [id("warehouse"), id("mill"), id("anvil")]);
        assert_eq!(
            mods.iter()
                .map(|host| host.id().clone())
                .collect::<Vec<_>>(),
            [id("warehouse"), id("mill"), id("anvil")]
        );
        assert_eq!(mill.reached, ["warehouse"]);
        assert_eq!(anvil.reached, ["mill"]);
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
        let mut machine = machine(vec![
            entry("first", &[Side::DedicatedServer]),
            entry("second", &[Side::DedicatedServer]),
        ]);

        let refusal = open_services(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut first, &mut second])
                .expect("all active hosts are supplied"),
            &environment(),
        )
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
        let mut machine = machine(vec![entry("consumer", &[Side::DedicatedServer])]);

        let refusal = open_services(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut consumer])
                .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect_err("nothing answers `absent:prices`");
        assert_eq!(
            refusal,
            ServiceRefused::NoProvider {
                consumer: id("consumer"),
                service: service("absent", "prices"),
            }
        );
        assert!(refusal.to_string().contains("absent:prices"), "{refusal}");
        assert_eq!(consumer.reached, Vec::<String>::new());
    }

    #[test]
    fn a_provider_this_side_parks_is_a_missing_provider_rather_than_a_wait() {
        // The provider is installed and declares the client alone, so it never
        // runs here. A consumer that required it must be refused now: waiting for
        // a mod that will not run is the deadlock this rule avoids.
        let mut consumer = Wired::new("consumer", &[], &[service("warehouse", "prices")]);
        let mut warehouse = Wired::new("warehouse", &[service("warehouse", "prices")], &[]);
        let mut machine = machine(vec![
            entry("consumer", &[Side::DedicatedServer]),
            entry("warehouse", &[Side::Client]),
        ]);

        assert_eq!(
            open_services(
                &mut LoadedMods::<&mut dyn HostMod>::new(
                    &mut machine,
                    &mut [&mut consumer, &mut warehouse]
                )
                .expect("all active hosts are supplied"),
                &environment(),
            ),
            Err(ServiceRefused::NoProvider {
                consumer: id("consumer"),
                service: service("warehouse", "prices"),
            })
        );
        assert!(
            warehouse.answered.is_empty(),
            "a parked mod answered a call"
        );
    }

    #[test]
    fn a_consumer_that_refuses_stops_the_walk_and_leaves_the_rest_unwired() {
        // `mill` refuses, so `anvil` — which comes after it in the order — is never
        // offered its services at all. Half a wired set is what this is about: the
        // caller has to be able to refuse the whole thing.
        let mut anvil = Wired::new("anvil", &[], &[service("mill", "sum")]);
        let mut mill = Wired::new(
            "mill",
            &[service("mill", "sum")],
            &[service("warehouse", "prices")],
        )
        .refusing();
        let mut warehouse = Wired::new("warehouse", &[service("warehouse", "prices")], &[]);
        let mut machine = machine(vec![
            entry("anvil", &[Side::DedicatedServer]),
            entry("mill", &[Side::DedicatedServer]),
            entry("warehouse", &[Side::DedicatedServer]),
        ]);

        let refusal = open_services(
            &mut LoadedMods::<&mut dyn HostMod>::new(
                &mut machine,
                &mut [&mut anvil, &mut mill, &mut warehouse],
            )
            .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect_err("the mod in the mill of the chain refuses");
        assert!(matches!(
            refusal,
            ServiceRefused::Consumer { ref mod_id, .. } if mod_id == &id("mill")
        ));
        assert!(refusal.to_string().contains("mill"), "{refusal}");
        assert!(
            anvil.reached.is_empty(),
            "a mod after the refusal was wired anyway"
        );
        assert!(
            mill.answered.is_empty(),
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

            fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
                Ok(())
            }

            fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
                Ok(Some(
                    ServiceDeclaration::new(
                        self.id.clone(),
                        Vec::new(),
                        vec![service("warehouse", "prices")],
                    )
                    .expect("valid service declaration"),
                ))
            }

            fn use_services(
                &mut self,
                directory: &mut dyn ServiceDirectory,
            ) -> Result<(), ModError> {
                let failed = directory
                    .call(&ServiceRequest::new(
                        service("warehouse", "stock"),
                        json!(null),
                    ))
                    .expect_err("undeclared");
                Err(ModError::Refused {
                    detail: failed.to_string(),
                })
            }
        }

        let mut sneaking = Sneaking { id: id("consumer") };
        let mut warehouse = Wired::new(
            "warehouse",
            &[
                service("warehouse", "prices"),
                service("warehouse", "stock"),
            ],
            &[],
        );
        let mut machine = machine(vec![
            entry("consumer", &[Side::DedicatedServer]),
            entry("warehouse", &[Side::DedicatedServer]),
        ]);

        let refusal = open_services(
            &mut LoadedMods::<&mut dyn HostMod>::new(
                &mut machine,
                &mut [&mut sneaking, &mut warehouse],
            )
            .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect_err("the call was not declared");
        assert!(
            refusal.to_string().contains("does not require"),
            "{refusal}"
        );
        assert!(
            warehouse.answered.is_empty(),
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

            fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
                Ok(())
            }
        }

        let mut plain = Plain { id: id("plain") };
        let mut machine = machine(vec![entry("plain", &[Side::DedicatedServer])]);
        let opened = open_services(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut plain])
                .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect("nothing to resolve resolves");
        assert_eq!(opened.order(), []);
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
        refused: Vec<ServiceId>,
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
                refused: Vec::new(),
            }
        }
    }

    impl HostMod for Hedging {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
            Ok(())
        }

        fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
            Ok(Some(self.declaration.clone()))
        }

        fn answer_service(
            &mut self,
            _request: &ServiceRequest,
        ) -> Result<ServiceResponse, ModError> {
            Ok(ServiceResponse::new(json!(self.id.as_str())))
        }

        // Optional consumers distinguish absence from a reached provider's refusal.
        fn use_services(&mut self, directory: &mut dyn ServiceDirectory) -> Result<(), ModError> {
            let optional: Vec<ServiceId> = self.declaration.may_use().cloned().collect();
            for wanted in optional {
                match directory.call(&ServiceRequest::new(wanted.clone(), json!(null))) {
                    Ok(answer) => self
                        .reached
                        .push(answer.value().as_str().unwrap_or_default().to_owned()),
                    Err(failed) if failed.is_absent() => self.absent.push(wanted),
                    Err(ravel_host_api::ServiceCallFailed::Refused(_)) => self.refused.push(wanted),
                    Err(failed) => {
                        return Err(ModError::Refused {
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
        panics: bool,
    }

    impl Grudging {
        fn new(name: &str, provides: &[ServiceId]) -> Self {
            Self {
                id: id(name),
                declaration: ServiceDeclaration::new(id(name), provides.to_vec(), [])
                    .expect("a legal declaration"),
                panics: false,
            }
        }
    }

    impl HostMod for Grudging {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
            Ok(())
        }

        fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
            Ok(Some(self.declaration.clone()))
        }

        fn answer_service(
            &mut self,
            _request: &ServiceRequest,
        ) -> Result<ServiceResponse, ModError> {
            assert!(!self.panics, "provider answer panic");
            Err(ModError::Refused {
                detail: "this provider is not answering today".to_owned(),
            })
        }
    }

    #[test]
    fn an_installed_optional_provider_is_composed_before_the_mod_that_may_use_it() {
        // The ordering half. A set that loads but wires the consumer first would
        // pass every "it still loads" assertion and be wrong only in a running game,
        // so the order and the answer are both checked here.
        let mut consumer = Hedging::new("consumer", &[], &[service("warehouse", "prices")]);
        let mut warehouse = Wired::new("warehouse", &[service("warehouse", "prices")], &[]);
        let mut machine = machine(vec![
            entry("consumer", &[Side::DedicatedServer]),
            entry("warehouse", &[Side::DedicatedServer]),
        ]);

        // Supplied consumer-first, so a slice order would have called it too early.
        let mut mods: [&mut dyn HostMod; 2] = [&mut consumer, &mut warehouse];
        let opened = open_services(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect("the graph resolves");

        assert_eq!(opened.order(), [id("warehouse"), id("consumer")]);
        assert_eq!(
            mods.iter()
                .map(|host| host.id().clone())
                .collect::<Vec<_>>(),
            [id("warehouse"), id("consumer")]
        );
        assert_eq!(consumer.reached, ["warehouse"]);
        assert!(
            consumer.absent.is_empty(),
            "an installed provider read as absent"
        );
        assert_eq!(warehouse.answered, [service("warehouse", "prices")]);
    }

    #[test]
    fn a_set_whose_optional_provider_is_not_installed_loads_and_the_call_reports_absence() {
        // The same declaration as a requirement would refuse the whole set here.
        let mut consumer = Hedging::new("consumer", &[], &[service("absent", "prices")]);
        let mut machine = machine(vec![entry("consumer", &[Side::DedicatedServer])]);

        let opened = open_services(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut consumer])
                .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect("an absent provider refuses none");

        assert_eq!(opened.order(), [id("consumer")]);
        assert_eq!(consumer.absent, [service("absent", "prices")]);
        assert_eq!(consumer.reached, Vec::<String>::new());
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
        let mut machine = machine(vec![entry("consumer", &[Side::DedicatedServer])]);

        assert_eq!(
            open_services(
                &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut consumer])
                    .expect("all active hosts are supplied"),
                &environment(),
            ),
            Err(ServiceRefused::NoProvider {
                consumer: id("consumer"),
                service: service("gone", "clock"),
            })
        );
    }

    #[test]
    fn a_public_provider_refusal_is_attributed_to_its_owner() {
        let mut consumer = Hedging::new("consumer", &[], &[service("warehouse", "prices")]);
        let mut warehouse = Grudging::new("warehouse", &[service("warehouse", "prices")]);
        let mut machine = machine(vec![
            entry("consumer", &[Side::DedicatedServer]),
            entry("warehouse", &[Side::DedicatedServer]),
        ]);

        let refusal = open_services(
            &mut LoadedMods::<&mut dyn HostMod>::new(
                &mut machine,
                &mut [&mut consumer, &mut warehouse],
            )
            .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect_err("the provider said no");

        assert!(matches!(
            refusal,
            ServiceRefused::Consumer { ref mod_id, .. } if mod_id == &id("warehouse")
        ));
        assert!(
            refusal.to_string().contains("not answering today"),
            "{refusal}"
        );
        assert_eq!(consumer.absent, Vec::new());
        assert_eq!(consumer.refused, [service("warehouse", "prices")]);
        let failure = machine
            .failure(&id("warehouse"))
            .expect("known provider")
            .expect("recorded refusal");
        assert_eq!(failure.mod_id, Some(id("warehouse")));
        assert_eq!(failure.environment, environment());
    }

    #[test]
    fn drive_records_provider_failures_without_failing_optional_service_consumers() {
        for panics in [false, true] {
            for requirement in [Requirement::Optional, Requirement::Required] {
                provider_failure(panics, requirement);
            }
        }
    }

    fn provider_failure(panics: bool, requirement: Requirement) {
        let wanted = service("warehouse", "prices");
        let consumer = Hedging::new("consumer", &[], std::slice::from_ref(&wanted));
        let mut provider = Grudging::new("warehouse", std::slice::from_ref(&wanted));
        provider.panics = panics;
        let mut provider_entry = entry("warehouse", &[Side::DedicatedServer]);
        provider_entry.requirement = requirement;
        let environment = ravel_core::FailureEnvironment {
            ravel: "0.1.0".parse().expect("valid version"),
            minecraft: "Synthetic26.1.5".to_owned(),
            platform: "test".to_owned(),
            architecture: "test".to_owned(),
        };
        let composed: Vec<(LifecycleEntry, Box<dyn HostMod>)> = vec![
            (
                entry("consumer", &[Side::DedicatedServer]),
                Box::new(consumer),
            ),
            (provider_entry, Box::new(provider)),
        ];
        let result = crate::drive(Side::DedicatedServer, &environment, composed);
        if requirement == Requirement::Required {
            assert!(
                matches!(result, Err(crate::DriveError::Services(ServiceRefused::Consumer { mod_id, source }))
                if mod_id == id("warehouse")
                    && source.state() == LifecycleState::Registered)
            );
        } else {
            let (machine, _) = result.expect("the optional provider may fail");
            assert_eq!(machine.state(&id("warehouse")), Ok(LifecycleState::Failed));
            assert_eq!(machine.state(&id("consumer")), Ok(LifecycleState::Running));
            let attribution = machine
                .failure(&id("warehouse"))
                .expect("known owner")
                .expect("recorded provider failure");
            assert_eq!(attribution.state, LifecycleState::Registered);
            assert_eq!(attribution.environment, environment);
        }
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
        let mut machine = machine(vec![
            entry("first", &[Side::DedicatedServer]),
            entry("second", &[Side::DedicatedServer]),
        ]);

        let refusal = open_services(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut first, &mut second])
                .expect("all active hosts are supplied"),
            &environment(),
        )
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
        let mut machine = machine(vec![entry("first", &[Side::DedicatedServer])]);

        let opened = open_services(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine, &mut [&mut first])
                .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect("no edge, no cycle");
        assert_eq!(opened.order(), [id("first")]);
        assert_eq!(first.absent, [service("second", "right")]);
    }

    #[test]
    fn an_optional_provider_this_side_parks_is_absent_rather_than_a_refusal_or_a_wait() {
        let mut consumer = Hedging::new("consumer", &[], &[service("warehouse", "prices")]);
        let mut warehouse = Wired::new("warehouse", &[service("warehouse", "prices")], &[]);
        let mut machine = machine(vec![
            entry("consumer", &[Side::DedicatedServer]),
            entry("warehouse", &[Side::Client]),
        ]);

        let opened = open_services(
            &mut LoadedMods::<&mut dyn HostMod>::new(
                &mut machine,
                &mut [&mut consumer, &mut warehouse],
            )
            .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect("a parked provider refuses no optional consumer");

        assert_eq!(opened.order(), [id("consumer")]);
        assert_eq!(consumer.absent, [service("warehouse", "prices")]);
        assert!(
            warehouse.answered.is_empty(),
            "a parked mod answered a call"
        );
    }

    #[test]
    fn a_graph_holding_an_optional_edge_opens_in_the_same_order_whatever_order_it_arrives_in() {
        let build = || {
            (
                Wired::new("warehouse", &[service("warehouse", "prices")], &[]),
                Hedging::new("consumer", &[], &[service("warehouse", "prices")]),
                Wired::new("other", &[service("other", "clock")], &[]),
            )
        };
        let entries = vec![
            entry("warehouse", &[Side::DedicatedServer]),
            entry("consumer", &[Side::DedicatedServer]),
            entry("other", &[Side::DedicatedServer]),
        ];

        let (mut warehouse, mut consumer, mut other) = build();
        let mut mods: [&mut dyn HostMod; 3] = [&mut warehouse, &mut consumer, &mut other];
        let forward = open_services(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine(entries.clone()), &mut mods)
                .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect("resolves");
        assert_eq!(
            mods.iter()
                .map(|host| host.id().clone())
                .collect::<Vec<_>>(),
            forward.order()
        );

        let (mut warehouse, mut consumer, mut other) = build();
        let mut mods: [&mut dyn HostMod; 3] = [&mut other, &mut consumer, &mut warehouse];
        let reversed = open_services(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine(entries), &mut mods)
                .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect("resolves");
        assert_eq!(
            mods.iter()
                .map(|host| host.id().clone())
                .collect::<Vec<_>>(),
            reversed.order()
        );

        assert_eq!(forward, reversed);
        assert_eq!(
            forward.order(),
            [id("other"), id("warehouse"), id("consumer")],
            "the order is not the smallest topological one"
        );
        assert_eq!(consumer.reached, ["warehouse"]);
    }

    #[test]
    fn a_mod_with_only_optional_services_is_still_offered_them() {
        // The guard that skips a mod with nothing to call reads both sides. Reading
        // only `requires` would silently never call an optional-only consumer, which
        // is indistinguishable from a working absent branch until a provider exists.
        let mut consumer = Hedging::new("consumer", &[], &[service("warehouse", "prices")]);
        let mut warehouse = Wired::new("warehouse", &[service("warehouse", "prices")], &[]);
        let mut machine = machine(vec![
            entry("consumer", &[Side::DedicatedServer]),
            entry("warehouse", &[Side::DedicatedServer]),
        ]);

        open_services(
            &mut LoadedMods::<&mut dyn HostMod>::new(
                &mut machine,
                &mut [&mut warehouse, &mut consumer],
            )
            .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect("resolves");
        assert_eq!(warehouse.answered, [service("warehouse", "prices")]);
        assert_eq!(consumer.reached, ["warehouse"]);
    }

    #[test]
    fn the_same_declarations_open_in_the_same_order_whatever_order_they_arrive_in() {
        let build = || {
            (
                Wired::new("warehouse", &[service("warehouse", "prices")], &[]),
                Wired::new("consumer", &[], &[service("warehouse", "prices")]),
                Wired::new("other", &[service("other", "clock")], &[]),
            )
        };
        let entries = vec![
            entry("warehouse", &[Side::DedicatedServer]),
            entry("consumer", &[Side::DedicatedServer]),
            entry("other", &[Side::DedicatedServer]),
        ];

        let (mut warehouse, mut consumer, mut other) = build();
        let mut mods: [&mut dyn HostMod; 3] = [&mut warehouse, &mut consumer, &mut other];
        let forward = open_services(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine(entries.clone()), &mut mods)
                .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect("resolves");
        assert_eq!(
            mods.iter()
                .map(|host| host.id().clone())
                .collect::<Vec<_>>(),
            forward.order()
        );

        let (mut warehouse, mut consumer, mut other) = build();
        let mut mods: [&mut dyn HostMod; 3] = [&mut other, &mut consumer, &mut warehouse];
        let reversed = open_services(
            &mut LoadedMods::<&mut dyn HostMod>::new(&mut machine(entries), &mut mods)
                .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect("resolves");
        assert_eq!(
            mods.iter()
                .map(|host| host.id().clone())
                .collect::<Vec<_>>(),
            reversed.order()
        );

        assert_eq!(forward, reversed);
        // Not identity order, and that is the point of these three names: sorted
        // alone gives `consumer, other, warehouse`, so an implementation that only
        // sorted would be caught here rather than agreeing by accident.
        assert_eq!(
            forward.order(),
            [id("other"), id("warehouse"), id("consumer")],
            "the order is not the smallest topological one"
        );
    }
}
