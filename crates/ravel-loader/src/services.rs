use std::collections::{BTreeMap, BTreeSet};
use std::fmt;

use ravel_core::{
    FailureEnvironment, FailureKind, LifecycleState, ModId, StructuredFailure, precedence,
};
use ravel_host_api::{
    HostError, Mod, ServiceCallFailed, ServiceDeclaration, ServiceDirectory, ServiceId,
    ServiceRequest, ServiceResponse, ServiceUnavailable,
};

use crate::lifecycle::{LifecycleError, LifecycleMachine};

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
/// Callback order follows service dependencies; supplied mod slots retain resolved lifecycle order.
/// Present optional services add dependency edges; absent ones do not refuse the set.
/// Required failures stop opening; optional failures are isolated before world mutation.
/// The loader calls this after registration and before entering `running`.
///
/// # Errors
///
/// Returns [`ServiceRefused`] for cycles or required/halting owner failures.
/// Missing required services isolate optional consumers before world mutation;
/// otherwise they refuse the set.
pub fn open_services<H: std::ops::DerefMut<Target: Mod>>(
    loaded: &mut crate::LoadedMods<'_, H>,
    environment: &FailureEnvironment,
) -> Result<ServiceOrder, ServiceRefused> {
    let (machine, mods) = loaded.parts();
    if let Some(by) = crate::events::active_blocker(machine) {
        return Err(ServiceRefused::Halted { by: by.clone() });
    }
    let result = open_with_context(
        &mut ServiceContext {
            machine,
            environment,
            failures: BTreeMap::new(),
        },
        mods,
    );
    crate::loaded::arrange(mods, machine.order());
    result
}

struct ServiceContext<'a> {
    machine: &'a mut LifecycleMachine,
    environment: &'a FailureEnvironment,
    failures: BTreeMap<ModId, HostError>,
}

impl ServiceContext<'_> {
    fn machine(&self) -> &LifecycleMachine {
        self.machine
    }

    fn failure(&self, service: &ServiceId) -> Option<HostError> {
        let owner = service.owner();
        if let Some(source) = self.failures.get(owner) {
            return Some(source.clone());
        }
        let prior = self.machine.failure(owner).ok().flatten()?;
        Some(HostError::PreviouslyFailed {
            mod_id: owner.clone(),
            attribution: Box::new(prior.clone()),
        })
    }

    fn fail(&mut self, source: HostError) -> Result<(), ServiceRefused> {
        self.isolate(source, |source| ServiceRefused::Mod {
            mod_id: source.mod_id().clone(),
            source,
        })
    }

    fn isolate(
        &mut self,
        source: HostError,
        refused: impl FnOnce(HostError) -> ServiceRefused,
    ) -> Result<(), ServiceRefused> {
        let failures = &mut self.failures;
        crate::lifecycle::isolate_recording(
            self.machine,
            self.environment,
            source,
            ServiceRefused::Lifecycle,
            |source| {
                failures.insert(source.mod_id().clone(), source.clone());
            },
            refused,
        )
    }

    fn refuse(&mut self, owner: &ModId, refused: ServiceRefused) -> Result<(), ServiceRefused> {
        let failures = &mut self.failures;
        crate::lifecycle::isolate_loader_refusal_recording(
            self.machine,
            self.environment,
            owner,
            refused,
            ServiceRefused::Lifecycle,
            |source| {
                failures.insert(source.mod_id().clone(), source.clone());
            },
        )
    }

    fn fail_if_provider_broken(
        &mut self,
        declaration: &ServiceDeclaration,
    ) -> Result<bool, ServiceRefused> {
        for service in declaration.requires() {
            let Some(source) = self.failure(service) else {
                continue;
            };
            let refused = ServiceRefused::BrokenProvider {
                consumer: declaration.owner().clone(),
                service: service.clone(),
                source: Box::new(source),
            };
            self.refuse(declaration.owner(), refused)?;
            return Ok(true);
        }
        Ok(false)
    }
}

fn open_with_context<H: std::ops::DerefMut<Target: Mod>>(
    context: &mut ServiceContext<'_>,
    mods: &mut [H],
) -> Result<ServiceOrder, ServiceRefused> {
    let declarations = collect_declarations(context, mods)?;
    let providers = providers(&declarations);
    let mut order = dependency_order(&declarations, &providers, context)?;

    crate::loaded::arrange(mods, &order);
    for position in 0..mods.len() {
        let (started, remaining) = mods.split_at_mut(position);
        let consumer = &mut remaining[0];
        let Some(declaration) = declarations.get(consumer.id()) else {
            continue;
        };
        let mod_id = consumer.id().clone();
        let state = context
            .machine()
            .state(&mod_id)
            .expect("a declared consumer is tracked");
        if state == LifecycleState::Failed || context.fail_if_provider_broken(declaration)? {
            continue;
        }
        if declaration.requires().next().is_none() && declaration.may_use().next().is_none() {
            continue;
        }

        // The state above also supplies callback attribution without another lookup.
        let mut directory = Started {
            caller: mod_id.clone(),
            declaration,
            providers: &providers,
            mods: started,
            context,
            failure: None,
            stopped: None,
        };
        let outcome = crate::callback(&mod_id, state, || consumer.use_services(&mut directory));
        if let Some(error) = directory.failure {
            return Err(error);
        }
        if let Err(source) = outcome {
            context.fail(source)?;
        }
    }

    order.retain(|owner| !context.machine().has_failed(owner));
    Ok(ServiceOrder { order })
}

