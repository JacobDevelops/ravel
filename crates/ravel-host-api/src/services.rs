use std::collections::BTreeSet;
use std::fmt;

use ravel_core::{ContentId, ModId};
use serde_json::Value;

/// A service identifier namespaced by the only mod allowed to answer it.
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

/// Declares provided, required, and optional services before load ordering.
///
/// Missing required services refuse the set; missing optional services report
/// [`ServiceUnavailable::Absent`]. Present providers precede either kind of consumer.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct ServiceDeclaration {
    owner: ModId,
    provides: BTreeSet<ServiceId>,
    requires: BTreeSet<ServiceId>,
    may_use: BTreeSet<ServiceId>,
}

impl ServiceDeclaration {
    /// Declares provided and required services with no optional dependencies.
    ///
    /// # Errors
    ///
    /// Returns the same declaration errors as [`Self::with_optional`].
    pub fn new(
        owner: ModId,
        provides: impl IntoIterator<Item = ServiceId>,
        requires: impl IntoIterator<Item = ServiceId>,
    ) -> Result<Self, ServiceDeclarationError> {
        Self::with_optional(owner, provides, requires, [])
    }

    /// Declares provided, required, and optional services.
    ///
    /// # Errors
    ///
    /// Returns an error for foreign ownership, self-dependencies, or required/optional overlap.
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

    /// Returns whether the mod declared `service` as required or optional.
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

/// A service call with JSON arguments, independent of the provider's Rust types.
#[derive(Clone, Debug, Eq, PartialEq)]
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
#[derive(Clone, Debug, Eq, PartialEq)]
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

/// A call that never reached a provider, distinct from a provider's refusal.
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
    /// A required provider is missing; this indicates an unresolved host directory.
    NoProvider {
        /// The service nobody answers.
        service: ServiceId,
    },
    /// An optional provider is absent, not a provider that was reached and refused.
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

/// Access to declared services whose providers the loader has already started.
pub trait ServiceDirectory {
    /// Calls the service `request` names.
    ///
    /// # Errors
    ///
    /// Returns [`ServiceCallFailed::Unavailable`] for no provider, or [`ServiceCallFailed::Refused`] for a provider's refusal.
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
    /// Returns whether an optional provider is absent, not present and refusing.
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
            [clock.clone()],
            [prices.clone()],
        )
        .expect("a legal declaration");

        assert_eq!(declaration.may_use().collect::<Vec<_>>(), [&prices]);
        assert_eq!(declaration.requires().collect::<Vec<_>>(), [&clock]);
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
