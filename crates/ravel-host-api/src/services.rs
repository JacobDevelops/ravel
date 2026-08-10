use std::collections::BTreeSet;
use std::fmt;

use ravel_core::{ContentId, ModId};
use serde_json::Value;

/// The name one mod's service is reached by.
///
/// Namespaced by its owner for the reason a registry key and a packet contract
/// are: exactly one mod may answer a given service, and a name whose first half
/// is the answering mod's identity cannot be claimed by anybody else. That makes
/// "who provides this" a question about the name rather than about a table.
#[derive(Clone, Debug, Eq, Hash, Ord, PartialEq, PartialOrd)]
pub struct ServiceId(ContentId);

impl ServiceId {
    /// Names the service `id` identifies.
    #[must_use]
    pub const fn new(id: ContentId) -> Self {
        Self(id)
    }

    /// Returns the mod that must answer this service.
    #[must_use]
    pub const fn owner(&self) -> &ModId {
        self.0.namespace()
    }

    /// Returns the underlying identifier.
    #[must_use]
    pub const fn as_content_id(&self) -> &ContentId {
        &self.0
    }
}

impl fmt::Display for ServiceId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        self.0.fmt(formatter)
    }
}

/// What one mod answers, what it needs answered, and what it will use if present.
///
/// All three halves are declared rather than discovered, and that is what makes
/// the order decidable before any mod code runs: the loader can see the whole
/// graph, name a missing provider or a cycle, and refuse — instead of finding out
/// when a call arrives at a mod that is not there yet.
///
/// # Required and optional
///
/// [`requires`](Self::requires) and [`may_use`](Self::may_use) differ in exactly
/// one thing: what a missing provider means. A required service nothing answers
/// refuses the whole set, the way an unresolved package does. An optional one
/// nothing answers is an ordinary fact about this install — the set loads, and the
/// call reports [`ServiceUnavailable::Absent`] for the consumer to branch on.
///
/// They are the same in everything else. A declared optional service whose
/// provider *is* running is ordered before the consumer exactly as a required one
/// is, and calling it is legal rather than [`ServiceUnavailable::Undeclared`]. So
/// "is this mod installed" is answered once, by the loader, before any mod runs —
/// and never by a consumer that got ordered first and saw nothing.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceDeclaration {
    owner: ModId,
    provides: BTreeSet<ServiceId>,
    requires: BTreeSet<ServiceId>,
    may_use: BTreeSet<ServiceId>,
}

impl ServiceDeclaration {
    /// Declares what `owner` answers and what it needs.
    ///
    /// The three-part form of [`ServiceDeclaration::with_optional`], for a mod
    /// with nothing optional to say.
    ///
    /// # Errors
    ///
    /// Returns whatever [`ServiceDeclaration::with_optional`] would for the same
    /// arguments and an empty optional set.
    pub fn new(
        owner: ModId,
        provides: impl IntoIterator<Item = ServiceId>,
        requires: impl IntoIterator<Item = ServiceId>,
    ) -> Result<Self, ServiceDeclarationError> {
        Self::with_optional(owner, provides, requires, [])
    }

