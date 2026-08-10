//! Where a mod's drawing goes, and in what order among other mods.
//!
//! Two mods adding to one part of the frame is the same question
//! [`crate::services`] answers for named calls between mods, and drawing had
//! none of it: whichever loaded first drew first, and neither could say
//! otherwise. A mod declares the published stages it contributes to and the mods
//! it wants to follow, and Ravel decides the order once for the set — refusing a
//! cycle and a stage nothing publishes rather than resolving either quietly.
//!
//! # What this does not give a mod
//!
//! **A way to draw.** Nothing draws at any stage today and this module ships no
//! drawing path. `docs/decisions/adg-6-configuration-and-screens.md` settled
//! that the screens Ravel owns are declared in data and drawn in Java, and
//! nothing here extends that to a mod's own drawing; [`crate::sides`] says the
//! same from the other direction, that a mod's client-only state is what it
//! knows on the client rather than what it puts on the display. What ships is
//! the stage name, the ordering and the two refusals, which are decidable
//! without a frame. A mod that declares a contribution today is saying where its
//! drawing would go.
//!
//! # Examples
//!
//! ```
//! use ravel_sdk::render_stages::{RenderStage, StageDeclaration};
//! use ravel_sdk::{ModId, Side};
//!
//! let glow: ModId = "glow_mod".parse()?;
//! let outline: ModId = "outline_mod".parse()?;
//!
//! let mut stages = StageDeclaration::default();
//! stages.contribute(&glow, "after_terrain");
//! stages.contribute(&outline, "after_terrain");
//! // The outline goes over the glow, so it is entered after it.
//! stages.after(&outline, &glow);
//!
//! let order = stages.resolve().expect("a stage without a cycle resolves");
//! assert_eq!(order.at("after_terrain"), [glow.clone(), outline]);
//!
//! // A stage nothing publishes is refused where it is written.
//! assert!(stages.contribute_checked(&glow, "after_everything").is_err());
//!
//! // Drawing is the client's alone; a server never reaches a stage.
//! assert!(RenderStage::AfterTerrain.happens_on(Side::Client));
//! assert!(!RenderStage::AfterTerrain.happens_on(Side::DedicatedServer));
//! # Ok::<(), ravel_sdk::ModIdError>(())
//! ```

pub use ravel_data::{RenderStage, StageDeclaration, StageOrder, StageRefused};
