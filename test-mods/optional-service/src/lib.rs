#![forbid(unsafe_code)]

//! A compass optionally consumes a survey service, ordered after it when present.
//!
//! [`Bearings`] distinguishes not-yet-wired, absent, and answered states.
//! Only absence is tolerated; an installed provider's refusal propagates.

use ravel_sdk::content::ContentId;
use ravel_sdk::services::{
    ServiceDeclaration, ServiceDirectory, ServiceId, ServiceRequest, ServiceResponse,
};
use ravel_sdk::{Declared, LifecycleState, Mod, ModError, declare_mod, json};

/// The identity the optional provider is packaged under.
pub const SURVEY_MOD_ID: &str = "optional_survey";

/// The identity the presence-gated consumer is packaged under.
pub const COMPASS_MOD_ID: &str = "optional_compass";

/// The service [`SurveyMod`] answers and [`CompassMod`] may use.
///
/// # Panics
///
/// Panics if `landmarks` is not a legal content path.
#[must_use]
pub fn landmarks() -> ServiceId {
    ServiceId::new(
        ContentId::new(SurveyMod::declared_id().clone(), "landmarks")
            .expect("`landmarks` is a legal path"),
    )
}

/// A compass's wiring state, distinguishing absence from not having been wired.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub enum Bearings {
    /// This mod has not been offered its services yet.
    #[default]
    Unwired,
    /// It asked, and nothing in this set answers the survey.
    Alone,
    /// A survey answered, and this is what it named.
    Guided(Vec<String>),
}

impl Bearings {
    /// Returns the landmarks a survey named, when one did.
    #[must_use]
    pub fn named(&self) -> Option<&[String]> {
        match self {
            Self::Guided(names) => Some(names),
            Self::Unwired | Self::Alone => None,
        }
    }
}

/// A mod other mods may use and none has to install.
///
/// # Examples
///
/// ```
/// use ravel_sdk::Mod;
/// use ravel_test_mod_optional_service::{SurveyMod, landmarks};
///
/// let survey = SurveyMod::new();
/// assert_eq!(survey.id().as_str(), "optional_survey");
///
/// // It is an ordinary provider: nothing about it knows it is optional, because
/// // optionality is the consumer's statement about the dependency.
/// let declaration = survey.service_declaration().expect("valid declaration").expect("it answers one service");
/// assert_eq!(declaration.provides().collect::<Vec<_>>(), [&landmarks()]);
/// assert_eq!(declaration.may_use().count(), 0);
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct SurveyMod {
    known: Vec<String>,
    answered: usize,
    refusing: bool,
}

impl SurveyMod {
    /// Builds the provider, holding two landmarks.
    #[must_use]
    pub fn new() -> Self {
        Self {
            known: vec!["spire".to_owned(), "hollow".to_owned()],
            answered: 0,
            refusing: false,
        }
    }

    /// Builds a provider that is installed, runs, and says no to every call.
    ///
    /// The row that separates "no survey" from "a survey that refused". Both leave
    /// a consumer without landmarks and they are not the same event.
    #[must_use]
    pub fn refusing() -> Self {
        Self {
            refusing: true,
            ..Self::new()
        }
    }

    /// Returns the landmarks this survey holds.
    #[must_use]
    pub fn known(&self) -> &[String] {
        &self.known
    }

    /// Returns how many calls this survey has answered.
    #[must_use]
    pub const fn answered(&self) -> usize {
        self.answered
    }
}

#[declare_mod(id = "optional_survey")]
impl Mod for SurveyMod {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        let _ = state;
        Ok(())
    }

    fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
        ServiceDeclaration::new(Self::declared_id().clone(), [landmarks()], [])
            .map(Some)
            .map_err(|error| ModError::Refused {
                mod_id: self.id().clone(),
                state: LifecycleState::Constructed,
                detail: error.to_string(),
            })
    }

    /// Answers with the landmarks this survey holds.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] from a survey built by
    /// [`SurveyMod::refusing`]. A refusal here is this mod answering, which is why
    /// a consumer must not read it as this mod being absent.
    fn answer_service(&mut self, request: &ServiceRequest) -> Result<ServiceResponse, ModError> {
        let _ = request;
        if self.refusing {
            return Err(ModError::Refused {
                mod_id: Self::declared_id().clone(),
                state: LifecycleState::Running,
                detail: "this survey has no chart loaded".to_owned(),
            });
        }
        self.answered += 1;
        Ok(ServiceResponse::new(json!({ "landmarks": self.known })))
    }
}