    /// Declares what `owner` answers, what it needs, and what it may use.
    ///
    /// A name in `may_use` is a service this mod calls when something answers it
    /// and does without when nothing does. Declaring it is what buys the ordering:
    /// a provider that *is* installed still runs first.
    ///
    /// # Errors
    ///
    /// Returns [`ServiceDeclarationError::NotOurs`] for a provided service whose
    /// name belongs to another mod, because a name is what says who answers and a
    /// mod answering under another's namespace would make two mods claimable
    /// under one identity.
    ///
    /// Returns [`ServiceDeclarationError::RequiresItself`] for a service the same
    /// declaration both provides and requires. It is not a cycle the loader has
    /// to find — it is one mod asking to be ordered before itself, which no order
    /// satisfies, and refusing it here names the mod rather than a cycle of one.
    ///
    /// Returns [`ServiceDeclarationError::MayUseItself`] for a service the same
    /// declaration both provides and may use, for that same reason: the provider
    /// is present by construction, so the optional edge is a real one and it is a
    /// self-edge.
    ///
    /// Returns [`ServiceDeclarationError::RequiredAndOptional`] for a service on
    /// both the required and the optional side. The two sides disagree about what
    /// a missing provider means, and a declaration that says both does not say
    /// which — picking one would be the loader deciding what the author meant.
    pub fn with_optional(
        owner: ModId,
        provides: impl IntoIterator<Item = ServiceId>,
        requires: impl IntoIterator<Item = ServiceId>,
        may_use: impl IntoIterator<Item = ServiceId>,
    ) -> Result<Self, ServiceDeclarationError> {
        let provides: BTreeSet<ServiceId> = provides.into_iter().collect();
        let requires: BTreeSet<ServiceId> = requires.into_iter().collect();
        let may_use: BTreeSet<ServiceId> = may_use.into_iter().collect();

        if let Some(foreign) = provides.iter().find(|id| id.owner() != &owner) {
            return Err(ServiceDeclarationError::NotOurs {
                declaring: owner,
                service: foreign.clone(),
            });
        }
        if let Some(own) = requires.intersection(&provides).next() {
            return Err(ServiceDeclarationError::RequiresItself {
                declaring: owner,
                service: own.clone(),
            });
        }
        if let Some(own) = may_use.intersection(&provides).next() {
            return Err(ServiceDeclarationError::MayUseItself {
                declaring: owner,
                service: own.clone(),
            });
        }
        if let Some(both) = may_use.intersection(&requires).next() {
            return Err(ServiceDeclarationError::RequiredAndOptional {
                declaring: owner,
                service: both.clone(),
            });
        }
        Ok(Self {
            owner,
            provides,
            requires,
            may_use,
        })
    }

    /// Returns the mod this declaration is for.
    #[must_use]
    pub const fn owner(&self) -> &ModId {
        &self.owner
    }

    /// Returns every service this mod answers, in name order.
    pub fn provides(&self) -> impl Iterator<Item = &ServiceId> + '_ {
        self.provides.iter()
    }

    /// Returns every service this mod needs answered, in name order.
    pub fn requires(&self) -> impl Iterator<Item = &ServiceId> + '_ {
        self.requires.iter()
    }

    /// Returns every service this mod uses when something answers it, in name
    /// order.
    pub fn may_use(&self) -> impl Iterator<Item = &ServiceId> + '_ {
        self.may_use.iter()
    }

    /// Returns whether this declaration names `service` on either calling side.
    ///
    /// What the directory checks a call against: a mod may call what it requires
    /// and what it may use, and nothing else.
    #[must_use]
    pub fn calls(&self, service: &ServiceId) -> bool {
        self.requires.contains(service) || self.may_use.contains(service)
    }

    /// Returns whether `service` is one this mod declared it may do without.
    #[must_use]
    pub fn may_do_without(&self, service: &ServiceId) -> bool {
        self.may_use.contains(service)
    }
}

/// A declaration that could not be built.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServiceDeclarationError {
    /// A provided service is named in another mod's namespace.
    NotOurs {
        /// The mod that declared it.
        declaring: ModId,
        /// The service whose name belongs to somebody else.
        service: ServiceId,
    },
    /// A mod both provides and requires one service.
    RequiresItself {
        /// The mod that declared both halves.
        declaring: ModId,
        /// The service on both sides.
        service: ServiceId,
    },
    /// A mod both provides and may use one service.
    MayUseItself {
        /// The mod that declared both halves.
        declaring: ModId,
        /// The service on both sides.
        service: ServiceId,
    },
    /// A mod declared one service as both required and optional.
    RequiredAndOptional {
        /// The mod that declared it twice.
        declaring: ModId,
        /// The service on both calling sides.
        service: ServiceId,
    },
}

impl fmt::Display for ServiceDeclarationError {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::NotOurs { declaring, service } => write!(
                formatter,
                "`{declaring}` offers to answer `{service}`, which is `{}`'s to answer",
                service.owner()
            ),
            Self::RequiresItself { declaring, service } => write!(
                formatter,
                "`{declaring}` both answers and requires `{service}`, so no order starts it"
            ),
            Self::MayUseItself { declaring, service } => write!(
                formatter,
                "`{declaring}` both answers and may use `{service}`, so no order starts it"
            ),
            Self::RequiredAndOptional { declaring, service } => write!(
                formatter,
                "`{declaring}` declares `{service}` as both required and optional, and those \
                 disagree about a missing provider"
            ),
        }
    }
}

