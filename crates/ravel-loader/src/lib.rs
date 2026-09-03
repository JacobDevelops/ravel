#![forbid(unsafe_code)]

//! Loader orchestration independent of any concrete Minecraft version.

mod admission;
mod config;
mod discovery;
mod driver;
mod events;
mod interception;
mod lifecycle;
mod pack;
mod registry;
mod resources;
mod saves;
mod services;
mod surface;
mod world;

pub use admission::admitted;
pub use config::{ConfigRefused, Configured, open_configuration};
pub use discovery::{
    Discovered, DiscoveredPackage, DiscoveryRefused, MANIFEST_FILE, PACKAGES_DIRECTORY, discover,
};
pub use driver::{DriveError, drive};
pub use events::{
    Delivery, DispatchRefused, broadcast, broadcast_profiled, deliver_packet, route, route_profiled,
};
pub use interception::{InterceptionError, intercept};
pub use lifecycle::{
    LifecycleEntry, LifecycleError, LifecycleEvent, LifecycleMachine, Requirement,
    executes_mod_code,
};
pub use pack::{
    Half, PACK_DIRECTORY, PackError, RESOURCE_PACKS, WORLD_DATA_PACKS, Written, pack_id, write_pack,
};
pub use registry::Registration;
pub use resources::{ResourceRefused, Supplied, open_resources};
pub use saves::{SaveRefused, open_saved_data};
pub use services::{ServiceOrder, ServiceRefused, open_services};
pub use surface::{ModEntry, ModSurface};
pub use world::WorldRefused;
