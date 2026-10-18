#![forbid(unsafe_code)]

//! Content and audio declarations, deterministic pack generation, ordered tag/resource
//! merging, and client render-stage contribution ordering.

mod audio;
mod declare;
mod generate;
mod render_stages;
mod resources;
mod server_data;
mod tags;

pub use audio::{AudioDeclaration, AudioEntry, AudioError, AudioEvent, PlayRequest};
pub use declare::{
    BlockBehaviour, ContentBehaviour, ContentDeclaration, ContentDeclarationError, ItemBehaviour,
};
pub use generate::{GeneratedData, GeneratedFile};
pub use render_stages::{RenderStage, StageDeclaration, StageOrder, StageRefused};
pub use resources::{Override, Resource, ResourceSet};
pub use server_data::{
    GeneratedServerData, ServerDataDeclaration, ServerDataEntry, ServerDataError, ServerDataKind,
};
pub use tags::{TagError, TagId, TagSet};
