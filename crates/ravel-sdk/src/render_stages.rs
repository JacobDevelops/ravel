//! Render-stage declarations and contributor ordering.
//!
//! Resolution refuses unknown stages and cycles. No mod drawing path is
//! implemented; these declarations currently describe intended contributions.
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
//! stages.contribute(&glow, RenderStage::AfterTerrain);
//! stages.contribute(&outline, RenderStage::AfterTerrain);
//! // The outline goes over the glow, so it is entered after it.
//! stages.after(&outline, &glow);
//!
//! let order = stages.resolve().expect("a stage without a cycle resolves");
//! assert_eq!(order.at(RenderStage::AfterTerrain), [glow.clone(), outline]);
//!
//! // A stage nothing publishes is refused where it is written.
//! assert!(RenderStage::published("after_everything").is_none());
//!
//! // Drawing is the client's alone; a server never reaches a stage.
//! assert!(RenderStage::AfterTerrain.happens_on(Side::Client));
//! assert!(!RenderStage::AfterTerrain.happens_on(Side::DedicatedServer));
//! # Ok::<(), ravel_sdk::ModIdError>(())
//! ```

pub use ravel_data::{RenderStage, StageDeclaration, StageOrder, StageRefused};
