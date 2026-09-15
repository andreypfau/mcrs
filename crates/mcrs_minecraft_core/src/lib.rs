/// `SharedConstants.getCurrentVersion().name()`. There is no launcher and no
/// version manifest to read it from, so the target version is stated once here.
pub const VERSION_NAME: &str = "26.3-snapshot-9";

pub mod block_pos;
pub mod bounding_box;
pub mod codec;
pub mod column_pos;
pub mod direction;
pub mod holder_set;
pub mod local_pos;
pub mod mirror;
pub mod mth;
pub mod quart_pos;
pub mod region_pos;
pub mod resource_key;
pub mod resource_location;
pub mod rotation;
pub mod section_pos;
pub mod tag_key;
pub mod value_provider;
pub mod voxel_shape;

pub use block_pos::BlockPos;
pub use bounding_box::BoundingBox;
pub use column_pos::ColumnPos;
pub use direction::{Axis, Direction, DirectionSet, dist_manhattan};
pub use holder_set::HolderSet;
pub use local_pos::LocalPos;
pub use mirror::Mirror;
pub use quart_pos::QuartPos;
pub use region_pos::RegionPos;
pub use resource_key::ResourceKey;
pub use resource_location::ResourceLocation;
pub use rotation::Rotation;
pub use section_pos::SectionPos;
pub use tag_key::{TagKey, TaggedRegistry};

// Re-export the proc macro for the rl! declarative macro.
#[doc(hidden)]
pub use mcrs_minecraft_core_macros::rl_impl as __rl_impl;
