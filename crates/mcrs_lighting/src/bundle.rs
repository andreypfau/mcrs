use crate::components::{
    BlockEgress, BlockIncoming, BlockLight, BlockLightWorkspace, BlockPendingEgress, SkyEgress,
    SkyIncoming, SkyLight, SkyLightWorkspace, SkyPendingEgress,
};
use crate::nibble::NibbleArray;
use crate::storage::LightStorage;
use bevy_ecs::prelude::Bundle;

#[derive(Bundle)]
pub struct BlockLightBundle {
    pub light: BlockLight,
    pub egress: BlockEgress,
    pub incoming: BlockIncoming,
    pub workspace: BlockLightWorkspace,
    pub pending_egress: BlockPendingEgress,
}

// `LightStorage::set` promotes `Null -> Uniform(v)` on the first non-zero write,
// which blanket-fills every cell with `v` and breaks per-cell BFS propagation.
// Sections that participate in block-light propagation start with explicit
// `Mixed(zeros)` so per-cell writes stay independent. An idle-time compaction
// pass will revisit empty sections later.
impl Default for BlockLightBundle {
    fn default() -> Self {
        Self {
            light: BlockLight(LightStorage::Mixed(Box::new(NibbleArray::zeros()))),
            egress: BlockEgress::default(),
            incoming: BlockIncoming::default(),
            workspace: BlockLightWorkspace::default(),
            pending_egress: BlockPendingEgress::default(),
        }
    }
}

#[derive(Bundle)]
pub struct SkyLightBundle {
    pub light: SkyLight,
    pub egress: SkyEgress,
    pub incoming: SkyIncoming,
    pub workspace: SkyLightWorkspace,
    pub pending_egress: SkyPendingEgress,
}

// Sky-light propagation shares the same Null->Uniform-on-first-write hazard
// described above for BlockLightBundle. Without explicit `Mixed(zeros)` the
// first top-face seed at level 15 promotes storage to `Uniform(15)`, which
// then reports 15 for every cell and short-circuits per-cell BFS
// attenuation through partial-air sections (e.g. one with a water cell).
// The column-walker fast path in `propagate_increase_sky_system` writes
// `Uniform(15)` directly when the section is all-air, so this initial Mixed
// state only matters for the BFS path.
impl Default for SkyLightBundle {
    fn default() -> Self {
        Self {
            light: SkyLight(LightStorage::Mixed(Box::new(NibbleArray::zeros()))),
            egress: SkyEgress::default(),
            incoming: SkyIncoming::default(),
            workspace: SkyLightWorkspace::default(),
            pending_egress: SkyPendingEgress::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn block_light_bundle_default_compiles() {
        let _bundle = BlockLightBundle::default();
    }

    #[test]
    fn sky_light_bundle_default_compiles() {
        let _bundle = SkyLightBundle::default();
    }
}