/// A mod that is better with another mod installed and works without it.
///
/// # Examples
///
/// ```
/// use ravel_sdk::Mod;
/// use ravel_test_mod_optional_service::{Bearings, CompassMod, landmarks};
///
/// let compass = CompassMod::new();
/// assert_eq!(compass.id().as_str(), "optional_compass");
/// // Nothing has been asked yet, which is a third state and not "alone".
/// assert_eq!(compass.bearings(), &Bearings::Unwired);
///
/// let declaration = compass.service_declaration().expect("valid declaration").expect("it may use one service");
/// assert_eq!(declaration.may_use().collect::<Vec<_>>(), [&landmarks()]);
/// // Nothing is required, so this mod never stops a set from loading.
/// assert_eq!(declaration.requires().count(), 0);
/// ```
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct CompassMod {
    bearings: Bearings,
}

impl CompassMod {
    /// Builds the consumer.
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Returns what this compass knows, and how it came to know it.
    #[must_use]
    pub const fn bearings(&self) -> &Bearings {
        &self.bearings
    }
}

#[declare_mod(id = "optional_compass")]
impl Mod for CompassMod {
    fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
        let _ = state;
        Ok(())
    }

    /// Declares the survey as a service this mod may use rather than one it needs.
    ///
    /// The difference is the whole mod: with `requires` in place of the optional
    /// argument, installing this compass without the survey would refuse the set.
    fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
        ServiceDeclaration::with_optional(Self::declared_id().clone(), [], [], [landmarks()])
            .map(Some)
            .map_err(|error| ModError::Refused {
                mod_id: self.id().clone(),
                state: LifecycleState::Constructed,
                detail: error.to_string(),
            })
    }

    /// Calls the survey when one is installed, and records doing without when none
    /// is.
    ///
    /// # Errors
    ///
    /// Returns [`ModError::Refused`] for every outcome except absence — including
    /// a survey that was reached and refused. That branch reads
    /// `ServiceCallFailed::is_absent` rather than the refusal's sentence, so "no
    /// survey installed" and "the survey said no" stay two different events to this
    /// mod and not two different strings.
    fn use_services(&mut self, directory: &mut dyn ServiceDirectory) -> Result<(), ModError> {
        match directory.call(&ServiceRequest::new(landmarks(), json!({ "within": 512 }))) {
            Ok(answer) => {
                let named = answer
                    .value()
                    .get("landmarks")
                    .and_then(|value| value.as_array())
                    .ok_or_else(|| ModError::Refused {
                        mod_id: Self::declared_id().clone(),
                        state: LifecycleState::Constructed,
                        detail: "the survey answered without a `landmarks` list".to_owned(),
                    })?
                    .iter()
                    .filter_map(|value| value.as_str().map(ToOwned::to_owned))
                    .collect();
                self.bearings = Bearings::Guided(named);
                Ok(())
            }
            Err(failed) if failed.is_absent() => {
                self.bearings = Bearings::Alone;
                Ok(())
            }
            Err(failed) => Err(ModError::Refused {
                mod_id: Self::declared_id().clone(),
                state: LifecycleState::Constructed,
                detail: failed.to_string(),
            }),
        }
    }
}

#[cfg(test)]
mod tests {
    use ravel_sdk::services::{ServiceCallFailed, ServiceResponse, ServiceUnavailable};
    use ravel_sdk::{Declared, LifecycleState, Mod, json};

    use super::{
        Bearings, COMPASS_MOD_ID, CompassMod, SURVEY_MOD_ID, ServiceDirectory, ServiceRequest,
        SurveyMod, landmarks,
    };

    /// A directory that answers exactly one way, so each branch can be reached.
    ///
    /// The composed proof — that a real loader produces these outcomes, in this
    /// order — is `tests/conformance/tests/integration/optional_services.rs`, over
    /// `ravel_loader::open_services`. This fixture only reaches the branches; it is
    /// not evidence about what a host does.
    struct Fixed(Result<ServiceResponse, ServiceCallFailed>);

    impl ServiceDirectory for Fixed {
        fn call(
            &mut self,
            _request: &ServiceRequest,
        ) -> Result<ServiceResponse, ServiceCallFailed> {
            self.0.clone()
        }
    }