impl std::error::Error for ServiceDeclarationError {}

/// One call to a service.
///
/// The arguments and the answer are open JSON rather than a Rust type both mods
/// name. A consumer that named the provider's type would be depending on the
/// provider's *build* rather than on the version range its manifest declares, and
/// ADG-2 distributes a mod as source the player's machine compiles, so the two
/// builds are not interchangeable. This is the same trade the packet contracts
/// make: a shape agreed in data, checked at the boundary, and versioned by the
/// provider.
#[derive(Clone, Debug, PartialEq)]
pub struct ServiceRequest {
    /// The service being called.
    pub id: ServiceId,
    /// What the caller is asking.
    pub arguments: Value,
}

impl ServiceRequest {
    /// Builds a call to `id` carrying `arguments`.
    #[must_use]
    pub const fn new(id: ServiceId, arguments: Value) -> Self {
        Self { id, arguments }
    }
}

/// What a service answered.
#[derive(Clone, Debug, PartialEq)]
pub struct ServiceResponse(Value);

impl ServiceResponse {
    /// Records an answer.
    #[must_use]
    pub const fn new(value: Value) -> Self {
        Self(value)
    }

    /// Returns the answer.
    #[must_use]
    pub const fn value(&self) -> &Value {
        &self.0
    }
}

/// Why a call reached nobody.
///
/// Distinct from the answering mod's own refusal: this says the call never got
/// as far as a mod, which is a wiring fact and not an answer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServiceUnavailable {
    /// The caller declared neither that it requires this service nor that it may
    /// use it.
    Undeclared {
        /// The mod that called.
        caller: ModId,
        /// The service it called.
        service: ServiceId,
    },
    /// Nothing in this set answers a service the caller declared it **requires**.
    ///
    /// A resolved set cannot reach this — the loader refuses a missing provider
    /// before any mod runs — so it exists for a directory built over a set that
    /// was never resolved, which is a host mistake worth naming rather than
    /// papering over.
    NoProvider {
        /// The service nobody answers.
        service: ServiceId,
    },
    /// Nothing in this set answers a service the caller declared it **may use**.
    ///
    /// Not a mistake by anyone. The caller said it works either way, and this is
    /// the answer to the question it was really asking: the provider is not
    /// installed, or is installed and parked on the other side. A consumer
    /// branches on this rather than refusing, which is the whole difference
    /// between an optional declaration and a required one.
    ///
    /// Distinct from [`ServiceCallFailed::Refused`], which means a provider *was*
    /// reached and said no. Those two are different events and this enum is how a
    /// mod tells them apart without reading a sentence.
    Absent {
        /// The service nothing in this set answers.
        service: ServiceId,
    },
}

impl fmt::Display for ServiceUnavailable {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Undeclared { caller, service } => write!(
                formatter,
                "`{caller}` called `{service}` and does not require it"
            ),
            Self::NoProvider { service } => {
                write!(formatter, "nothing in this set answers `{service}`")
            }
            Self::Absent { service } => write!(
                formatter,
                "nothing in this set answers `{service}`, which the caller may do without"
            ),
        }
    }
}

impl std::error::Error for ServiceUnavailable {}

/// What the loader offers a mod so it can reach the services it declared.
///
/// The mod-facing half of service resolution, the way [`crate::Registrar`] is the
/// mod-facing half of the registration barrier: a mod calls what it declared
/// without naming the loader, which `docs/architecture/dependency-rules.md`
/// forbids it from depending on.
///
/// A directory only ever reaches providers the loader has already started, so a
/// call cannot arrive at a mod that has not run — the order is a property of the
/// directory a mod is handed rather than a rule a mod is asked to respect.
pub trait ServiceDirectory {
    /// Calls the service `request` names.
    ///
    /// # Errors
    ///
    /// Returns [`ServiceCallFailed::Unavailable`] when the call reached no mod,
    /// and [`ServiceCallFailed::Refused`] when the answering mod refused it.
    ///
    /// A call to a service the caller declared it *may use* and nothing answers
    /// returns [`ServiceUnavailable::Absent`], which is an ordinary outcome to
    /// branch on rather than a reason to refuse — see
    /// [`ServiceCallFailed::is_absent`].
    fn call(&mut self, request: &ServiceRequest) -> Result<ServiceResponse, ServiceCallFailed>;
}

