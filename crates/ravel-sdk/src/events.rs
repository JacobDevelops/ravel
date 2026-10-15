//! Version-neutral game events and player commands.
//!
//! Events reach running mods in resolved load order; commands reach only the
//! mod that claimed their root literal.
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