    #[test]
    fn both_halves_are_packaged_under_the_identities_they_report() {
        assert_eq!(SurveyMod::declared_id().as_str(), SURVEY_MOD_ID);
        assert_eq!(CompassMod::declared_id().as_str(), COMPASS_MOD_ID);
        assert_eq!(landmarks().owner(), SurveyMod::declared_id());
    }

    #[test]
    fn the_compass_requires_nothing_and_may_use_the_survey() {
        let declaration = CompassMod::new()
            .service_declaration()
            .expect("valid service declaration")
            .expect("it declares one service");
        assert_eq!(declaration.requires().count(), 0);
        assert_eq!(declaration.may_use().collect::<Vec<_>>(), [&landmarks()]);
        assert!(declaration.may_do_without(&landmarks()));
        assert!(declaration.calls(&landmarks()));
    }

    #[test]
    fn an_answer_leaves_the_compass_holding_what_the_survey_named() {
        let mut compass = CompassMod::new();
        compass
            .use_services(&mut Fixed(Ok(ServiceResponse::new(
                json!({ "landmarks": ["spire", "hollow"] }),
            ))))
            .expect("an answered call");
        assert_eq!(
            compass.bearings().named(),
            Some(&["spire".to_owned(), "hollow".to_owned()][..])
        );
    }

    #[test]
    fn absence_leaves_the_compass_alone_rather_than_unwired_or_refusing() {
        let mut compass = CompassMod::new();
        compass
            .use_services(&mut Fixed(Err(ServiceCallFailed::Unavailable(
                ServiceUnavailable::Absent {
                    service: landmarks(),
                },
            ))))
            .expect("absence is not failure");
        assert_eq!(compass.bearings(), &Bearings::Alone);
        assert!(compass.bearings().named().is_none());
    }

    #[test]
    fn a_survey_that_answered_and_refused_refuses_the_compass_too() {
        // The event a consumer must not swallow: the survey is installed, ran, and
        // said no. Reading that as absence would hide a broken survey as a missing
        // one, and no player could tell the difference from the mod list.
        let mut survey = SurveyMod::refusing();
        let refusal = survey
            .answer_service(&ServiceRequest::new(landmarks(), json!(null)))
            .expect_err("this survey has no chart");

        let mut compass = CompassMod::new();
        let failed = compass
            .use_services(&mut Fixed(Err(ServiceCallFailed::Refused(refusal))))
            .expect_err("a refusal is not absence");
        assert_eq!(failed.mod_id(), CompassMod::declared_id());
        assert!(failed.to_string().contains("no chart loaded"), "{failed}");
        assert_eq!(
            compass.bearings(),
            &Bearings::Unwired,
            "a refused compass recorded a branch it did not take"
        );
    }

    #[test]
    fn an_undeclared_or_required_shortfall_is_not_absence_either() {
        // `Absent` is the only outcome this mod carries on past. Both of these say
        // something is wrong with the wiring rather than with the install.
        for reason in [
            ServiceUnavailable::NoProvider {
                service: landmarks(),
            },
            ServiceUnavailable::Undeclared {
                caller: CompassMod::declared_id().clone(),
                service: landmarks(),
            },
        ] {
            let mut compass = CompassMod::new();
            assert!(
                compass
                    .use_services(&mut Fixed(Err(ServiceCallFailed::Unavailable(reason))))
                    .is_err()
            );
            assert_eq!(compass.bearings(), &Bearings::Unwired);
        }
    }

    #[test]
    fn the_survey_answers_what_it_holds_and_counts_the_calls() {
        let mut survey = SurveyMod::new();
        assert_eq!(survey.answered(), 0);
        let answer = survey
            .answer_service(&ServiceRequest::new(landmarks(), json!({ "within": 512 })))
            .expect("an ordinary survey answers");
        assert_eq!(answer.value(), &json!({ "landmarks": ["spire", "hollow"] }));
        assert_eq!(survey.known(), ["spire", "hollow"]);
        assert_eq!(survey.answered(), 1);
    }

    #[test]
    fn neither_half_declares_anything_it_does_not_need() {
        let compass = CompassMod::new();
        assert!(
            compass
                .saved_data_declaration()
                .expect("valid declaration")
                .is_none()
        );
        assert!(
            compass
                .configuration_declaration()
                .expect("valid declaration")
                .is_none()
        );
        let mut compass = compass;
        compass.enter(LifecycleState::Running).expect("accepted");
        let mut survey = SurveyMod::new();
        survey.enter(LifecycleState::Running).expect("accepted");
    }
}
