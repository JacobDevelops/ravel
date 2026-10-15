//! Named JSON requests between mods, with providers started before consumers.
//!
//! Declare required services with [`ServiceDeclaration::new`] or optional
//! services with [`ServiceDeclaration::with_optional`], then call them from
//! [`crate::Mod::use_services`]. Optional absence is distinct from provider refusal.
//!
//! # Examples
//!
//! ```
//! use ravel_sdk::content::ContentId;
//! use ravel_sdk::services::{
//!     ServiceDeclaration, ServiceDirectory, ServiceId, ServiceRequest, ServiceResponse,
//! };
//! use ravel_sdk::{Declared, LifecycleState, Mod, ModError, declare_mod, json};
//!
//! #[derive(Default)]
//! struct Base;
//!
//! fn prices() -> ServiceId {
//!     ServiceId::new(
//!         ContentId::new(Base::declared_id().clone(), "prices").expect("a legal path"),
//!     )
//! }
//!
//! #[declare_mod(id = "base")]
//! impl Mod for Base {
//!     fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
//!         let _ = state;
//!         Ok(())
//!     }
//!
//!     fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
//!         ServiceDeclaration::new(Self::declared_id().clone(), [prices()], [])
//!             .map(Some)
//!             .map_err(|error| ModError::Refused {
//!                 mod_id: Self::declared_id().clone(),
//!                 state: LifecycleState::Constructed,
//!                 detail: error.to_string(),
//!             })
//!     }
//!
//!     fn answer_service(
//!         &mut self,
//!         request: &ServiceRequest,
//!     ) -> Result<ServiceResponse, ModError> {
//!         let item = request.arguments.get("item").and_then(|value| value.as_str());
//!         Ok(ServiceResponse::new(json!(if item == Some("lantern") {
//!             3
//!         } else {
//!             0
//!         })))
//!     }
//! }
//!
//! // A consumer declares what it needs and calls it through the directory.
//! #[derive(Default)]
//! struct Shop {
//!     lantern_price: u64,
//! }
//!
//! #[declare_mod(id = "shop")]
//! impl Mod for Shop {
//!     fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
//!         let _ = state;
//!         Ok(())
//!     }
//!
//!     fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
//!         ServiceDeclaration::new(Self::declared_id().clone(), [], [prices()])
//!             .map(Some)
//!             .map_err(|error| ModError::Refused {
//!                 mod_id: Self::declared_id().clone(),
//!                 state: LifecycleState::Constructed,
//!                 detail: error.to_string(),
//!             })
//!     }
//!
//!     fn use_services(
//!         &mut self,
//!         directory: &mut dyn ServiceDirectory,
//!     ) -> Result<(), ModError> {
//!         let answer = directory
//!             .call(&ServiceRequest::new(prices(), json!({ "item": "lantern" })))
//!             .map_err(|failed| ModError::Refused {
//!                 mod_id: Self::declared_id().clone(),
//!                 state: LifecycleState::Constructed,
//!                 detail: failed.to_string(),
//!             })?;
//!         self.lantern_price = answer.value().as_u64().unwrap_or_default();
//!         Ok(())
//!     }
//! }
//!
//! // A mod may only offer to answer names in its own namespace.
//! assert!(
//!     ServiceDeclaration::new(Shop::declared_id().clone(), [prices()], []).is_err()
//! );
//! assert_eq!(prices().to_string(), "base:prices");
//! ```
//!
//! # Example: a mod that uses another only if it is installed
//!
//! ```
//! use ravel_sdk::content::ContentId;
//! use ravel_sdk::services::{
//!     ServiceDeclaration, ServiceDirectory, ServiceId, ServiceRequest,
//! };
//! use ravel_sdk::{Declared, LifecycleState, Mod, ModError, declare_mod, json};
//!
//! fn landmarks() -> ServiceId {
//!     let atlas = "atlas".parse().expect("a legal mod id");
//!     ServiceId::new(ContentId::new(atlas, "landmarks").expect("a legal path"))
//! }
//!
//! #[derive(Default)]
//! struct Compass {
//!     alone: bool,
//! }
//!
//! #[declare_mod(id = "compass")]
//! impl Mod for Compass {
//!     fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
//!         let _ = state;
//!         Ok(())
//!     }
//!
//!     fn service_declaration(&self) -> Result<Option<ServiceDeclaration>, ModError> {
//!         // Nothing required, one service used if something answers it.
//!         ServiceDeclaration::with_optional(
//!             Self::declared_id().clone(),
//!             [],
//!             [],
//!             [landmarks()],
//!         )
//!         .map(Some)
//!         .map_err(|error| ModError::Refused {
//!             mod_id: Self::declared_id().clone(),
//!             state: LifecycleState::Constructed,
//!             detail: error.to_string(),
//!         })
//!     }
//!
//!     fn use_services(
//!         &mut self,
//!         directory: &mut dyn ServiceDirectory,
//!     ) -> Result<(), ModError> {
//!         match directory.call(&ServiceRequest::new(landmarks(), json!(null))) {
//!             Ok(_answer) => Ok(()),
//!             // The one outcome an optional consumer carries on past.
//!             Err(failed) if failed.is_absent() => {
//!                 self.alone = true;
//!                 Ok(())
//!             }
//!             // A provider that was reached and said no is a different event.
//!             Err(failed) => Err(ModError::Refused {
//!                 mod_id: Self::declared_id().clone(),
//!                 state: LifecycleState::Constructed,
//!                 detail: failed.to_string(),
//!             }),
//!         }
//!     }
//! }
//!
//! let declaration = Compass::default()
//!     .service_declaration()
//!     .expect("valid service declaration")
//!     .expect("it declares one service");
//! assert!(declaration.may_do_without(&landmarks()));
//! assert_eq!(declaration.requires().count(), 0);
//!
//! // One service cannot be both: the two sides disagree about a missing provider.
//! assert!(
//!     ServiceDeclaration::with_optional(
//!         Compass::declared_id().clone(),
//!         [],
//!         [landmarks()],
//!         [landmarks()],
//!     )
//!     .is_err()
//! );
//! ```

pub use ravel_host_api::{
    ServiceCallFailed, ServiceDeclaration, ServiceDeclarationError, ServiceDirectory, ServiceId,
    ServiceRequest, ServiceResponse, ServiceUnavailable,
};
