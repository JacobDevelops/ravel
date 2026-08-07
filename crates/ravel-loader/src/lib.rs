#![forbid(unsafe_code)]

//! Loader orchestration independent of any concrete Minecraft version.

mod driver;
mod events;
mod lifecycle;
mod registry;
mod surface;
mod world;

pub use driver::{DriveError, drive};
pub use events::{Delivery, DispatchRefused, broadcast, broadcast_profiled, route, route_profiled};
pub use lifecycle::{
    LifecycleEntry, LifecycleError, LifecycleEvent, LifecycleMachine, Requirement,
    executes_mod_code,
};
pub use registry::Registration;
pub use surface::{ModEntry, ModSurface};
pub use world::WorldRefused;
