//! Records of the mods that shaped a world.
//!
//! [`ShapingRecord::at_creation`] records a [`WorldShaping`] set;
//! [`ShapingRecord::admit`] refuses a changed set and names missing or added mods.
//!
//! No runtime integration exists: no [`crate::Mod`] hook declares a shaping set,
//! and no world-open path writes or admits a record. These values describe the
//! check a host would make; a mod calling them checks only itself.
//!
//! # Examples
//!
//! ```
//! use ravel_sdk::world_shaping::{ShapingRecord, WorldShaping};
//! use ravel_sdk::ModId;
//!
//! let terrain: ModId = "terrain_mod".parse()?;
//! let caves: ModId = "cave_mod".parse()?;
//!
//! let mut at_creation = WorldShaping::default();
//! at_creation.declare(terrain.clone());
//! at_creation.declare(caves.clone());
//!
//! let written = ShapingRecord::at_creation(&at_creation);
//! let read = ShapingRecord::read(&written.to_bytes()).expect("what was written reads back");
//! assert_eq!(read.mods(), [terrain.clone(), caves.clone()]);
//!
//! // The same set opens the world; one of them gone does not.
//! read.admit(&at_creation).expect("what made the world may open it");
//!
//! let mut without_caves = WorldShaping::default();
//! without_caves.declare(terrain);
//! let refusal = read
//!     .admit(&without_caves)
//!     .expect_err("a world is refused when what shaped it is gone");
//! assert_eq!(refusal.missing(), [caves]);
//! # Ok::<(), ravel_sdk::ModIdError>(())
//! ```

pub use ravel_save::{ShapingError, ShapingMismatch, ShapingRecord, WorldShaping};
