#![forbid(unsafe_code)]

//! Safe public API for Ravel mod authors.
//!
//! This crate is the whole of what a mod needs to name. A mod that lists
//! `ravel-sdk` and nothing else from this workspace reaches the author-facing
//! surface through what is re-exported here, and reaches nothing else: a
//! transitive dependency is not nameable in Rust, so what this crate does not
//! re-export is not reachable through it.
//!
//! Nothing stops an author from writing another Ravel crate into their own
//! manifest — `ravel-package`'s model does not read a mod's `Cargo.toml`
//! contents, and no crate here is published, so nothing draws that line. What
//! holds is that an author never needs to, which is what
//! `examples/minimal-mod` demonstrates and `tests/conformance/tests/sdk_boundary.rs`
//! recomputes.
//!
//! # What a mod is
//!
//! A mod is a Rust type that implements [`Mod`], with `#[declare_mod]` binding
//! it to the identity its package declares:
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
//! `enter` is the one step every mod is asked to take, so it is the one method
//! an author writes; `id` is the one `#[declare_mod]` generates. Everything else
//! has a default, so the block above is a complete mod. A mod that registers
//! content implements [`Mod::register`], a mod that reacts to the game
//! implements [`Mod::observe`], and a mod that owns world data implements the
//! saved-data methods.
//!
//! # What a mod declares
//!
//! Content is declared in one place and claimed from there. A
//! [`content::ContentDeclaration`] holds a mod's registry entries, its tag
//! memberships, the translations it supplies and any bytes Ravel does not
//! generate for it; [`content::claim_all`] takes the entries to the registration
//! barrier, and [`content::ContentDeclaration::generate`] produces the files.
//!
//! The three kinds of declaration behave differently once several mods are
//! installed, and each module says so where an author will read it:
//!
//! - [`content`] — registry entries, which have one owner, so a second claim is
//!   a conflict the barrier refuses;
//! - [`tags`] — memberships, which have no owner, so contributions merge;
//! - [`resources`] — supplied paths, which have one supplier, so the later mod
//!   overrides and the mods it shadowed are named;
//! - [`generate`] — the pack a declaration produces, byte for byte the same each
//!   time it is produced.
//!
//! # What a mod does while it runs
//!
//! [`events`] is what the game did and what a player typed. [`network`] is what a
//! mod declares it sends between a client and a server: a contract, and a value
//! that round-trips under it or is refused — declared and testable today, and not
//! yet carried anywhere, which that module states plainly. [`sides`] is which of
//! those a mod is doing — and [`sides::ClientOnly`] is where its client-only work
//! goes, so that work is not merely hidden on a server side but never runs there.
//!
//! [`config`] is what a player and a pack may change about a mod, declared as a
//! document and resolved into the values in force for one launch. [`save`] is the
//! versioned world data a mod owns, and [`save::MigrationChain`] is how a world
//! written by an older build reaches the shape this one reads — every step or
//! none, so a world is refused rather than opened part-migrated. [`services`] is
//! the one thing here that crosses from one mod to another while both run: a named
//! call, ordered so a provider starts before anything that needs it.
//!
//! # What this crate deliberately does not expose
//!
//! `docs/architecture/dependency-rules.md` requires the SDK to expose no
//! bootstrap detail, no generated JVM type, and no Minecraft-version internal.
//! There is therefore no path through this crate to `ravel-bootstrap`, to
//! `ravel-minecraft` and the `JavaHandle` that is the workspace's only JNI-typed
//! value, to `ravel-minecraft-adapter` and the pinned version's data, to
//! `ravel-loader`, or to `jni` itself — and none of those is a dependency of
//! this crate, so a mod's dependency graph does not contain them either. A mod
//! author writes no Java and names no JVM type.
//!
//! `examples/minimal-mod` is a mod written against this surface alone, and
//! `examples/minimal-mod/src/boundary.rs` is the compile-pass/fail suite that
//! holds the two paragraphs above to something a compiler checks.

// `#[declare_mod]` expands to paths rooted at `::ravel_sdk`, because that is the
// one crate a mod's manifest lists and the only name generated code can rely on.
// This crate's own tests use the attribute, so it needs that name for itself.
extern crate self as ravel_sdk;

pub mod config;
pub mod content;
pub mod events;
pub mod generate;
pub mod network;
pub mod resources;
pub mod save;
pub mod services;
pub mod sides;
pub mod tags;

