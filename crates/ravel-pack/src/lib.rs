#![forbid(unsafe_code)]

//! Pack pinning, declarative composition, and closure validation.
//!
//! A modpack is not a mod and this crate is not in a mod's build graph. `SPEC.md`
//! §4.3 describes the pack author as the role that selects mods, pins exact
//! versions, configures them declaratively, validates dependency, resource,
//! registry and progression consistency, and produces a reproducible manifest.
//! This crate is the pinning, the declarative configuration, the four closure
//! checks, and the manifest. It is not the *selection*: §4.3's first bullet says a
//! pack author selects **compatible** mods, and nothing here reads a member's
//! `compatibility.minecraft`. A [`PackDeclaration`] holds no game version, so a pack
//! can pin members that declare a game this installation is not, and all four
//! closures still hold — which the shipped example does.
//!
//! [`PackDeclaration`] is what a pack author writes and [`validate`] is the only
//! thing that decides whether it holds together. The split is the same one
//! `ravel-data` draws between a declaration and the sets several declarations add
//! up to: a declaration refuses what one author can get wrong alone — a member
//! pinned twice, a settings document filed under the wrong mod — and everything
//! that needs the members' manifests is a closure check.
//!
//! A [`ValidatedPack`] is the only thing that can be written, so a pack whose
//! closures do not hold has no files to emit. That is the same shape as
//! `ravel-save`'s admitted aggregate and `ravel-loader`'s opened services: the
//! successful value is the proof, rather than a boolean beside it.
//!
//! # What this crate does not check
//!
//! Whether the layout it writes under `data/` and `assets/` is the layout the pinned
//! Minecraft version reads. That is a belief this repository holds and has not checked
//! against the pinned jar, and [`RESERVED_PREFIX`] exists so the files that are Ravel's own
//! are separable from the files that are that belief. [`VANILLA_NAMESPACE`] carries
//! the other half: a tag member in the game's namespace is allowed and is not
//! verified to exist, because what the game provides is inside the pinned artifact.
//!
//! Whether anything reads what this crate writes. Nothing in Ravel does. The
//! `data/` and `assets/` half is written against this repository's belief about the
//! game's pack format, and the [`RESERVED_PREFIX`] half is written against a reader
//! that does not exist yet: no code outside this crate and its own suite reads
//! `ravel/`, and `ravel-loader`'s `open_configuration` takes its pack documents from
//! a caller-supplied map that no production caller fills. So a written pack is
//! validated only against itself, and the write→read round trip is unprovable by
//! construction. That is the same construction that hid three blocking defects across
//! eleven M3 review rounds, so "the pack writes correct bytes" is not established
//! until something in Ravel reads them back. `ROADMAP.md`'s open-questions table
//! records the owed work.
//!
//! Which keys a pack's settings layer may state. See the comment at the settings
//! write in `validate`; the check exists in `ravel-config` and runs at launch rather
//! than here.
//!
//! All of these are stated where the code is rather than only in a report, because the
//! next reader of this crate is the person who would otherwise assume they were checked.

mod declare;
mod progression;
mod validate;
mod write;

pub use declare::{PackDeclaration, PackError, ResourceReference, SuppliedFile};
pub use progression::{Progression, ProgressionError, ProgressionState};
pub use validate::{
    ClosureError, PackFile, RESERVED_PREFIX, UnprovidedEntry, VANILLA_NAMESPACE, ValidatedPack,
    validate,
};
pub use write::WriteError;
