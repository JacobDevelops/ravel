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

/// What one mod answers, and what it needs answered before it can start.
///
/// Both halves are declared rather than discovered, and that is what makes the
/// order decidable before any mod code runs: the loader can see the whole graph,
/// name a missing provider or a cycle, and refuse — instead of finding out when a
/// call arrives at a mod that is not there yet.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceDeclaration {
    owner: ModId,
    provides: BTreeSet<ServiceId>,
    requires: BTreeSet<ServiceId>,
}

impl ServiceDeclaration {
    /// Declares what `owner` answers and what it needs.
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
    pub fn new(
        owner: ModId,
        provides: impl IntoIterator<Item = ServiceId>,
        requires: impl IntoIterator<Item = ServiceId>,
    ) -> Result<Self, ServiceDeclarationError> {
        let provides: BTreeSet<ServiceId> = provides.into_iter().collect();
        let requires: BTreeSet<ServiceId> = requires.into_iter().collect();

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
        Ok(Self {
            owner,
            provides,
            requires,
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
    /// The caller did not declare that it requires this service.
    Undeclared {
        /// The mod that called.
        caller: ModId,
        /// The service it called.
        service: ServiceId,
    },
    /// Nothing in this set answers the service.
    ///
    /// A resolved set cannot reach this — the loader refuses a missing provider
    /// before any mod runs — so it exists for a directory built over a set that
    /// was never resolved, which is a host mistake worth naming rather than
    /// papering over.
    NoProvider {
        /// The service nobody answers.
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
        ] {
            assert!(reason.to_string().contains("base:prices"), "{reason}");
        }
    }
}
