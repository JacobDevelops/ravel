//! Versioned world data a mod owns.
//!
//! `SPEC.md` §14 requires persistent data to be versioned, so a mod that owns
//! any declares every version this build can read. The loader resolves the
//! declaration against what the world holds and produces a [`SavedDataPlan`]:
//! either the version is current, or there is an ordered list of migration steps
//! to run, or the world is refused. A refusal withholds the world rather than
//! opening it half-applied.
//!
//! **The mod crosses those steps, not the host.** The payload a mod is handed is
//! at whatever version the world stored, so an implementation reads
//! [`SavedDataPlan::steps`] and applies its chain to them; one that reads the
//! payload as though it were already current loses every field a later version
//! added. A host is free to hand over an already-current payload, and one of them
//! does, but that is a stronger guarantee than the contract rather than the
//! contract itself.
//!
//! [`SavedDataPlan`] decides which versions a world has to cross and
//! [`MigrationChain`] is what crosses them: one [`MigrationStep`] per version, run
//! in order, producing a new payload rather than editing the stored one. Every
//! step or none — a chain that refuses partway leaves the world holding exactly
//! what it held, which is what makes a refusal a withheld world rather than a
//! half-migrated one.
//!
//! A mod never reads or writes the world file. It answers with one
//! [`SavedDataEnvelope`] and the loader owns the aggregate those envelopes go
//! into, which is what keeps [`MAX_AGGREGATE_BYTES`] a property of the whole
//! world rather than of one mod's good behaviour.
//!
//! # Examples
//!
//! ```
//! use ravel_sdk::save::{SavedDataDeclaration, SavedDataPlan, SavedDataVersion};
//! use ravel_sdk::{Declared, LifecycleState, Mod, ModError, declare_mod};
//!
//! struct Example;
//!
//! #[declare_mod(id = "example")]
//! impl Mod for Example {
//!     fn enter(&mut self, state: LifecycleState) -> Result<(), ModError> {
//!         let _ = state;
//!         Ok(())
//!     }
//! }
//!
//! let one = SavedDataVersion::new(1).expect("1 is not zero");
//! let two = SavedDataVersion::new(2).expect("2 is not zero");
//! let declaration =
//!     SavedDataDeclaration::new(Example::declared_id().clone(), [one, two])?;
//!
//! // A world already at the version this build writes needs no migration.
//! let current = SavedDataPlan::open(&declaration, Some(two))?;
//! assert!(current.is_current());
//!
//! // An older world names the steps, in order.
//! let older = SavedDataPlan::open(&declaration, Some(one))?;
//! assert_eq!(older.steps(), [two]);
//!
//! // A fresh world holds nothing, and absence initializes rather than refuses.
//! assert!(SavedDataPlan::open(&declaration, None)?.is_current());
//! # Ok::<(), ravel_sdk::save::SavedDataError>(())
//! ```

/// The payload an envelope carries.
///
/// A mod's state is its own to shape, so this is an open JSON value rather than
/// a type Ravel defines. The version in the envelope beside it is what makes an
/// old shape readable, and the migration steps in a [`SavedDataPlan`] are what
/// make it current.
pub use serde_json::Value as SavedPayload;

pub use ravel_save::{
    MAX_AGGREGATE_BYTES, MigrationChain, MigrationFailed, MigrationStep, PackageConflict,
    RequiredPackage, SavedDataDeclaration, SavedDataEnvelope, SavedDataError, SavedDataPlan,
    SavedDataVersion,
};
