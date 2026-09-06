//! Light is the least fixed point of
//!
//! ```text
//! L(p) = max( E(p),  max over face neighbours q of ( L(q) - A(q -> p) ) )
//! ```
//!
//! where `A` is infinite when the two facing shapes occlude each other and
//! `max(1, dampening(p))` otherwise. Three things follow:
//!
//! - the map is monotone, so a rising relaxation reaches the same answer in any
//!   order — hence no locks, no halo exchange and no barriers in [`relax`];
//! - every step costs at least one level, so a change cannot travel further
//!   than fifteen cells and the work can be confined to a box;
//! - lowering light is not something relaxation can do, so an edit erases its
//!   box and fills it again rather than running a second, opposite algorithm.

pub mod block;
pub mod epoch;
pub mod field;
pub mod level;
pub mod plugin;
pub mod queue;
pub mod region;
pub mod relax;
pub mod storage;
pub mod world;

use bevy_ecs::prelude::Component;
use mcrs_voxel_math::chunk_pos::BLOCKS;
use mcrs_voxel_storage::{VoxelId, VoxelPalette};

use crate::storage::LightStorage;

pub use relax::relax;

pub type SectionBlocks = VoxelPalette<VoxelId, { BLOCKS::SIZE }>;

#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct BlockLight(pub LightStorage);

#[derive(Component, Clone, Debug, Default, PartialEq)]
pub struct SkyLight(pub LightStorage);

pub mod prelude {
    pub use crate::block::{Layer, LightProperties, LightRegistry, SpecialBlocks};
    pub use crate::epoch::{
        EpochStats, EpochTimings, LightJob, LightUpdate, PublishedLight, SectionLight,
    };
    pub use crate::level::{BlockColumn, LightBounds, LightLevel, LocalPos};
    pub use crate::plugin::{
        IntakeBudget, LightBudget, LightEpoch, LightPlugin, LightSet, LightStatus, LightWorkQueue,
        Lighting, PendingEdits, dispatch_epoch, light_has_settled, publish_light,
    };
    pub use crate::queue::{DEFAULT_PRIORITY, LightQueue, Priority};
    pub use crate::region::{BlockBox, Influence};
    pub use crate::storage::LightStorage;
    pub use crate::world::{ColumnSurface, Edit, LightWorld, Section};
    pub use crate::{BlockLight, SectionBlocks, SkyLight};
}
