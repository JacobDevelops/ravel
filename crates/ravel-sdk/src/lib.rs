#![forbid(unsafe_code)]

//! Safe public API for Ravel mod authors.
//!
//! Implement [`Mod`], bind its identity with [`declare_mod`], and expose its
//! constructor with [`export_mod`]. The modules below provide version-neutral
//! declarations and runtime contracts; no JVM or host internals are exposed.
//!
//! ```
//! use ravel_sdk::{Declared, LifecycleState, Mod, ModError, declare_mod};
//!
//! #[derive(Default)]
//! struct Example {
//!     entered: Vec<LifecycleState>,
//! }
//!
//! #[declare_mod(id = "example")]
//! impl Mod for Example {
//!     fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
//!         self.entered.push(state);
//!         Ok(())
//!     }
//! }
//!
//! let mut example = Example::default();
//! assert_eq!(Example::declared_id().as_str(), "example");
//! assert_eq!(example.id().as_str(), "example");
//!
//! example.enter(LifecycleState::Loaded)?;
//! assert_eq!(example.entered, [LifecycleState::Loaded]);
//! # Ok::<(), ModError>(())
//! ```
//!

// Let attribute expansions use the same absolute path inside this crate.
extern crate self as ravel_sdk;

pub mod audio;
pub mod config;
pub mod content;
pub mod events;
pub mod generate;
pub mod interception;
pub mod network;
pub mod provision;
pub mod render_stages;
pub mod resources;
pub mod save;
pub mod server_data;
pub mod services;
pub mod sides;
pub mod tags;
pub mod world_shaping;

pub use ravel_core::{LifecycleState, ModId, ModIdError, Side, SideSet, Version, VersionRange};
/// A lifecycle refusal attributed to the mod and phase that failed.
pub use ravel_host_api::HostError as ModError;
/// The lifecycle contract a mod implements; only `enter` is required.
pub use ravel_host_api::HostMod as Mod;
pub use ravel_sdk_macros::declare_mod;
/// The error returned when parsing a [`JsonValue`] from text.
pub use serde_json::Error as JsonError;
/// A JSON value used by configuration, saved data, and service messages.
///
/// Parse received text with [`str::parse`]; build values with [`json`].
///
/// # Examples
///
/// ```
/// use ravel_sdk::{JsonError, JsonValue, json};
///
/// fn capacity(value: &JsonValue) -> Option<u64> {
///     value.as_u64()
/// }
///
/// assert_eq!(capacity(&json!(16)), Some(16));
/// assert_eq!(capacity(&json!("sixteen")), None);
///
/// let parsed: JsonValue = r#"{ "stock": 12 }"#.parse()?;
/// assert_eq!(parsed, json!({ "stock": 12 }));
///
/// let refused: Result<JsonValue, JsonError> = "{".parse();
/// assert!(refused.is_err());
/// # Ok::<(), JsonError>(())
/// ```
pub use serde_json::Value as JsonValue;
/// Builds a [`JsonValue`].
///
/// # Examples
///
/// ```
/// use ravel_sdk::json;
///
/// let payload = json!({ "stock": 12, "currency": "coin" });
/// assert_eq!(payload["stock"], 12);
/// ```
pub use serde_json::json;

/// Exports the constructor a generated host calls for this package.
///
/// Invoke once at the crate root. Use `export_mod!(|side| expression)` when
/// construction needs the running [`Side`].
///
/// # Examples
///
/// ```
/// use ravel_sdk::{LifecycleState, Mod, ModError, Side, declare_mod, export_mod};
///
/// #[derive(Default)]
/// struct Example;
///
/// #[declare_mod(id = "example")]
/// impl Mod for Example {
///     fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
///         let _ = state;
///         Ok(())
///     }
/// }
///
/// export_mod!(Example::default());
///
/// // What a host calls, with the side it is running on.
/// assert_eq!(__ravel_entry(Side::Client).id().as_str(), "example");
/// ```
///
/// A mod that is built differently on each side:
///
/// ```
/// use ravel_sdk::{LifecycleState, Mod, ModError, Side, declare_mod, export_mod};
///
/// struct Example {
///     side: Side,
/// }
///
/// #[declare_mod(id = "example")]
/// impl Mod for Example {
///     fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
///         let _ = state;
///         Ok(())
///     }
/// }
///
/// export_mod!(|side| Example { side });
/// ```
#[macro_export]
macro_rules! export_mod {
    (|$side:ident| $value:expr) => {
        /// The mod this package builds, as a host constructs it.
        ///
        /// Written by `ravel_sdk::export_mod!`. A host calls it by this name and
        /// nothing else calls it at all.
        #[doc(hidden)]
        pub fn __ravel_entry(side: $crate::Side) -> ::std::boxed::Box<dyn $crate::Mod> {
            let $side = side;
            ::std::boxed::Box::new($value)
        }
    };
    ($value:expr) => {
        $crate::export_mod!(|__ravel_side| {
            let _ = __ravel_side;
            $value
        });
    };
}

/// A type's declared identity, parsed once and shared for the process.
///
/// [`declare_mod`] implements this trait. Use [`Declared::declared_id`] before
/// constructing a mod, or [`Mod::id`] on an instance.
///
/// # Examples
///
/// ```
/// use ravel_sdk::{Declared, LifecycleState, Mod, ModError, declare_mod};
///
/// struct Example;
///
/// #[declare_mod(id = "example")]
/// impl Mod for Example {
///     fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
///         let _ = state;
///         Ok(())
///     }
/// }
///
/// assert_eq!(Example::declared_id().as_str(), "example");
/// assert_eq!(Example.id(), Example::declared_id());
/// ```
pub trait Declared {
    /// Returns the identity the declaration bound to this type.
    fn declared_id() -> &'static ModId;
}