pub use ravel_core::{LifecycleState, ModId, ModIdError, Side, SideSet, Version, VersionRange};
/// Why a mod refused the step the loader asked it to take.
///
/// A mod raises [`ModError::Refused`] with its own identity, the state it was
/// asked to enter, and a sentence in its own words. The loader attributes the
/// failure to that mod and that phase rather than to the set.
pub use ravel_host_api::HostError as ModError;
/// What a mod implements, and the whole of what the loader asks of one.
///
/// A mod writes `enter`, because every mod is taken through the lifecycle, and
/// `#[declare_mod]` generates `id`. Every other method has a default, so a mod
/// implements only the parts of the lifecycle it takes part in.
pub use ravel_host_api::HostMod as Mod;
pub use ravel_sdk_macros::declare_mod;
/// Why a JSON document did not parse.
///
/// The error `str::parse::<`[`JsonValue`]`>` returns. It is re-exported for the
/// same reason [`JsonValue`] is: without a name here, a mod that wanted to carry
/// the failure — in its own error type, or in a signature — could not write the
/// type down, because `serde_json` is not nameable from a crate that lists
/// `ravel-sdk` and nothing else.
///
/// It implements [`std::error::Error`] and [`std::fmt::Display`], so the usual
/// route is `error.to_string()` into a [`ModError::Refused`] detail.
pub use serde_json::Error as JsonError;
/// One of the open JSON values this facade carries.
///
/// Three surfaces here are deliberately open rather than typed by Ravel: a
/// [`save::SavedPayload`], a [`services::ServiceRequest`]'s arguments and a
/// [`services::ServiceResponse`]'s answer, and a [`config`] setting's value. All
/// three are this type, and this is the name a mod uses for it. `serde_json` is
/// not nameable from a crate that lists `ravel-sdk` and nothing else, so without
/// a name here an author who wanted to write the type down — in a signature, or
/// as the receiver of an associated function like `JsonValue::as_u64` — had one
/// spelling available, `save::SavedPayload`, which says "saved data" about a
/// setting that has nothing to do with a world.
///
/// [`save::SavedPayload`] remains the name for the payload in an envelope, where
/// the saved-data reading is the right one.
///
/// # Reading a document a mod was given
///
/// [`json`] builds a value a mod wrote. A value a mod *received* as text — a
/// file it read, a string a player typed into a command — is parsed with
/// `str::parse`, because this type implements [`std::str::FromStr`]. That is the
/// whole of it: there is no `from_str` function re-exported here, and none is
/// needed. The error is [`JsonError`], which is named here so a mod can write
/// the failure down as well as produce it.
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
/// // The same value, read from text rather than built.
/// let parsed: JsonValue = r#"{ "stock": 12 }"#.parse()?;
/// assert_eq!(parsed, json!({ "stock": 12 }));
///
/// // A document that is not JSON is an error a mod can name and report.
/// let refused: Result<JsonValue, JsonError> = "{".parse();
/// assert!(refused.is_err());
/// # Ok::<(), JsonError>(())
/// ```
pub use serde_json::Value as JsonValue;
/// Builds a [`JsonValue`].
///
/// Each of the open surfaces [`JsonValue`] names belongs to the mod that shapes
/// it. This is the constructor for all of them, so an author never has to add a
/// second dependency to write one down.
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

/// A type `#[declare_mod]` has bound an identity to.
///
/// The attribute implements this, and it is the only thing in the SDK an author
/// does not write by hand and cannot usefully write by hand: the stored
/// [`ModId`] is parsed once from the declared string and shared for the process.
///
/// It is a separate trait from [`Mod`] because the identity belongs to the type
/// rather than to an instance. A mod that needs its own id before it has been
/// constructed — to build a [`content::RegistryKey`] in an associated function,
/// say — calls [`Declared::declared_id`], and one that has an instance calls
/// `Mod::id`.
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
    pub use crate::config::{ConfigurationDeclaration, LayeredConfiguration, SettingKey};
    pub use crate::content::{
        ContentDeclaration, ContentId, ContentKind, Registrar, RegistrationRefused, RegistryKey,
        claim_all,
    };
    pub use crate::events::{CommandInvocation, GameEvent};
    pub use crate::network::{Direction, Packet, PacketContract, PacketDelivery, PacketId};
    pub use crate::save::{
        MigrationChain, MigrationFailed, SavedDataDeclaration, SavedDataVersion,
    };
    pub use crate::services::{
        ServiceDeclaration, ServiceDirectory, ServiceId, ServiceRequest, ServiceResponse,
    };
    pub use crate::sides::ClientOnly;
    pub use crate::tags::TagId;
    pub use crate::{Declared, LifecycleState, Mod, ModError, ModId, Side, declare_mod};
}

#[cfg(test)]
mod tests {
    use crate::content::{ContentId, ContentKind, Registrar, RegistrationRefused, RegistryKey};
    use crate::events::{CommandInvocation, GameEvent};
    use crate::{Declared, LifecycleState, Mod, ModError, ModId, Side, declare_mod};

    /// A registrar that accepts everything, so a test can drive registration
    /// without the loader that owns the real claim table.
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

    #[test]
    fn a_refusal_carries_the_mod_and_the_phase_it_happened_in() {
        let error = ModError::Refused {
            mod_id: Example::declared_id().clone(),
            state: LifecycleState::Constructed,
            detail: "the example declined".to_owned(),
        };
        assert_eq!(error.mod_id(), Example::declared_id());
        assert_eq!(error.state(), LifecycleState::Constructed);
    }
}
