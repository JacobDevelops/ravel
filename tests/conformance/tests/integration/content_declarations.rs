#![forbid(unsafe_code)]

//! Content declarations, registration conflicts, and deterministic generated packs.

use ravel_core::ModId;

mod declarations;
mod generation;
mod packs;
mod registration;

fn id(text: &str) -> ModId {
    text.parse().expect("a legal mod id")
}
