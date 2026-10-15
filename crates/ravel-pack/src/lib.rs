#![forbid(unsafe_code)]

//! Pack declarations, closure validation, and reproducible output.
//!
//! Validation checks dependencies, resources, registry claims, progression, and settings.
//! It does not check game-version compatibility, the game's pack layout, or whether
//! entries in [`VANILLA_NAMESPACE`] exist.

mod declare;
mod error;
mod progression;
mod validate;
mod write;

pub use declare::{PackDeclaration, PackError, ResourceReference, SuppliedFile};
pub use error::{ClosureError, UnprovidedEntry};
pub use progression::{Progression, ProgressionError, ProgressionState};
pub use validate::{PackFile, RESERVED_PREFIX, VANILLA_NAMESPACE, ValidatedPack, validate};
pub use write::WriteError;
