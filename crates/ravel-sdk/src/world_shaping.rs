//! What shaped a world, written into the world once.
//!
//! [`crate::save`] versions the data a mod owns and migrates it, so a world
//! written by an older build reaches the shape this one reads and a world that
//! cannot is refused rather than opened. Removing a mod that *shaped* a world
//! was the silent case: the chunks it made stay, the chunks made next are made
//! by something else, and nothing says so.
//!
//! A mod that shapes a world declares itself in a [`WorldShaping`]. Ravel writes
//! that set into the world at creation as a [`ShapingRecord`], and
//! [`ShapingRecord::admit`] answers, before any world data is read, whether the
//! set now installed is the set that made it. The refusal names what is missing
//! and what has arrived, because both are the same divergence read from opposite
//! ends.
//!
//! What "shaping" means is the mod's own — a generator, a biome, a placement.
//! Ravel models none of it and this module names none of it. What Ravel owns is
//! the set, because the set is what a world has to remember and no member of it
//! can remember on the world's behalf.
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
//! let read = ShapingRecord::read(&written.bytes()).expect("what was written reads back");
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
