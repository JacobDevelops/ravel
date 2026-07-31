#![forbid(unsafe_code)]

//! Loader orchestration independent of any concrete Minecraft version.

mod lifecycle;
mod world;

pub use lifecycle::{
    LifecycleEntry, LifecycleError, LifecycleEvent, LifecycleMachine, Requirement,
    executes_mod_code,
};
pub use world::WorldRefused;
