//! What one mod asks another for.
//!
//! A service is a named call one mod answers and others make. It is the one thing
//! in this facade that crosses from one mod to another at run time, so it is
//! deliberately narrow: a [`ServiceId`] is namespaced by the mod that answers it,
//! so exactly one mod can, and a [`ServiceDeclaration`] states both halves — what
//! this mod answers and what it needs answered — before any of it runs.
//!
//! Declaring rather than discovering is what makes the order decidable. The loader
//! reads every declaration, orders providers before consumers, and refuses a set
//! whose services do not resolve: a service nothing answers, or two mods that
//! require each other. Both refusals name the offender and happen before a mod is
//! started, so nothing is half-wired.
//!
//! [`crate::Mod::use_services`] is where a mod makes its calls, and the
//! [`ServiceDirectory`] it is handed reaches only mods the loader has already
//! started. A call to a mod later in the order is not something to avoid — there
//! is nothing there to call.
//!
//! # A request and its answer are data
//!
//! [`ServiceRequest::arguments`] and [`ServiceResponse::value`] are JSON, not a
//! Rust type both mods name. That is the same trade [`crate::network`] makes and
//! it is made for the same reason: ADG-2 distributes a mod as source the player's
//! machine compiles, so a consumer that named the provider's Rust type would be
//! pinned to the provider's build rather than to the version range its package
//! manifest declares. A provider owns the shape of its answer and versions it the
//! way it versions anything else it publishes.
//!
//! # Examples
//!
//! ```
//! use ravel_sdk::content::ContentId;
//! use ravel_sdk::services::{
//!     ServiceDeclaration, ServiceDirectory, ServiceId, ServiceRequest, ServiceResponse,
//! };
//! use ravel_sdk::{Declared, LifecycleState, Mod, ModError, declare_mod};
//! use serde_json::json;
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
//!     fn service_declaration(&self) -> Option<ServiceDeclaration> {
//!         ServiceDeclaration::new(Self::declared_id().clone(), [prices()], []).ok()
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
//!     fn service_declaration(&self) -> Option<ServiceDeclaration> {
//!         ServiceDeclaration::new(Self::declared_id().clone(), [], [prices()]).ok()
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

pub use ravel_host_api::{
    ServiceCallFailed, ServiceDeclaration, ServiceDeclarationError, ServiceDirectory, ServiceId,
    ServiceRequest, ServiceResponse, ServiceUnavailable,
};
