#![forbid(unsafe_code)]

//! An original Ravel implementation of the behaviour specified in RVL-002.
//!
//! A leaf of its wave's content cluster. It extends the tiered-crop entry by one
//! tier above that entry's numbered progression, and it adds the content that
//! tier needs to be reachable: a ground block, a material, six crops, four
//! deposits and four families of consumables and tools that span all six tiers
//! rather than only the new one.
//!
//! Almost everything it does is declared data or a small numeric rule applied to
//! declared data. What survives as counterpart work is exactly that: the fuel
//! derivation, the food effect rule, the tool grade table and dispatch rule, the
//! boss drop rule, the ground behaviour it shares with its dependency, and the
//! three states of absence.
//!
//! What a dependent reaches for:
//!
//! - [`settings`] — the eight settings, the five flags they publish and the two
//!   they only read;
//! - [`tier`] — the sixth tier, its four declared and four adjusted values;
//! - [`crops`] — the six crops and which of the three absence states each
//!   reaches;
//! - [`census`] — the registered content, counted, and the creative listing's
//!   order;
//! - [`ground`] — the sixth-tier ground, its drops and what gates conversion;
//! - [`fuel`] — the two derivations behind all twelve burn times;
//! - [`foods`] — the six rows, the cumulative effects and the additive duration;
//! - [`tools`] — the grade table, unbreakability and modifier dispatch;
//! - [`drops`] — what a defeated boss adds to a drop event;
//! - [`recipes`] — the recipe census, its conditions, and the two counts;
//! - [`world`] — the four deposits and the shaping set;
//! - [`data`] — loot, tags, translations and what survives a reload;
//! - [`dumps`] — the documents those tables render out under `generated/`.
//!
//! `NOTES.md` beside this file says which sections are carried here and which
//! are left declarable only because Ravel publishes no seam for them.

pub mod census;
pub mod crops;
pub mod data;
pub mod drops;
pub mod dumps;
pub mod foods;
pub mod fuel;
pub mod ground;
pub mod recipes;
pub mod settings;
pub mod tier;
pub mod tools;
pub mod world;

mod registration;

pub use registration::Entry;

use ravel_sdk::prelude::export_mod;

pub use crate::settings::Settings;

/// The identity this package is declared and validated under.
pub const MOD_ID: &str = "rvl_002_counterpart";

/// The namespace the host's own content and tags are named in.
pub const HOST_NAMESPACE: &str = "host";

/// The namespace conventional cross-mod tags are named in.
///
/// A tag named there has no owner, which is the whole point of the convention:
/// every mod that makes an ingot puts one in `c:ingots`.
pub const CONVENTION_NAMESPACE: &str = "c";

export_mod!(Entry::new());
