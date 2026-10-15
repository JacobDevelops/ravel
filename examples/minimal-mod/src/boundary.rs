//! Compile-pass/fail cases for the mod-author dependency and privacy boundaries.
//!
//! `tests/conformance/tests/integration/sdk_boundary.rs` checks the SDK dependency closure and
//! verifies pinned error codes. Only cases without SDK imports pin codes because
//! that check compiles them without dependencies.
//!
//! # Passing: a complete mod, through the facade alone
//!
//! ```
//! use ravel_sdk::prelude::*;
//!
//! struct Example;
//!
//! #[declare_mod(id = "example")]
//! impl Mod for Example {
//!     fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
//!         let _ = state;
//!         Ok(())
//!     }
//!
//!     fn observe(&mut self, event: GameEvent) -> Result<(), ModError> {
//!         let _ = event;
//!         Ok(())
//!     }
//!
//!     fn run_command(&mut self, invocation: &CommandInvocation) -> Result<(), ModError> {
//!         let _ = invocation;
//!         Ok(())
//!     }
//! }
//!
//! assert_eq!(Example::declared_id().as_str(), "example");
//! assert_eq!(Example.id().as_str(), "example");
//! ```
//!
//! # Passing: registration, world data, and sides, still through the facade
//!
//! ```
//! use ravel_sdk::content::{ContentId, ContentKind, Registrar, RegistryKey};
//! use ravel_sdk::save::{SavedDataDeclaration, SavedDataPlan, SavedDataVersion};
//! use ravel_sdk::{Declared, LifecycleState, Mod, ModError, Side, SideSet, declare_mod};
//!
//! struct Example;
//!
//! #[declare_mod(id = "example")]
//! impl Mod for Example {
//!     fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
//!         let _ = state;
//!         Ok(())
//!     }
//!
//!     fn register(&mut self, registrar: &mut dyn Registrar) -> Result<(), ModError> {
//!         let key = RegistryKey::new(
//!             ContentKind::Block,
//!             ContentId::new(Self::declared_id().clone(), "copper")
//!                 .expect("`copper` is a legal content path"),
//!         );
//!         registrar
//!             .claim(key)
//!             .map_err(|error| ModError::refused(error.to_string()))
//!     }
//!
//!     fn saved_data_declaration(&self) -> Result<Option<SavedDataDeclaration>, ModError> {
//!         SavedDataDeclaration::new(
//!             Self::declared_id().clone(),
//!             [SavedDataVersion::new(1).expect("1 is not zero")],
//!         )
//!         .map(Some)
//!         .map_err(|error| ModError::refused(error.to_string()))
//!     }
//! }
//!
//! let declaration = Example
//!     .saved_data_declaration()
//!     .expect("valid saved-data declaration")
//!     .expect("the example declares one version");
//! assert!(SavedDataPlan::open(&declaration, None)
//!     .expect("a fresh world initializes")
//!     .is_current());
//! assert!(SideSet::new([Side::Client])
//!     .expect("one side is a side set")
//!     .loads_on(Side::Client));
//! ```
//!
//! # Failing: the JVM entry crate is not reachable
//!
//! `docs/architecture/dependency-rules.md` says the SDK never re-exports
//! `ravel-bootstrap` and a mod cannot reach it. It is not a dependency of this
//! crate and not in `ravel-sdk`'s closure, so the name does not resolve.
//!
//! ```compile_fail,E0432
//! use ravel_bootstrap_runtime::LoaderRuntime;
//! ```
//!
//! # Failing: the only JNI-typed value in the workspace is not reachable
//!
//! `ravel-minecraft` is the highest crate that may name a JNI type and
//! `JavaHandle` is the only one it names. A mod does not see either.
//!
//! ```compile_fail,E0432
//! use ravel_minecraft::JavaHandle;
//! ```
//!
//! # Failing: Java itself is not reachable
//!
//! A mod author writes no Java and names no JVM type, so the `jni` crate is not
//! in a mod's dependency graph at all.
//!
//! ```compile_fail,E0433
//! use jni::objects::JObject;
//! ```
//!
//! # Failing: the pinned version's internals are not reachable
//!
//! `ravel-minecraft-adapter` holds the pinned version's hook inventory and mappings;
//! `ravel-minecraft-generate` owns code generation. Neither is a mod dependency, so
//! a mod compiled against the SDK carries no version internals, which is what M4-W6
//! goes on to prove against a second version.
//!
//! ```compile_fail,E0432
//! use ravel_minecraft_adapter::pinned;
//! ```
//!
//! ```compile_fail,E0433
//! use ravel_minecraft_generate::stubs::Stub;
//! ```
//!
//! # Failing: the loader is not reachable
//!
//! The registration barrier, the lifecycle machine, and the claim table belong to
//! `ravel-loader`. A mod reaches the decisions they make through
//! [`ravel_sdk::content::Registrar`] and never names the machinery.
//!
//! ```compile_fail,E0433
//! use ravel_loader::lifecycle::LifecycleSet;
//! ```
//!
//! # Failing: the assembled host, its reports, and the test harness are not reachable
//!
//! `ravel-composed-host` is the host the JVM entry point assembles,
//! `ravel-observability` holds the report and profiling surfaces the loader writes
//! through, and `ravel-testkit` drives loader processes for tests. All three sit
//! above a mod rather than beside it, and none is in a mod's graph.
//!
//! ```compile_fail,E0432
//! use ravel_bootstrap::compose;
//! ```
//!
//! ```compile_fail,E0432
//! use ravel_observability::Redactor;
//! ```
//!
//! ```compile_fail,E0432
//! use ravel_testkit::run;
//! ```
//!
//! # Failing: even the host contract crate is not an author dependency
//!
//! `ravel-host-api` declares the trait a mod implements, and `ravel-sdk`
//! re-exports it as [`ravel_sdk::Mod`]. The SDK is the one name an author has, so
//! the crate underneath it is not importable either.
//!
//! ```compile_fail,E0432
//! use ravel_host_api::HostMod;
//! ```
//!
//! # Failing: the facade has no module that leads anywhere else
//!
//! The refusals above hold because the crates are absent. This one holds because
//! the facade itself offers no path to them: adding a re-export would turn this
//! case green and fail the suite.
//!
//! ```compile_fail
//! use ravel_sdk::JavaHandle;
//! ```
//!
//! ```compile_fail
//! fn main() {
//!     let _ = ravel_sdk::bootstrap::Runtime::current();
//! }
//! ```
//!
//! # Failing: a mod cannot forge an identity the loader validated
//!
//! ADG-4 gives the host ownership of state, and identity is the first piece of
//! it. `ModId`'s inner string is private, so the only way to one is the parser
//! that enforces the grammar `schemas/package.schema.json` declares.
//!
//! The sanctioned route is the parser, which refuses the same string at runtime.
//! There is no compile-time route to an unvalidated identity: the literal form,
//! `#[declare_mod(id = ...)]`, holds its string at expansion time and runs the
//! same parser then, so an illegal id there is a build error at the author's own
//! call site rather than a panic the first time the loader asks.
//!
//! ```
//! use ravel_sdk::ModId;
//!
//! assert!("example".parse::<ModId>().is_ok());
//! assert!("Not A Legal Id".parse::<ModId>().is_err());
//! ```
//!
//! This case differs from the complete mod at the top of this file in one thing,
//! the id, so the grammar is the only candidate reason it is refused:
//!
//! ```compile_fail
//! use ravel_sdk::prelude::*;
//!
//! struct Example;
//!
//! #[declare_mod(id = "Not A Legal Id")]
//! impl Mod for Example {
//!     fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
//!         let _ = state;
//!         Ok(())
//!     }
//! }
//! ```
//!
//! ```compile_fail
//! use ravel_sdk::ModId;
//!
//! fn main() {
//!     let _ = ModId("Not A Legal Id".to_owned());
//! }
//! ```
//!
//! # Failing: a mod cannot forge a registry key
//!
//! A key names a kind and a validated identifier. Both fields are private, so a
//! mod cannot assemble one that skipped validation and hand it to the barrier.
//!
//! The two blocks differ in one thing: the first reaches the key through
//! `RegistryKey::new` and compiles, the second writes the same two values as a
//! struct literal and does not. Every other ingredient is identical, so the
//! privacy is the only candidate difference.
//!
//! ```
//! use ravel_sdk::content::{ContentId, ContentKind, RegistryKey};
//! use ravel_sdk::ModId;
//!
//! let namespace: ModId = "example".parse().expect("a legal mod id");
//! let id = ContentId::new(namespace, "copper").expect("a legal content path");
//! let _ = RegistryKey::new(ContentKind::Block, id);
//! ```
//!
//! ```compile_fail
//! use ravel_sdk::content::{ContentId, ContentKind, RegistryKey};
//! use ravel_sdk::ModId;
//!
//! fn main() {
//!     let namespace: ModId = "example".parse().expect("a legal mod id");
//!     let id = ContentId::new(namespace, "copper").expect("a legal content path");
//!     let _ = RegistryKey {
//!         kind: ContentKind::Block,
//!         id,
//!     };
//! }
//! ```
//!
//! # Failing: a mod cannot reach inside an envelope the host owns
//!
//! A [`ravel_sdk::save::SavedDataEnvelope`] is read through its accessors. Its
//! schema version and payload are not fields a mod can set, which is what keeps
//! the version in the envelope the one the loader resolved a plan against.
//!
//! The paired passing case builds the same envelope and reads the same payload
//! through the accessor the SDK offers, so the refusal is about the field and not
//! about the envelope.
//!
//! ```
//! use ravel_sdk::save::{SavedDataEnvelope, SavedDataVersion};
//! use ravel_sdk::{JsonValue, ModId};
//!
//! let owner: ModId = "example".parse().expect("a legal mod id");
//! let envelope = SavedDataEnvelope::new(
//!     owner,
//!     SavedDataVersion::new(1).expect("1 is not zero"),
//!     Vec::new(),
//!     None,
//!     JsonValue::Null,
//! );
//! assert_eq!(envelope.payload(), &JsonValue::Null);
//! ```
//!
//! ```compile_fail
//! use ravel_sdk::save::{SavedDataEnvelope, SavedDataVersion};
//! use ravel_sdk::{JsonValue, ModId};
//!
//! fn main() {
//!     let owner: ModId = "example".parse().expect("a legal mod id");
//!     let envelope = SavedDataEnvelope::new(
//!         owner,
//!         SavedDataVersion::new(1).expect("1 is not zero"),
//!         Vec::new(),
//!         None,
//!         JsonValue::Null,
//!     );
//!     let _ = envelope.payload;
//! }
//! ```
//!
//! # Failing: the declaration attribute refuses what it cannot mean
//!
//! `#[declare_mod]` generates the identity method, so a block that also writes
//! one is refused rather than silently overridden.
//!
//! ```compile_fail
//! use ravel_sdk::{Mod, ModId, declare_mod};
//!
//! struct Example {
//!     id: ModId,
//! }
//!
//! #[declare_mod(id = "example")]
//! impl Mod for Example {
//!     fn id(&self) -> &ModId {
//!         &self.id
//!     }
//! }
//! ```
//!
//! An attribute with no identity in it names what it needs instead of guessing:
//!
//! ```compile_fail
//! use ravel_sdk::{Mod, declare_mod};
//!
//! struct Example;
//!
//! #[declare_mod]
//! impl Mod for Example {}
//! ```
//!
//! And an identity bound to something other than a trait `impl` is refused,
//! because the method it generates would belong to no trait:
//!
//! ```compile_fail
//! use ravel_sdk::declare_mod;
//!
//! #[declare_mod(id = "example")]
//! struct Example;
//! ```
