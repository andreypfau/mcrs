//! Backwards-compatibility re-exports. The actual component types live under
//! `crate::common::components`, `crate::block_light::components`, and
//! `crate::sky_light::components`. This shim preserves the historical
//! `mcrs_minecraft_lighting::components::*` import path used by downstream
//! consumers.

pub use crate::block_light::components::{
    BlockBfsPending, BlockEgress, BlockIncoming, BlockLight, BlockLightWorkspace,
    BlockNeedsInitialSeed, BlockPendingEgress,
};
pub use crate::common::components::{IsAllAir, NeedsFullReseed, Wavefront};
pub(crate) use crate::sky_light::components::NeedsRetop;
pub use crate::sky_light::components::{
    SkyBfsPending, SkyEgress, SkyIncoming, SkyLight, SkyLightSeededAsTopmost, SkyLightWorkspace,
    SkyNeedsInitialSeed, SkyPendingEgress,
};