/// Why a call did not produce an answer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum ServiceCallFailed {
    /// The call reached nobody.
    Unavailable(ServiceUnavailable),
    /// The answering mod refused it.
    Refused(crate::HostError),
}

impl ServiceCallFailed {
    /// Returns whether this is an optionally declared service nobody installed.
    ///
    /// The one branch an optional consumer takes to carry on. Everything else —
    /// including a provider that answered and said no — is a failure it did not
    /// declare it could do without. Reading this rather than matching on the
    /// sentence is what makes "not installed" and "installed and refused" two
    /// different things to a mod and not two different strings.
    #[must_use]
    pub const fn is_absent(&self) -> bool {
        matches!(self, Self::Unavailable(ServiceUnavailable::Absent { .. }))
    }
}

impl fmt::Display for ServiceCallFailed {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::Unavailable(reason) => reason.fmt(formatter),
            Self::Refused(error) => error.fmt(formatter),
        }
    }
}

impl std::error::Error for ServiceCallFailed {
    fn source(&self) -> Option<&(dyn std::error::Error + 'static)> {
        match self {
            Self::Unavailable(reason) => Some(reason),
            Self::Refused(error) => Some(error),
        }
    }
}

#[cfg(test)]
mod tests {
    use ravel_core::{ContentId, ModId};
    use serde_json::json;

    use super::{
        ServiceDeclaration, ServiceDeclarationError, ServiceId, ServiceRequest, ServiceResponse,
        ServiceUnavailable,
    };

    fn id(value: &str) -> ModId {
        value.parse().expect("a legal mod id")
    }

    fn service(owner: &str, path: &str) -> ServiceId {
        ServiceId::new(ContentId::new(id(owner), path).expect("a legal content path"))
    }

    #[test]
    fn a_service_is_named_by_the_mod_that_answers_it() {
        let named = service("base", "prices");
        assert_eq!(named.owner(), &id("base"));
        assert_eq!(named.to_string(), "base:prices");
    }

    #[test]
    fn a_mod_cannot_offer_to_answer_another_mods_service() {
        let refusal = ServiceDeclaration::new(id("consumer"), [service("base", "prices")], [])
            .expect_err("the name is not the declaring mod's");
        assert_eq!(
            refusal,
            ServiceDeclarationError::NotOurs {
                declaring: id("consumer"),
                service: service("base", "prices"),
            }
        );
        assert!(refusal.to_string().contains("base"), "{refusal}");
    }

    #[test]
    fn a_mod_that_requires_what_it_answers_is_refused_by_name() {
        let refusal = ServiceDeclaration::new(
            id("base"),
            [service("base", "prices")],
            [service("base", "prices")],
        )
        .expect_err("nothing starts it");
        assert_eq!(
            refusal,
            ServiceDeclarationError::RequiresItself {
                declaring: id("base"),
                service: service("base", "prices"),
            }
        );
    }

    #[test]
    fn a_declaration_reports_both_halves_in_name_order() {
        let declaration = ServiceDeclaration::new(
            id("base"),
            [service("base", "stock"), service("base", "prices")],
            [service("other", "clock")],
        )
        .expect("a legal declaration");

        assert_eq!(declaration.owner(), &id("base"));
        assert_eq!(
            declaration
                .provides()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["base:prices", "base:stock"]
        );
        assert_eq!(
            declaration
                .requires()
                .map(ToString::to_string)
                .collect::<Vec<_>>(),
            ["other:clock"]
        );
    }

    #[test]
    fn a_call_and_its_answer_carry_the_values_they_were_built_with() {
        let request = ServiceRequest::new(service("base", "prices"), json!({ "item": "lantern" }));
        assert_eq!(request.arguments, json!({ "item": "lantern" }));
        assert_eq!(ServiceResponse::new(json!(3)).value(), &json!(3));
    }

