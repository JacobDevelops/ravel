//! The M4-W1 compile-pass/fail suite.
//!
//! `ROADMAP.md`'s M4-W1 row is verified by a compile-pass/fail suite proving
//! authors need no loader internals or Java. This module is that suite, and it
//! holds no code: every case below is a doctest, so `cargo test --doc` compiles
//! each one and fails if a passing case stops compiling **or** if a failing case
//! starts.
//!
//! # Why the cases live here
//!
//! A compile-fail case only means something in a crate that can see exactly what
//! a mod author can see. This crate's `Cargo.toml` lists one dependency,
//! `ravel-sdk`, which makes its doctests' import scope an author's import scope —
//! and `tests/conformance/tests/sdk_boundary.rs` reads that manifest and refuses
//! a second entry, so the scope cannot be widened without the gate saying so.
//! That test also recomputes what `ravel-sdk`'s dependency closure contains from
//! `Cargo.lock`, the resolver's own answer, so "not importable" here is backed by
//! "not in the graph" there rather than by this module's word for it.
//!
//! A pass-only suite would prove nothing about a boundary. The refusals are the
//! evidence.
//!
//! # Which cases carry an error code, and why the rest do not
//!
//! **Stable rustdoc ignores the code after `compile_fail`** — checking it is
//! nightly-only. A review set one case to `E0999`, which is not a real code, and
//! the suite stayed green. So a code written here means nothing unless something
//! else recomputes it, and
//! `the_pinned_error_codes_are_the_codes_rustc_reports` in
//! `tests/conformance/tests/sdk_boundary.rs` is that something: it reads the
//! cases out of this file, compiles each pinned one, and asserts rustc reports
//! the code the case names.
//!
//! That check compiles a case with no dependencies at all, which is faithful for
//! a case whose whole content is a crate name that does not resolve — the one
//! extern rustdoc gives these doctests, `ravel_sdk`, has no bearing on whether
//! `jni` resolves. It is not faithful for a case that names `ravel_sdk` itself,
//! so **those cases carry no code**, and the same test fails if one acquires one.
//!
//! What carries the weight for them is which kind of case it is. For the facade
//! cases, adding the missing re-export is what turns the case green. For a
//! private field or constructor the refusal is paired with a passing case that
//! reaches the same value by the route the SDK sanctions, so the difference
//! between the two is the privacy and cannot be anything else.
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
//!         registrar.claim(key).map_err(|refusal| ModError::Refused {
//!             mod_id: Self::declared_id().clone(),
//!             state: LifecycleState::Constructed,
//!             detail: refusal.to_string(),
//!         })
//!     }
//!
//!     fn saved_data_declaration(&self) -> Option<SavedDataDeclaration> {
//!         SavedDataDeclaration::new(
//!             Self::declared_id().clone(),
//!             [SavedDataVersion::new(1).expect("1 is not zero")],
//!         )
//!         .ok()
//!     }
//! }
//!
//! let declaration = Example
//!     .saved_data_declaration()
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
//! ```compile_fail,E0433
//! use ravel_bootstrap::runtime::Runtime;
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
//! `ravel-minecraft-adapter` holds the pinned version's hook inventory, mappings, and
//! generated bindings. A mod compiled against the SDK carries no version
//! internals, which is what M4-W6 goes on to prove against a second version.
//!
//! ```compile_fail,E0433
//! use ravel_minecraft_adapter::generate::stubs::Stub;
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
//! use ravel_composed_host::compose;
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
//! use ravel_sdk::save::{SavedDataEnvelope, SavedDataVersion, SavedPayload};
//! use ravel_sdk::ModId;
//!
//! let owner: ModId = "example".parse().expect("a legal mod id");
//! let envelope = SavedDataEnvelope::new(
//!     owner,
//!     SavedDataVersion::new(1).expect("1 is not zero"),
//!     Vec::new(),
//!     None,
//!     SavedPayload::Null,
//! );
//! assert_eq!(envelope.payload(), &SavedPayload::Null);
//! ```
//!
//! ```compile_fail
//! use ravel_sdk::save::{SavedDataEnvelope, SavedDataVersion};
//! use ravel_sdk::ModId;
//!
//! fn main() {
//!     let owner: ModId = "example".parse().expect("a legal mod id");
//!     let envelope = SavedDataEnvelope::new(
//!         owner,
//!         SavedDataVersion::new(1).expect("1 is not zero"),
//!         Vec::new(),
//!         None,
//!         ravel_sdk::save::SavedPayload::Null,
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
