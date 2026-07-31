#![forbid(unsafe_code)]

//! Loader orchestration independent of any concrete Minecraft version.

mod lifecycle;
mod registry;
mod surface;
mod world;

pub use lifecycle::{
    LifecycleEntry, LifecycleError, LifecycleEvent, LifecycleMachine, Requirement,
    executes_mod_code,
};
pub use registry::Registration;
pub use surface::{ModEntry, ModSurface};
pub use world::WorldRefused;