    #[test]
    fn every_unavailable_reason_names_the_service_it_is_about() {
        for reason in [
            ServiceUnavailable::Undeclared {
                caller: id("consumer"),
                service: service("base", "prices"),
            },
            ServiceUnavailable::NoProvider {
                service: service("base", "prices"),
            },
            ServiceUnavailable::Absent {
                service: service("base", "prices"),
            },
        ] {
            assert!(reason.to_string().contains("base:prices"), "{reason}");
        }
    }

    #[test]
    fn an_optional_service_is_declared_callable_and_reported_as_one_it_can_do_without() {
        let prices = service("base", "prices");
        let clock = service("other", "clock");
        let declaration = ServiceDeclaration::with_optional(
            id("consumer"),
            [],
            std::slice::from_ref(&clock).to_vec(),
            std::slice::from_ref(&prices).to_vec(),
        )
        .expect("a legal declaration");

        assert_eq!(declaration.may_use().collect::<Vec<_>>(), [&prices]);
        assert_eq!(declaration.requires().collect::<Vec<_>>(), [&clock]);
        // Both calling sides are callable; only the optional one may be absent.
        assert!(declaration.calls(&prices) && declaration.calls(&clock));
        assert!(!declaration.calls(&service("base", "stock")));
        assert!(declaration.may_do_without(&prices));
        assert!(!declaration.may_do_without(&clock));
    }

    #[test]
    fn the_three_part_form_declares_nothing_optional() {
        let declaration = ServiceDeclaration::new(id("consumer"), [], [service("base", "prices")])
            .expect("a legal declaration");
        assert_eq!(declaration.may_use().count(), 0);
        assert!(!declaration.may_do_without(&service("base", "prices")));
    }

    #[test]
    fn a_mod_that_may_use_what_it_answers_is_refused_by_name() {
        // The provider is present by construction, so this optional edge is a real
        // one — and it points at the mod that drew it.
        let refusal = ServiceDeclaration::with_optional(
            id("base"),
            [service("base", "prices")],
            [],
            [service("base", "prices")],
        )
        .expect_err("nothing starts it");
        assert_eq!(
            refusal,
            ServiceDeclarationError::MayUseItself {
                declaring: id("base"),
                service: service("base", "prices"),
            }
        );
        assert!(refusal.to_string().contains("may use"), "{refusal}");
    }

    #[test]
    fn one_service_declared_both_required_and_optional_is_refused_rather_than_resolved() {
        // The two sides disagree about a missing provider. Choosing either would be
        // the host deciding what the author meant.
        let refusal = ServiceDeclaration::with_optional(
            id("consumer"),
            [],
            [service("base", "prices")],
            [service("base", "prices")],
        )
        .expect_err("the declaration says two things");
        assert_eq!(
            refusal,
            ServiceDeclarationError::RequiredAndOptional {
                declaring: id("consumer"),
                service: service("base", "prices"),
            }
        );
        assert!(refusal.to_string().contains("base:prices"), "{refusal}");
    }

    #[test]
    fn only_an_absent_optional_service_reads_as_absence() {
        use super::ServiceCallFailed;
        use crate::HostError;
        use ravel_core::LifecycleState;

        let absent = ServiceCallFailed::Unavailable(ServiceUnavailable::Absent {
            service: service("base", "prices"),
        });
        assert!(absent.is_absent());

        // A provider that was reached and said no is not absence, and neither is a
        // required service nothing answers.
        for other in [
            ServiceCallFailed::Refused(HostError::Refused {
                mod_id: id("base"),
                state: LifecycleState::Constructed,
                detail: "not today".to_owned(),
            }),
            ServiceCallFailed::Unavailable(ServiceUnavailable::NoProvider {
                service: service("base", "prices"),
            }),
            ServiceCallFailed::Unavailable(ServiceUnavailable::Undeclared {
                caller: id("consumer"),
                service: service("base", "prices"),
            }),
        ] {
            assert!(!other.is_absent(), "{other}");
        }
    }
}
