pub mod block;
pub mod block_light;
pub mod epoch;
pub mod field;
pub mod level;
pub mod queue;
pub mod region;
pub mod relax;
pub mod world;

use mcrs_minecraft_chunk::VoxelPalette;
use mcrs_voxel::BlockStateId;
use mcrs_voxel::SectionPos;

pub use relax::relax;

pub type SectionBlocks = VoxelPalette<BlockStateId, { SectionPos::SIZE }>;

pub mod prelude {
    pub use crate::SectionBlocks;
    pub use crate::block::{LightProperties, LightRegistry, SpecialBlocks};
    pub use crate::epoch::{EpochStats, LightJob, LightUpdate, PublishedLight, SectionLight};
    pub use crate::level::{BlockColumn, LightBounds, LightLevel};
    pub use crate::queue::{DEFAULT_PRIORITY, LightQueue, Priority};
    pub use crate::region::Influence;
    pub use crate::world::{ColumnSurface, Edit, LightWorld, Section};
}