/// Collects declarations from mods on this side, rejecting foreign owners.
///
/// # Errors
///
/// Returns [`ServiceRefused::ForeignDeclaration`] naming both identities.
fn collect_declarations<H: std::ops::DerefMut<Target: Mod>>(
    context: &mut ServiceContext<'_>,
    mods: &[H],
) -> Result<BTreeMap<ModId, ServiceDeclaration>, ServiceRefused> {
    let mut declarations = BTreeMap::new();
    let eligible: Vec<_> = context
        .machine()
        .eligible()
        .map(|(entry, state)| (entry.mod_id.clone(), state))
        .collect();
    for (mod_id, state) in eligible {
        let instance = crate::loaded::instance(mods, &mod_id);
        let declaration = match crate::callback(&mod_id, state, || instance.service_declaration()) {
            Ok(Some(declaration)) => declaration,
            Ok(None) => continue,
            Err(source) => {
                context.fail(source)?;
                continue;
            }
        };
        if declaration.owner() != &mod_id {
            let refused = ServiceRefused::ForeignDeclaration {
                mod_id: mod_id.clone(),
                owner: declaration.owner().clone(),
            };
            context.refuse(&mod_id, refused)?;
            continue;
        }
        declarations.insert(mod_id, declaration);
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

/// Orders live service edges, isolating consumers whose required providers are missing.
fn dependency_order(
    declarations: &BTreeMap<ModId, ServiceDeclaration>,
    providers: &BTreeMap<ServiceId, ModId>,
    context: &mut ServiceContext<'_>,
) -> Result<Vec<ModId>, ServiceRefused> {
    for (mod_id, declaration) in declarations {
        for service in declaration.requires() {
            if providers.contains_key(service) || context.failure(service).is_some() {
                continue;
            }
            context.refuse(
                mod_id,
                ServiceRefused::NoProvider {
                    consumer: mod_id.clone(),
                    service: service.clone(),
                },
            )?;
            break;
        }
    }

    let mut consumers: BTreeMap<ModId, BTreeSet<ModId>> = BTreeMap::new();
    for (mod_id, declaration) in declarations {
        if !context.machine().loads(mod_id) {
            continue;
        }
        for service in declaration.requires().chain(declaration.may_use()) {
            let Some(provider) = providers.get(service) else {
                continue;
            };
            if context.machine().loads(provider) {
                consumers
                    .entry(provider.clone())
                    .or_default()
                    .insert(mod_id.clone());
            }
        }
    }

    let live = declarations
        .keys()
        .filter(|owner| context.machine().loads(owner))
        .cloned();
    precedence(live, &consumers).map_err(|cycle| ServiceRefused::Cycle {
        participants: cycle.participants().to_vec(),
    })
}

/// A directory over the mods already started.
struct Started<'a, 'm, H: std::ops::DerefMut<Target: Mod>> {
    caller: ModId,
    declaration: &'a ServiceDeclaration,
    providers: &'a BTreeMap<ServiceId, ModId>,
    mods: &'a mut [H],
    context: &'a mut ServiceContext<'m>,
    failure: Option<ServiceRefused>,
    stopped: Option<HostError>,
}

impl<H: std::ops::DerefMut<Target: Mod>> ServiceDirectory for Started<'_, '_, H> {
    fn call(&mut self, request: &ServiceRequest) -> Result<ServiceResponse, ServiceCallFailed> {
        if !self.declaration.calls(&request.id) {
            return Err(ServiceCallFailed::Unavailable(
                ServiceUnavailable::Undeclared {
                    caller: self.caller.clone(),
                    service: request.id.clone(),
                },
            ));
        }
        if let Some(source) = &self.stopped {
            return Err(ServiceCallFailed::Refused(source.clone()));
        }
        // Failed namespaces remain opaque: their declaration cannot be queried safely,
        // so a retained failure is not evidence that the requested service is absent.
        if let Some(source) = self.context.failure(&request.id) {
            return Err(ServiceCallFailed::Refused(source));
        }
        let Some(mod_id) = self.providers.get(&request.id) else {
            return Err(unavailable(self.declaration, request));
        };
        let state = self
            .context
            .machine()
            .state(mod_id)
            .expect("a declared provider is tracked");
        let provider = crate::loaded::instance_mut(self.mods, mod_id);
        match crate::callback(mod_id, state, || provider.answer_service(request)) {
            Ok(response) => Ok(response),
            Err(source) => {
                if let Err(error) = self.context.fail(source.clone()) {
                    self.failure = Some(error);
                    self.stopped = Some(source.clone());
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
    /// A required failure or halted set blocks service callbacks.
    Halted {
        /// The owner blocking the set.
        by: ModId,
    },
    /// Nothing running in this set answers a service a mod requires.
    NoProvider {
        /// The mod that requires it.
        consumer: ModId,
        /// The service nothing answers.
        service: ServiceId,
    },
    /// A required service belongs to a namespace whose mod failed before or during opening.
    BrokenProvider {
        /// The mod that cannot start without the service.
        consumer: ModId,
        /// The required service.
        service: ServiceId,
        /// The provider's original failure.
        source: Box<HostError>,
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
    Mod {
        /// The owner whose callback failed.
        mod_id: ModId,
        /// What it said.
        source: HostError,
    },
}

impl ServiceRefused {
    /// Returns the mod responsible for a refusal, not a foreign declared owner.
    #[must_use]
    pub fn mod_id(&self) -> Option<&ModId> {
        match self {
            Self::Halted { by } => Some(by),
            Self::NoProvider { consumer, .. } | Self::BrokenProvider { consumer, .. } => {
                Some(consumer)
            }
            Self::ForeignDeclaration { mod_id, .. } | Self::Mod { mod_id, .. } => Some(mod_id),
            Self::Lifecycle(error) => error.mod_id(),
            Self::Cycle { .. } => None,
        }
    }
}

impl fmt::Display for ServiceRefused {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Lifecycle(error) => error.fmt(formatter),
            Self::Halted { by } => write!(formatter, "services are blocked by `{by}`"),
            Self::NoProvider { consumer, service } => write!(
                formatter,
                "`{consumer}` requires `{service}` and nothing running answers it"
            ),
            Self::BrokenProvider {
                consumer,
                service,
                source,
            } => write!(
                formatter,
                "`{consumer}` requires `{service}`, but its provider failed: {source}"
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
            Self::Mod { mod_id, source } => {
                write!(formatter, "`{mod_id}` refused its services: {source}")
            }
        }
    }
}

impl std::error::Error for ServiceRefused {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Lifecycle(error) => Some(error),
            Self::Halted { .. }
            | Self::NoProvider { .. }
            | Self::Cycle { .. }
            | Self::ForeignDeclaration { .. } => None,
            Self::Mod { source, .. } => Some(source),
            Self::BrokenProvider { source, .. } => Some(source.as_ref()),
        }
    }
}

impl StructuredFailure for ServiceRefused {
    /// Callback classification comes from the attributed source.
    fn failure_kind(&self) -> Option<FailureKind> {
        match self {
            Self::Lifecycle(error) => error.failure_kind(),
            Self::NoProvider { .. } | Self::BrokenProvider { .. } => {
                Some(FailureKind::MissingRequiredDependency)
            }
            Self::Cycle { .. } => Some(FailureKind::DependencyCycle),
            Self::Mod { source, .. } => source.failure_kind(),
            Self::Halted { .. } | Self::ForeignDeclaration { .. } => None,
        }
    }

    fn lifecycle_state(&self) -> LifecycleState {
        match self {
            Self::Lifecycle(error) => error.lifecycle_state(),
            Self::Halted { .. } => LifecycleState::Failed,
            Self::NoProvider { .. }
            | Self::BrokenProvider { .. }
            | Self::Cycle { .. }
            | Self::ForeignDeclaration { .. } => LifecycleState::Resolved,
            Self::Mod { source, .. } => source.lifecycle_state(),
        }
    }

    /// Delegates callback text classification to the attributed source.
    fn quotes_untrusted_text(&self) -> bool {
        match self {
            Self::Mod { source, .. } => source.quotes_untrusted_text(),
            Self::BrokenProvider { source, .. } => source.quotes_untrusted_text(),
            Self::Lifecycle(error) => error.quotes_untrusted_text(),
            Self::Halted { .. }
            | Self::NoProvider { .. }
            | Self::Cycle { .. }
            | Self::ForeignDeclaration { .. } => false,
        }
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{ContentId, LifecycleState, ModId, Side, StructuredFailure};
    use ravel_host_api::{
        HostError, Mod, ModError, ServiceDeclaration, ServiceDirectory, ServiceId, ServiceRequest,
        ServiceResponse,
    };
    use serde_json::json;

    use super::{ServiceRefused, open_services};
    use crate::lifecycle::{LifecycleEntry, LifecycleMachine, Requirement};
    use crate::loaded::LoadedMods;
    use crate::tests::entry;

    #[test]
    fn lifecycle_refusals_preserve_the_owner_including_halted_worlds() {
        let owner = id("provider");
        for error in [
            crate::LifecycleError::WorldAccessBlocked {
                mod_id: owner.clone(),
                state: LifecycleState::Registered,
            },
            crate::LifecycleError::WorldAccessHalted {
                mod_id: owner.clone(),
            },
            crate::LifecycleError::PartialStateRefused {
                mod_id: owner.clone(),
            },
        ] {
            assert_eq!(ServiceRefused::Lifecycle(error).mod_id(), Some(&owner));
        }
        assert_eq!(
            ServiceRefused::Lifecycle(crate::LifecycleError::AttributionRequired).mod_id(),
            None
        );
    }

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

    impl Mod for Failing {
        fn id(&self) -> &ModId {
            &self.id
        }

        fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
            Ok(())
        }

        fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
            assert_ne!(self.phase, "declaration", "declaration panic");
            Ok(Some({
                let mut declaration = ServiceDeclaration::new(self.id.clone());

                declaration
                    .require(service("provider", "answer"))
                    .expect("valid declaration");
                declaration
            }))
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
                &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host, &mut provider])
                    .expect("all active hosts are supplied"),
                &environment(),
            )
            .expect_err("callback failure stays contained");
            let ServiceRefused::Mod { mod_id, source } = refusal else {
                panic!("expected a consumer refusal");
            };
            assert_eq!(mod_id, id("actual"));
            assert_eq!(source.mod_id(), &id("actual"));
        }
    }

    #[test]
    fn provider_panics_are_contained_and_stamped_with_the_provider() {
        let mut provider = Grudging::new("provider", &[service("provider", "answer")]);
        provider.panics = true;
        let declaration = {
            let mut declaration = ServiceDeclaration::new(id("consumer"));

            declaration
                .require(service("provider", "answer"))
                .expect("valid declaration");
            declaration
        };
        let mut providers: [&mut dyn Mod; 1] = [&mut provider];
        let mut machine = machine(vec![entry("provider", &[Side::DedicatedServer])]);
        let environment = environment();
        let mut context = super::ServiceContext {
            machine: &mut machine,
            environment: &environment,
            failures: std::collections::BTreeMap::new(),
        };
        let declared_providers =
            std::collections::BTreeMap::from([(service("provider", "answer"), id("provider"))]);
        let mut directory = super::Started {
            caller: id("consumer"),
            declaration: &declaration,
            providers: &declared_providers,
            mods: &mut providers,
            context: &mut context,
            failure: None,
            stopped: None,
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
        let repeated = directory
            .call(&ServiceRequest::new(
                service("provider", "answer"),
                json!(null),
            ))
            .expect_err("a failed provider remains broken on the second call");
        assert!(matches!(
            &repeated,
            ravel_host_api::ServiceCallFailed::Refused(_)
        ));
        assert_eq!(repeated.to_string(), error.to_string());
        let Some(ServiceRefused::Mod { source, .. }) = directory.failure else {
            panic!("expected provider attribution");
        };
        assert_eq!(source.mod_id(), &id("provider"));
        assert_eq!(provider.calls, 1, "a failed provider is never called again");
    }

    #[test]
    fn stopped_directories_retain_the_refusal_for_every_declared_service() {
        for optional in [false, true] {
            let broken = service("broken", "answer");
            let present = service("healthy", "answer");
            let absent = service("absent", "answer");
            let previously_failed = service("earlier", "answer");
            let undeclared = service("healthy", "undeclared");
            let mut provider = Grudging::new("broken", std::slice::from_ref(&broken));
            let mut healthy = Wired::new("healthy", std::slice::from_ref(&present), &[]);
            let declaration = stopped_directory_declaration(
                optional,
                [broken.clone(), present.clone()],
                [absent.clone(), previously_failed.clone()],
            );
            let mut hosts: [&mut dyn Mod; 2] = [&mut provider, &mut healthy];
            let mut earlier = entry("earlier", &[Side::DedicatedServer]);
            earlier.requirement = Requirement::Optional;
            let mut machine = machine(vec![
                entry("broken", &[Side::DedicatedServer]),
                entry("healthy", &[Side::DedicatedServer]),
                earlier,
            ]);
            let environment = environment();
            let mut context = super::ServiceContext {
                machine: &mut machine,
                environment: &environment,
                failures: std::collections::BTreeMap::new(),
            };
            isolate_earlier_service_failure(&mut context);
            let providers = std::collections::BTreeMap::from([
                (broken.clone(), id("broken")),
                (present.clone(), id("healthy")),
            ]);
            let mut directory = super::Started {
                caller: id("consumer"),
                declaration: &declaration,
                providers: &providers,
                mods: &mut hosts,
                context: &mut context,
                failure: None,
                stopped: None,
            };
            let original = directory
                .call(&ServiceRequest::new(broken, json!(null)))
                .expect_err("required provider failure stops the directory");
            for wanted in [present, absent, previously_failed] {
                let subsequent = directory
                    .call(&ServiceRequest::new(wanted, json!(null)))
                    .expect_err("a stopped directory retains its original refusal");
                assert!(!subsequent.is_absent());
                assert_eq!(subsequent, original);
            }
            assert_eq!(
                directory.call(&ServiceRequest::new(undeclared.clone(), json!(null))),
                Err(ravel_host_api::ServiceCallFailed::Unavailable(
                    ravel_host_api::ServiceUnavailable::Undeclared {
                        caller: id("consumer"),
                        service: undeclared,
                    }
                ))
            );
            assert!(directory.failure.is_some());
            assert_eq!(healthy.answered, Vec::<ServiceId>::new());
        }
    }

    fn stopped_directory_declaration(
        optional: bool,
        present: [ServiceId; 2],
        unavailable: [ServiceId; 2],
    ) -> ServiceDeclaration {
        let mut declaration = ServiceDeclaration::new(id("consumer"));
        for wanted in present {
            if optional {
                declaration
                    .use_if_present(wanted)
                    .expect("legal declaration");
            } else {
                declaration.require(wanted).expect("legal declaration");
            }
        }
        for wanted in unavailable {
            declaration
                .use_if_present(wanted)
                .expect("legal declaration");
        }
        declaration
    }

    fn isolate_earlier_service_failure(context: &mut super::ServiceContext<'_>) {
        context
            .fail(HostError::Unsupported {
                mod_id: id("earlier"),
                state: LifecycleState::Discovered,
            })
            .expect("an earlier optional failure is isolated");
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
            &mut LoadedMods::<&mut dyn Mod>::new(
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

    #[test]
    fn an_undeclared_service_never_reaches_its_namespace_owner() {
        let missing = service("alpha", "missing");
        let mut namespace = Wired::new("alpha", &[service("alpha", "declared")], &[]);
        let mut consumer = Hedging::new("zeta", &[], std::slice::from_ref(&missing));
        let mut machine = machine(vec![
            entry("alpha", &[Side::DedicatedServer]),
            entry("zeta", &[Side::DedicatedServer]),
        ]);
        let state = machine.state(&id("alpha")).expect("tracked namespace");
        let mut mods: [&mut dyn Mod; 2] = [&mut namespace, &mut consumer];
        open_services(
            &mut LoadedMods::new(&mut machine, &mut mods).expect("complete set"),
            &environment(),
        )
        .expect("undeclared optional service is absent");
        assert_eq!(consumer.absent, [missing]);
        let empty: &[ServiceId] = &[];
        assert_eq!(consumer.refused.as_slice(), empty);
        assert_eq!(namespace.answered.as_slice(), empty);
        assert_eq!(machine.state(&id("alpha")), Ok(state));
        assert_eq!(machine.failure(&id("alpha")), Ok(None));
    }

    #[test]
    fn a_failed_optional_provider_stays_broken_for_the_next_consumer() {
        let wanted = service("provider", "answer");
        let mut provider = Grudging::new("provider", std::slice::from_ref(&wanted));
        let mut first = Hedging::new("alpha", &[], std::slice::from_ref(&wanted));
        let mut second = Hedging::new("zeta", &[], std::slice::from_ref(&wanted));
        let mut provider_entry = entry("provider", &[Side::DedicatedServer]);
        provider_entry.requirement = Requirement::Optional;
        let mut machine = machine(vec![
            entry("alpha", &[Side::DedicatedServer]),
            provider_entry,
            entry("zeta", &[Side::DedicatedServer]),
        ]);
        let mut mods: [&mut dyn Mod; 3] = [&mut first, &mut second, &mut provider];
        open_services(
            &mut LoadedMods::new(&mut machine, &mut mods).expect("complete set"),
            &environment(),
        )
        .expect("optional refusal does not stop startup");
        assert_eq!(first.refused, std::slice::from_ref(&wanted));
        assert_eq!(second.refused, [wanted]);
        let empty: &[ServiceId] = &[];
        assert_eq!(first.absent.as_slice(), empty);
        assert_eq!(second.absent.as_slice(), empty);
        assert_eq!(machine.state(&id("provider")), Ok(LifecycleState::Failed));
        assert_eq!(
            provider.calls, 1,
            "later consumers cannot re-enter failed code"
        );
    }

    #[test]
    fn optional_service_failure_after_world_mutation_still_refuses_startup() {
        let wanted = service("provider", "answer");
        let mut provider = Grudging::new("provider", std::slice::from_ref(&wanted));
        let mut consumer = Hedging::new("consumer", &[], &[wanted]);
        let mut provider_entry = entry("provider", &[Side::DedicatedServer]);
        provider_entry.requirement = Requirement::Optional;
        let mut machine = machine(vec![
            provider_entry,
            entry("consumer", &[Side::DedicatedServer]),
        ]);
        crate::tests::advance_through(&mut machine, LifecycleState::Running);
        machine.record_world_mutation().expect("running world");
        let mut mods: [&mut dyn Mod; 2] = [&mut provider, &mut consumer];
        let refused = open_services(
            &mut LoadedMods::new(&mut machine, &mut mods).expect("complete set"),
            &environment(),
        )
        .expect_err("optional failure cannot leave a mutated world running");
        assert!(matches!(refused, ServiceRefused::Mod { mod_id, .. } if mod_id == id("provider")));
        assert_eq!(provider.calls, 1);
        assert_eq!(machine.halted_by(), Some(&id("provider")));
        assert!(machine.lifecycle_world_access().is_err());
    }

    #[test]
    fn missing_required_services_halt_optional_consumers_after_world_mutation() {
        let wanted = service("absent", "answer");
        let mut consumer = Wired::new("consumer", &[], std::slice::from_ref(&wanted));
        let mut consumer_entry = entry("consumer", &[Side::DedicatedServer]);
        consumer_entry.requirement = Requirement::Optional;
        let mut machine = machine(vec![consumer_entry]);
        crate::tests::advance_through(&mut machine, LifecycleState::Running);
        machine.record_world_mutation().expect("running world");
        let refused = open_services(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut consumer])
                .expect("complete set"),
            &environment(),
        )
        .expect_err("a missing service cannot leave a mutated world running");
        assert_eq!(
            refused,
            ServiceRefused::NoProvider {
                consumer: id("consumer"),
                service: wanted,
            }
        );
        assert_eq!(machine.halted_by(), Some(&id("consumer")));
        assert!(machine.lifecycle_world_access().is_err());
        assert_eq!(consumer.reached, Vec::<String>::new());
    }

    #[test]
    fn a_provider_failed_before_service_open_is_refused_not_absent() {
        let wanted = service("provider", "answer");
        let mut provider = Failing {
            id: id("provider"),
            phase: "declaration",
        };
        let mut consumer = Hedging::new("consumer", &[], std::slice::from_ref(&wanted));
        let mut provider_entry = entry("provider", &[Side::DedicatedServer]);
        provider_entry.requirement = Requirement::Optional;
        let mut machine = machine(vec![
            provider_entry,
            entry("consumer", &[Side::DedicatedServer]),
        ]);
        let source = HostError::OperationRefused {
            mod_id: id("provider"),
            state: LifecycleState::Discovered,
            detail: "earlier callback refusal".to_owned(),
        };
        machine
            .fail_callback(&source, &environment(), None)
            .expect("record an earlier optional failure");
        let prior = machine
            .failure(&id("provider"))
            .expect("tracked provider")
            .cloned();
        let mut mods: [&mut dyn Mod; 2] = [&mut provider, &mut consumer];
        open_services(
            &mut LoadedMods::new(&mut machine, &mut mods).expect("complete set"),
            &environment(),
        )
        .expect("optional provider remains opaque");
        assert_eq!(consumer.refused, [wanted]);
        let empty: &[ServiceId] = &[];
        assert_eq!(consumer.absent.as_slice(), empty);
        assert_eq!(
            machine.failure(&id("provider")).expect("tracked provider"),
            prior.as_ref()
        );
        let prior = prior.expect("recorded attribution");
        assert_eq!(
            consumer.failures,
            [HostError::PreviouslyFailed {
                mod_id: id("provider"),
                attribution: Box::new(prior.clone()),
            }]
        );
        assert_eq!(consumer.failures[0].failure_kind(), prior.kind);
        assert_eq!(consumer.failures[0].lifecycle_state(), prior.state);
        assert_eq!(consumer.failures[0].to_string(), prior.message.text());
    }

    #[test]
    fn a_required_service_in_a_failed_namespace_retains_the_prior_refusal() {
        let wanted = service("provider", "answer");
        let mut provider = Failing {
            id: id("provider"),
            phase: "declaration",
        };
        let mut consumer = Wired::new("consumer", &[], std::slice::from_ref(&wanted));
        let mut provider_entry = entry("provider", &[Side::DedicatedServer]);
        provider_entry.requirement = Requirement::Optional;
        let mut machine = machine(vec![
            provider_entry,
            entry("consumer", &[Side::DedicatedServer]),
        ]);
        machine
            .fail(
                &id("provider"),
                ravel_core::FailureAttribution {
                    mod_id: Some(id("provider")),
                    version: Some("2.3.4".parse().expect("valid version")),
                    dependency_chain: vec![id("earlier_consumer"), id("provider")],
                    state: LifecycleState::Discovered,
                    kind: Some(ravel_core::FailureKind::DependencyCycle),
                    surface: None,
                    environment: environment(),
                    message: ravel_core::DiagnosticMessage::raw("earlier dependency refusal"),
                },
            )
            .expect("record an earlier optional failure");
        let mut mods: [&mut dyn Mod; 2] = [&mut provider, &mut consumer];
        let refused = open_services(
            &mut LoadedMods::new(&mut machine, &mut mods).expect("complete set"),
            &environment(),
        )
        .expect_err("a required service cannot be supplied by a failed namespace");
        let ServiceRefused::BrokenProvider {
            consumer: mod_id,
            service,
            source,
        } = &refused
        else {
            panic!("a failed namespace retains its consumer and service");
        };
        assert_eq!(mod_id, &id("consumer"));
        assert_eq!(service, &wanted);
        let prior = machine
            .failure(&id("provider"))
            .expect("tracked provider")
            .expect("prior failure retained");
        assert_eq!(
            source.as_ref(),
            &HostError::PreviouslyFailed {
                mod_id: id("provider"),
                attribution: Box::new(prior.clone()),
            }
        );
        assert_eq!(source.lifecycle_state(), prior.state);
        assert_eq!(source.failure_kind(), prior.kind);
        assert_eq!(source.to_string(), prior.message.text());
        assert_eq!(
            refused.failure_kind(),
            Some(ravel_core::FailureKind::MissingRequiredDependency)
        );
        let empty: &[String] = &[];
        assert_eq!(consumer.reached.as_slice(), empty);
    }

    #[test]
    fn a_default_nonprovider_is_not_called_or_failed_for_its_namespace() {
        struct Plain(ModId);
        impl Mod for Plain {
            fn id(&self) -> &ModId {
                &self.0
            }
            fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
                Ok(())
            }
        }
        let wanted = service("plain", "missing");
        let mut plain = Plain(id("plain"));
        let declaration = {
            let mut declaration = ServiceDeclaration::new(id("consumer"));

            declaration
                .use_if_present(wanted.clone())
                .expect("valid optional declaration");
            declaration
        };
        let mut machine = machine(vec![entry("plain", &[Side::DedicatedServer])]);
        let state = machine.state(&id("plain")).expect("tracked namespace");
        let environment = environment();
        let mut context = super::ServiceContext {
            machine: &mut machine,
            environment: &environment,
            failures: std::collections::BTreeMap::new(),
        };
        let declared_providers = std::collections::BTreeMap::new();
        let mut mods: [&mut dyn Mod; 1] = [&mut plain];
        let mut directory = super::Started {
            caller: id("consumer"),
            declaration: &declaration,
            providers: &declared_providers,
            mods: &mut mods,
            context: &mut context,
            failure: None,
            stopped: None,
        };
        let refusal = directory
            .call(&ServiceRequest::new(wanted, json!(null)))
            .expect_err("undeclared service is unavailable");
        assert!(refusal.is_absent());
        assert!(directory.failure.is_none());
        assert_eq!(context.machine.state(&id("plain")), Ok(state));
        assert_eq!(context.machine.failure(&id("plain")), Ok(None));
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
                declaration: {
                    let mut declaration = ServiceDeclaration::new(id(name));
                    for service in provides.iter().cloned() {
                        declaration.provide(service).expect("a legal declaration");
                    }
                    for service in requires.iter().cloned() {
                        declaration.require(service).expect("a legal declaration");
                    }
                    declaration
                },
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
            self.declaration = {
                let mut declaration = ServiceDeclaration::new(id(owner));
                for service in provides.iter().cloned() {
                    declaration
                        .provide(service)
                        .expect("a legal declaration for the other identity");
                }

                declaration
            };
            self
        }
    }

    impl Mod for Wired {
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
    fn service_isolation_retains_errors_for_both_stop_results() {
        for requirement in [Requirement::Required, Requirement::Optional] {
            let mut owner = entry("provider", &[Side::DedicatedServer]);
            owner.requirement = requirement;
            let mut machine = machine(vec![owner]);
            let environment = environment();
            let mut context = super::ServiceContext {
                machine: &mut machine,
                environment: &environment,
                failures: std::collections::BTreeMap::new(),
            };
            let source = HostError::Unsupported {
                mod_id: id("provider"),
                state: LifecycleState::Discovered,
            };
            let result = context.fail(source.clone());
            let expected = if requirement == Requirement::Required {
                Err(ServiceRefused::Mod {
                    mod_id: id("provider"),
                    source: source.clone(),
                })
            } else {
                Ok(())
            };
            assert_eq!(result, expected);
            assert_eq!(context.failures.get(&id("provider")), Some(&source));
            assert_eq!(
                context.machine.state(&id("provider")),
                Ok(LifecycleState::Failed)
            );
        }
    }

    #[test]
    fn service_isolation_does_not_retain_unattributable_errors() {
        let mut machine = machine(Vec::new());
        let environment = environment();
        let mut context = super::ServiceContext {
            machine: &mut machine,
            environment: &environment,
            failures: std::collections::BTreeMap::new(),
        };
        let owner = id("outside_plan");
        assert_eq!(
            context.fail(HostError::Unsupported {
                mod_id: owner.clone(),
                state: LifecycleState::Discovered,
            }),
            Err(ServiceRefused::Lifecycle(
                crate::lifecycle::LifecycleError::UnknownMod { mod_id: owner }
            ))
        );
        assert!(context.failures.is_empty());
    }

    #[test]
    fn service_declaration_refusal_is_not_absence() {
        let mut host = Wired::new("refusing_mod", &[], &[]);
        host.refuses_declaration = true;
        let mut machine = machine(vec![entry("refusing_mod", &[Side::DedicatedServer])]);
        assert_eq!(
            open_services(
                &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut host])
                    .expect("all active hosts are supplied"),
                &environment(),
            ),
            Err(ServiceRefused::Mod {
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
        let mut mods: [&mut dyn Mod; 2] = [&mut consumer, &mut warehouse];
        let opened = open_services(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect("the graph resolves");

        assert_eq!(opened.order(), [id("warehouse"), id("consumer")]);
        assert_eq!(
            mods.iter()
                .map(|host| host.id().clone())
                .collect::<Vec<_>>(),
            [id("consumer"), id("warehouse")]
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
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut thief, &mut consumer])
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

        let mut mods: [&mut dyn Mod; 3] = [&mut anvil, &mut mill, &mut warehouse];
        let opened = open_services(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect("the chain resolves");

        assert_eq!(opened.order(), [id("warehouse"), id("mill"), id("anvil")]);
        assert_eq!(
            mods.iter()
                .map(|host| host.id().clone())
                .collect::<Vec<_>>(),
            [id("anvil"), id("mill"), id("warehouse")]
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
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut first, &mut second])
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
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut consumer])
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
        assert_eq!(machine.state(&id("consumer")), Ok(LifecycleState::Failed));
        assert!(
            machine
                .failure(&id("consumer"))
                .expect("tracked consumer")
                .is_some()
        );
    }

    #[test]
    fn optional_consumers_with_missing_or_parked_providers_are_isolated() {
        for parked in [false, true] {
            let wanted = service("warehouse", "prices");
            let mut consumer = Wired::new("consumer", &[], std::slice::from_ref(&wanted));
            let mut warehouse = Wired::new("warehouse", &[wanted], &[]);
            let mut healthy = Wired::new("healthy", &[], &[]);
            let mut consumer_entry = entry("consumer", &[Side::DedicatedServer]);
            consumer_entry.requirement = Requirement::Optional;
            let mut entries = vec![consumer_entry, entry("healthy", &[Side::DedicatedServer])];
            if parked {
                entries.push(entry("warehouse", &[Side::Client]));
            }
            let mut machine = machine(entries);
            let mut hosts: Vec<&mut dyn Mod> = vec![&mut consumer, &mut healthy];
            if parked {
                hosts.push(&mut warehouse);
            }
            let opened = open_services(
                &mut LoadedMods::new(&mut machine, &mut hosts)
                    .expect("all active hosts are supplied"),
                &environment(),
            )
            .expect("the optional consumer does not refuse the set");
            assert_eq!(opened.order(), [id("healthy")]);
            assert_eq!(machine.state(&id("consumer")), Ok(LifecycleState::Failed));
            let failure = machine
                .failure(&id("consumer"))
                .expect("tracked consumer")
                .expect("missing service is attributed");
            assert!(failure.message.text().contains("warehouse:prices"));
            assert_eq!(consumer.reached, Vec::<String>::new());
            assert_eq!(warehouse.answered, Vec::<ServiceId>::new());
        }
    }

    #[test]
    fn missing_services_remove_optional_consumers_from_the_dependency_graph() {
        let mut first = Wired::new(
            "first",
            &[service("first", "answer")],
            &[service("second", "answer"), service("absent", "answer")],
        );
        let mut second = Hedging::new(
            "second",
            &[service("second", "answer")],
            &[service("first", "answer")],
        );
        let mut first_entry = entry("first", &[Side::DedicatedServer]);
        first_entry.requirement = Requirement::Optional;
        let mut machine = machine(vec![first_entry, entry("second", &[Side::DedicatedServer])]);
        let opened = open_services(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut first, &mut second])
                .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect("a failed optional consumer cannot create a service cycle");
        assert_eq!(opened.order(), [id("second")]);
        assert_eq!(first.reached, Vec::<String>::new());
        assert_eq!(second.refused, [service("first", "answer")]);
        assert_eq!(second.absent, Vec::<ServiceId>::new());
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
                &mut LoadedMods::<&mut dyn Mod>::new(
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
        let mut mods: [&mut dyn Mod; 3] = [&mut warehouse, &mut mill, &mut anvil];

        let refusal = open_services(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect_err("the mod in the mill of the chain refuses");
        assert_eq!(
            mods.iter()
                .map(|host| host.id().clone())
                .collect::<Vec<_>>(),
            [id("anvil"), id("mill"), id("warehouse")],
            "service refusal must restore resolved lifecycle order"
        );
        assert!(matches!(
            refusal,
            ServiceRefused::Mod { ref mod_id, .. } if mod_id == &id("mill")
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

        impl Mod for Sneaking {
            fn id(&self) -> &ModId {
                &self.id
            }

            fn enter(&mut self, _state: LifecycleState) -> Result<(), ModError> {
                Ok(())
            }

            fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
                Ok(Some({
                    let mut declaration = ServiceDeclaration::new(self.id.clone());

                    declaration
                        .require(service("warehouse", "prices"))
                        .expect("valid service declaration");
                    declaration
                }))
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
            &mut LoadedMods::<&mut dyn Mod>::new(
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

        impl Mod for Plain {
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
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut plain])
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
        failures: Vec<HostError>,
    }

    impl Hedging {
        fn new(name: &str, provides: &[ServiceId], may_use: &[ServiceId]) -> Self {
            Self {
                id: id(name),
                declaration: {
                    let mut declaration = ServiceDeclaration::new(id(name));
                    for service in provides.iter().cloned() {
                        declaration.provide(service).expect("a legal declaration");
                    }

                    for service in may_use.iter().cloned() {
                        declaration
                            .use_if_present(service)
                            .expect("a legal declaration");
                    }
                    declaration
                },
                reached: Vec::new(),
                absent: Vec::new(),
                refused: Vec::new(),
                failures: Vec::new(),
            }
        }
    }

    impl Mod for Hedging {
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
                    Err(ravel_host_api::ServiceCallFailed::Refused(source)) => {
                        self.refused.push(wanted);
                        self.failures.push(source);
                    }
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
        calls: usize,
    }

    impl Grudging {
        fn new(name: &str, provides: &[ServiceId]) -> Self {
            Self {
                id: id(name),
                declaration: {
                    let mut declaration = ServiceDeclaration::new(id(name));
                    for service in provides.iter().cloned() {
                        declaration.provide(service).expect("a legal declaration");
                    }

                    declaration
                },
                panics: false,
                calls: 0,
            }
        }
    }

    impl Mod for Grudging {
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
            self.calls += 1;
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
        let mut mods: [&mut dyn Mod; 2] = [&mut consumer, &mut warehouse];
        let opened = open_services(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut mods)
                .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect("the graph resolves");

        assert_eq!(opened.order(), [id("warehouse"), id("consumer")]);
        assert_eq!(
            mods.iter()
                .map(|host| host.id().clone())
                .collect::<Vec<_>>(),
            [id("consumer"), id("warehouse")]
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
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut consumer])
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
        consumer.declaration = {
            let mut declaration = ServiceDeclaration::new(id("consumer"));

            declaration
                .require(service("gone", "clock"))
                .expect("a legal declaration");
            declaration
                .use_if_present(service("absent", "prices"))
                .expect("a legal declaration");
            declaration
        };
        let mut machine = machine(vec![entry("consumer", &[Side::DedicatedServer])]);

        assert_eq!(
            open_services(
                &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut consumer])
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
            &mut LoadedMods::<&mut dyn Mod>::new(
                &mut machine,
                &mut [&mut consumer, &mut warehouse],
            )
            .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect_err("the provider said no");

        assert!(matches!(
            refusal,
            ServiceRefused::Mod { ref mod_id, .. } if mod_id == &id("warehouse")
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
    fn a_required_consumer_never_opens_after_its_optional_provider_breaks() {
        for requirement in [Requirement::Required, Requirement::Optional] {
            let wanted = service("warehouse", "prices");
            let mut warehouse = Grudging::new("warehouse", std::slice::from_ref(&wanted));
            let mut earlier = Hedging::new("earlier", &[], std::slice::from_ref(&wanted));
            let mut later = Wired::new("later", &[], std::slice::from_ref(&wanted));
            let mut provider = entry("warehouse", &[Side::DedicatedServer]);
            provider.requirement = Requirement::Optional;
            let mut consumer = entry("later", &[Side::DedicatedServer]);
            consumer.requirement = requirement;
            let mut machine = machine(vec![
                provider,
                entry("earlier", &[Side::DedicatedServer]),
                consumer,
            ]);
            let result = open_services(
                &mut LoadedMods::<&mut dyn Mod>::new(
                    &mut machine,
                    &mut [&mut warehouse, &mut earlier, &mut later],
                )
                .expect("all active hosts are supplied"),
                &environment(),
            );
            if requirement == Requirement::Required {
                assert!(matches!(result, Err(ServiceRefused::BrokenProvider {
                    consumer, service, ..
                }) if consumer == id("later") && service == wanted));
            } else {
                assert_eq!(
                    result.expect("optional consumer is isolated").order(),
                    [id("earlier")]
                );
            }
            assert_eq!(machine.state(&id("warehouse")), Ok(LifecycleState::Failed));
            assert_eq!(machine.state(&id("later")), Ok(LifecycleState::Failed));
            assert_eq!(later.reached, Vec::<String>::new());
            assert_eq!(warehouse.calls, 1);
        }
    }

    #[test]
    fn optional_required_consumers_are_isolated_after_provider_declaration_failures() {
        for foreign in [false, true] {
            let mut provider: Box<dyn Mod> = if foreign {
                Box::new(Wired::new("provider", &[], &[]).answering_for("other", &[]))
            } else {
                Box::new(Failing {
                    id: id("provider"),
                    phase: "declaration",
                })
            };
            let mut consumer = Wired::new("consumer", &[], &[service("provider", "answer")]);
            let mut other = Wired::new("other", &[], &[]);
            let mut provider_entry = entry("provider", &[Side::DedicatedServer]);
            provider_entry.requirement = Requirement::Optional;
            let mut consumer_entry = entry("consumer", &[Side::DedicatedServer]);
            consumer_entry.requirement = Requirement::Optional;
            let mut machine = machine(vec![
                provider_entry,
                consumer_entry,
                entry("other", &[Side::DedicatedServer]),
            ]);
            let opened = open_services(
                &mut LoadedMods::<&mut dyn Mod>::new(
                    &mut machine,
                    &mut [&mut *provider, &mut consumer, &mut other],
                )
                .expect("all active hosts are supplied"),
                &environment(),
            )
            .expect("optional provider and consumer failures are isolated");
            assert_eq!(opened.order(), [id("other")]);
            assert_eq!(machine.state(&id("provider")), Ok(LifecycleState::Failed));
            assert_eq!(machine.state(&id("consumer")), Ok(LifecycleState::Failed));
            assert!(
                machine
                    .failure(&id("consumer"))
                    .expect("tracked consumer")
                    .is_some()
            );
            assert_eq!(consumer.reached, Vec::<String>::new());
        }
    }

    #[test]
    fn an_optional_foreign_declaration_fails_its_actual_owner_and_keeps_others() {
        let mut thief = Wired::new("thief", &[], &[]).answering_for("other", &[]);
        let mut other = Wired::new("other", &[], &[]);
        let mut thief_entry = entry("thief", &[Side::DedicatedServer]);
        thief_entry.requirement = Requirement::Optional;
        let mut machine = machine(vec![thief_entry, entry("other", &[Side::DedicatedServer])]);
        let opened = open_services(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut thief, &mut other])
                .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect("an optional thief is isolated");
        assert_eq!(opened.order(), [id("other")]);
        assert_eq!(machine.state(&id("thief")), Ok(LifecycleState::Failed));
        let failure = machine
            .failure(&id("thief"))
            .expect("tracked thief")
            .expect("attributed declaration refusal");
        assert_eq!(failure.mod_id, Some(id("thief")));
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
        let mut machine = machine(vec![
            entry("consumer", &[Side::DedicatedServer]),
            provider_entry,
        ]);
        let mut mods: Vec<Box<dyn Mod>> = vec![Box::new(consumer), Box::new(provider)];
        let result = crate::start_all(
            &mut LoadedMods::new(&mut machine, &mut mods).expect("complete set"),
            &environment,
        );
        if requirement == Requirement::Required {
            assert!(
                matches!(result, Err(crate::DriveError::Services(ServiceRefused::Mod { mod_id, source }))
                if mod_id == id("warehouse")
                    && source.state() == LifecycleState::Registered)
            );
        } else {
            result.expect("the optional provider may fail");
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
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut first, &mut second])
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
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine, &mut [&mut first])
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
            &mut LoadedMods::<&mut dyn Mod>::new(
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
        let mut mods: [&mut dyn Mod; 3] = [&mut warehouse, &mut consumer, &mut other];
        let forward = open_services(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine(entries.clone()), &mut mods)
                .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect("resolves");
        assert_eq!(
            mods.iter()
                .map(|host| host.id().clone())
                .collect::<Vec<_>>(),
            [id("warehouse"), id("consumer"), id("other")]
        );

        let (mut warehouse, mut consumer, mut other) = build();
        let mut mods: [&mut dyn Mod; 3] = [&mut other, &mut consumer, &mut warehouse];
        let reversed = open_services(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine(entries), &mut mods)
                .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect("resolves");
        assert_eq!(
            mods.iter()
                .map(|host| host.id().clone())
                .collect::<Vec<_>>(),
            [id("warehouse"), id("consumer"), id("other")]
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
            &mut LoadedMods::<&mut dyn Mod>::new(
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
        let mut mods: [&mut dyn Mod; 3] = [&mut warehouse, &mut consumer, &mut other];
        let forward = open_services(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine(entries.clone()), &mut mods)
                .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect("resolves");
        assert_eq!(
            mods.iter()
                .map(|host| host.id().clone())
                .collect::<Vec<_>>(),
            [id("warehouse"), id("consumer"), id("other")]
        );

        let (mut warehouse, mut consumer, mut other) = build();
        let mut mods: [&mut dyn Mod; 3] = [&mut other, &mut consumer, &mut warehouse];
        let reversed = open_services(
            &mut LoadedMods::<&mut dyn Mod>::new(&mut machine(entries), &mut mods)
                .expect("all active hosts are supplied"),
            &environment(),
        )
        .expect("resolves");
        assert_eq!(
            mods.iter()
                .map(|host| host.id().clone())
                .collect::<Vec<_>>(),
            [id("warehouse"), id("consumer"), id("other")]
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
