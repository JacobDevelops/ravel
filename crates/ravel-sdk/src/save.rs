//! Versioned world data and atomic migration chains.
//!
//! A mod runs its [`MigrationChain`] over the stored payload using the host's
//! [`SavedDataPlan`], then reads current data and returns a [`SavedDataEnvelope`].
//! The host owns file I/O; unmet package requirements refuse opening.
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

/// The mod-defined JSON payload stored in a versioned envelope.
pub use serde_json::Value as SavedPayload;

pub use ravel_save::{
    MAX_AGGREGATE_BYTES, MigrationChain, MigrationFailed, MigrationStep, PackageConflict,
    RequiredPackage, SavedDataDeclaration, SavedDataEnvelope, SavedDataError, SavedDataPlan,
    SavedDataVersion,
};
