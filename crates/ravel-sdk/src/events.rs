//! What the game did, and what a player typed.
//!
//! [`GameEvent`] is a closed set, and that is deliberate: an unknown event would
//! name something no version adapter has resolved a hook for. The loader
//! broadcasts every event to every running mod in resolved load order, so a mod
//! that cares about ordering sees the same order each run, and a `match` that
//! ignores an event costs one arm.
//!
//! A [`CommandInvocation`] reaches only the one mod that claimed the root
//! literal during the registration window, so a mod is never asked to run a
//! command it does not own and never has to check.
//!
//! # Why there is no `hooks` module
//!
//! A hook is the host-side mechanism M3 settled: a named target in a version's
//! `hooks.toml`, resolved against that version's mappings and installed into the
//! game by `ravel-bootstrap`. This module is its mod-facing form. A mod author
//! never names a hook, does not choose one, and cannot be affected by one moving
//! between versions — which is the point of `SPEC.md` §3.7's version boundary, and
//! the reason `docs/architecture/dependency-rules.md` keeps the SDK clear of
//! version internals. Exposing a hook here would breach that rule and hand an
//! author a name that is only stable for one Minecraft version. So the absence is
//! a decision rather than an omission: [`GameEvent`] is what a hook becomes by the
//! time it reaches a mod.
//!
//! # Examples
//!
//! ```
//! use ravel_sdk::events::GameEvent;
//! use ravel_sdk::Side;
//!
//! // A client tick has no meaning on a dedicated server, so a mod on that side
//! // is not called rather than being handed an event to learn to ignore.
//! assert!(!GameEvent::ClientTick.happens_on(Side::DedicatedServer));
//! assert!(GameEvent::ServerTick.happens_on(Side::IntegratedServer));
//! assert_eq!(GameEvent::ResourcesReloaded.as_str(), "resources-reloaded");
//! ```
//!
//! A command's arguments are text a player typed and nothing before the mod
//! interprets them:
//!
//! ```
//! use ravel_sdk::content::{ContentId, ContentKind, RegistryKey};
//! use ravel_sdk::events::CommandInvocation;
//! use ravel_sdk::{ModId, Side};
//!
//! let namespace: ModId = "example".parse()?;
//! let key = RegistryKey::new(
//!     ContentKind::Command,
//!     ContentId::new(namespace, "home")?,
//! );
//! let invocation = CommandInvocation::new(
//!     key,
//!     Side::IntegratedServer,
//!     vec!["set".to_owned(), "base".to_owned()],
//! );
//!
//! assert_eq!(invocation.arguments, ["set", "base"]);
//! # Ok::<(), Box<dyn std::error::Error>>(())
//! ```

pub use ravel_host_api::{CommandInvocation, GameEvent};
