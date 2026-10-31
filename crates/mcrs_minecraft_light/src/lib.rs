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
pub mod block_light;
pub mod epoch;
pub mod field;
pub mod level;
#[cfg(feature = "bevy")]
pub mod plugin;
pub mod queue;
pub mod region;
pub mod relax;
pub mod world;

use mcrs_minecraft_chunk::VoxelPalette;
use mcrs_minecraft_core::BlockStateId;
use mcrs_minecraft_core::SectionPos;

pub use relax::relax;

pub type SectionBlocks = VoxelPalette<BlockStateId, { SectionPos::SIZE }>;

pub mod prelude {
    pub use crate::SectionBlocks;
    pub use crate::block::{LightProperties, LightRegistry, SpecialBlocks};
    pub use crate::epoch::{EpochStats, LightJob, LightUpdate, PublishedLight, SectionLight};
    pub use crate::level::{BlockColumn, LightBounds, LightLevel};
    #[cfg(feature = "bevy")]
    pub use crate::plugin::{
        IntakeBudget, LightBudget, LightEpoch, LightPlugin, LightSet, LightStatus, LightWorkQueue,
        Lighting, PendingEdits, SectionRelit, dispatch_epoch, publish_light,
    };
    pub use crate::queue::{DEFAULT_PRIORITY, LightQueue, Priority};
    pub use crate::region::Influence;
    pub use crate::world::{ColumnSurface, Edit, LightWorld, Section};
}