/// The names a mod almost always wants.
///
/// ```
/// use ravel_sdk::prelude::*;
///
/// struct Example;
///
/// #[declare_mod(id = "example")]
/// impl Mod for Example {
///     fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
///         let _ = state;
///         Ok(())
///     }
/// }
///
/// assert_eq!(Example::declared_id().as_str(), "example");
/// ```
pub mod prelude {
    pub use crate::audio::AudioDeclaration;
    pub use crate::config::{ConfigurationDeclaration, LayeredConfiguration, SettingKey};
    pub use crate::content::{
        ContentDeclaration, ContentId, ContentKind, Registrar, RegistrationRefused, RegistryKey,
        claim_all,
    };
    pub use crate::events::{CommandInvocation, GameEvent};
    pub use crate::interception::{Outcome, Point, Points};
    pub use crate::network::{Direction, Packet, PacketContract, PacketDelivery, PacketId};
    pub use crate::render_stages::{RenderStage, StageDeclaration};
    pub use crate::save::{
        MigrationChain, MigrationFailed, SavedDataDeclaration, SavedDataVersion,
    };
    pub use crate::server_data::{ServerDataDeclaration, ServerDataKind};
    pub use crate::services::{
        ServiceDeclaration, ServiceDirectory, ServiceId, ServiceRequest, ServiceResponse,
    };
    pub use crate::sides::ClientOnly;
    pub use crate::tags::TagId;
    pub use crate::world_shaping::{ShapingRecord, WorldShaping};
    pub use crate::{
        Declared, LifecycleState, Mod, ModError, ModId, Side, declare_mod, export_mod,
    };
}

#[cfg(test)]
mod tests {
    use crate::content::{ContentId, ContentKind, Registrar, RegistrationRefused, RegistryKey};
    use crate::events::{CommandInvocation, GameEvent};
    use crate::{Declared, LifecycleState, Mod, ModError, ModId, Side, declare_mod};

    #[derive(Default)]
    struct Accepting {
        claimed: Vec<RegistryKey>,
    }

    impl Registrar for Accepting {
        fn claim(&mut self, key: RegistryKey) -> Result<(), RegistrationRefused> {
            self.claimed.push(key);
            Ok(())
        }

        fn claimed(&self) -> &[RegistryKey] {
            &self.claimed
        }

        fn loads(&self, mod_id: &ModId) -> bool {
            let _ = mod_id;
            false
        }
    }

    #[derive(Default)]
    struct Example {
        entered: Vec<LifecycleState>,
        observed: Vec<GameEvent>,
        ran: Vec<Vec<String>>,
    }

    impl Example {
        fn command() -> RegistryKey {
            RegistryKey::new(
                ContentKind::Command,
                ContentId::new(Self::declared_id().clone(), "example")
                    .expect("`example` is a legal content path"),
            )
        }
    }

    #[declare_mod(id = "example")]
    impl Mod for Example {
        fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
            self.entered.push(state);
            Ok(())
        }

        fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), ModError> {
            registrar
                .claim(Self::command())
                .map_err(|refusal| ModError::Refused {
                    mod_id: Self::declared_id().clone(),
                    state: LifecycleState::Constructed,
                    detail: refusal.to_string(),
                })
        }

        fn observe(&mut self, event: GameEvent) -> Result<(), ModError> {
            self.observed.push(event);
            Ok(())
        }

        fn run_command(&mut self, invocation: &CommandInvocation) -> Result<(), ModError> {
            self.ran.push(invocation.arguments.clone());
            Ok(())
        }
    }

    #[test]
    fn a_declared_mod_reports_the_identity_the_attribute_bound_to_it() {
        let expected: ModId = "example".parse().expect("`example` is a legal mod id");
        assert_eq!(Example::declared_id(), &expected);
        assert_eq!(Example::default().id(), &expected);
    }

    #[test]
    fn the_stored_identity_is_parsed_once_and_shared() {
        assert!(std::ptr::eq(Example::declared_id(), Example::declared_id()));
    }

    #[test]
    fn the_defaulted_methods_the_block_did_not_write_still_answer() {
        let example = Example::default();
        assert!(example.saved_data_declaration().is_none());
        assert!(
            example
                .save_saved_data()
                .expect("a mod that owns no world data produces no envelope")
                .is_none()
        );
    }

    #[test]
    fn the_whole_author_facing_lifecycle_runs_through_the_facade_alone() {
        let mut example = Example::default();
        let mut registrar = Accepting::default();

        for state in LifecycleState::ALL {
            example
                .enter(state)
                .expect("the example accepts every step");
        }
        example
            .register(&mut registrar)
            .expect("the accepting registrar refuses nothing");
        example
            .observe(GameEvent::ServerTick)
            .expect("the example records every event");
        example
            .run_command(&CommandInvocation::new(
                Example::command(),
                Side::IntegratedServer,
                vec!["first".to_owned()],
            ))
            .expect("the example runs the command it claimed");

        assert_eq!(example.entered, LifecycleState::ALL);
        assert_eq!(registrar.claimed(), [Example::command()]);
        assert_eq!(example.observed, [GameEvent::ServerTick]);
        assert_eq!(example.ran, [["first".to_owned()]]);
        assert_eq!(
            Example::command().to_string(),
            "command:example:example",
            "a key built from the declared identity names that identity"
        );
    }
}
