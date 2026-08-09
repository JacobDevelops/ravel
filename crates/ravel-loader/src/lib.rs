#![forbid(unsafe_code)]

//! Loader orchestration independent of any concrete Minecraft version.

mod admission;
mod driver;
mod events;
mod lifecycle;
mod registry;
mod surface;
mod world;

pub use admission::admitted;
pub use driver::{DriveError, drive};
pub use events::{
    Delivery, DispatchRefused, broadcast, broadcast_profiled, deliver_packet, route, route_profiled,
};
pub use lifecycle::{
    LifecycleEntry, LifecycleError, LifecycleEvent, LifecycleMachine, Requirement,
    executes_mod_code,
};
pub use registry::Registration;
pub use surface::{ModEntry, ModSurface};
pub use world::WorldRefused;
