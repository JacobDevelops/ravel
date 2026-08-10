#![forbid(unsafe_code)]

//! Declarative resource and data contracts for mods and packs.
//!
//! A mod says what content it contributes and this crate decides what that
//! means. [`ContentDeclaration`] is the one place a mod says it, [`TagSet`] and
//! [`ResourceSet`] are what several mods' declarations add up to, and
//! [`GeneratedData`] is the pack one declaration produces.
//!
//! The three kinds of declaration differ in exactly one way, and the difference
//! is why they are modelled separately rather than as one table:
//!
//! | Declaration | Owners | A second declaration of the same name |
//! | --- | --- | --- |
//! | registry entry | one | a conflict the registration barrier refuses |
//! | tag membership | none | a contribution, merged with the first |
//! | resource path | one supplier | an override; the later mod supplies it |
//!
//! Nothing here reads a file, opens a package, or knows a load order. A caller
//! supplies the resolved order, which is `ravel-loader`'s to decide, and this
//! crate is what turns that order into an answer a report or a game can use.

mod audio;
mod declare;
mod generate;
mod render_stages;
mod resources;
mod server_data;
mod tags;

pub use audio::{AudioDeclaration, AudioEntry, AudioError, AudioEvent, PlayRequest};
pub use declare::{ContentDeclaration, DeclarationError};
pub use generate::{GeneratedData, GeneratedFile};
pub use render_stages::{RenderStage, StageDeclaration, StageOrder, StageRefused};
pub use resources::{Override, Resource, ResourceSet};
pub use server_data::{
    GeneratedServerData, ServerDataDeclaration, ServerDataEntry, ServerDataError, ServerDataKind,
};
pub use tags::{TagError, TagId, TagSet};
