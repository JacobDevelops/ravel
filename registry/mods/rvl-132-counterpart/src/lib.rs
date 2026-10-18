#![forbid(unsafe_code)]

//! An original Ravel implementation of the behaviour specified in RVL-132.
//!
//! A bridging library with no gameplay of its own: it ships no block, no item,
//! no recipe document, no texture and no screen, and a player can encounter it
//! only indirectly, because it makes vanilla containers, stations and dropped
//! items answer questions that only modded machines ask.
//!
//! Section 1.1 says what survives as counterpart work and this package is that
//! list: the three ingredient kinds and their matching, ordering and hashing
//! ([`kinds`], [`data`]), the serialised forms ([`forms`]), the adapter table
//! between store shapes ([`storage`]), the attachment table ([`capabilities`]),
//! and the recipe views ([`recipes`]).
//!
//! **The interface vocabulary is not here.** Section five's abstraction, its
//! registry, the matcher and serialiser interfaces, the candidate set and the
//! candidate value, and the slot-addressed and slotless storage abstractions are
//! all owned by RVL-210's counterpart by the wave's decision. This package
//! depends on `rvl-210-counterpart` and consumes them from its crate root, and
//! redeclares none of them. `NOTES.md` records that ownership.
//!
//! What a reader reaches for:
//!
//! - [`kinds`] — the three ingredient kinds, their conditions and their order;
//! - [`data`] — the shared data comparison a pack-visible setting changes;
//! - [`forms`] — the tag forms an instance and a condition are stored in;
//! - [`storage`] — the twelve store adapters and what they must not lose;
//! - [`capabilities`] — the two compatibility bundles and 47 attachments;
//! - [`readings`] — the work and heat readings;
//! - [`recipes`] — the three recipe views and the brewing closure;
//! - [`holdings`] — the two item stores that live inside items;
//! - [`cache`] — the three caches and the reload that clears them;
//! - [`settings`] — the settings document and the two it declines;
//! - [`census`] — what the pinned entry counts;
//! - [`dumps`] — the generated documents those tables render to.

pub mod cache;
pub mod capabilities;
pub mod census;
pub mod data;
pub mod dumps;
pub mod forms;
pub mod holdings;
pub mod kinds;
pub mod readings;
pub mod recipes;
pub mod settings;
pub mod storage;

mod registration;

pub use registration::{Entry, Refused, Step};

use ravel_sdk::prelude::export_mod;

/// The identity this package is declared and validated under.
pub const MOD_ID: &str = "rvl_132_counterpart";

export_mod!(Entry::new());
