#![forbid(unsafe_code)]

//! Loader orchestration independent of any concrete Minecraft version.

mod lifecycle;

pub use lifecycle::{
    LifecycleEntry, LifecycleError, LifecycleEvent, LifecycleMachine, Requirement,
    executes_mod_code,
};
