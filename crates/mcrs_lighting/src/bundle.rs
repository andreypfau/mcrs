use crate::components::{
    BlockEgress, BlockIncoming, BlockLight, BlockLightWorkspace, SkyEgress, SkyIncoming, SkyLight,
    SkyLightWorkspace,
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
        }
    }
}

#[derive(Bundle, Default)]
pub struct SkyLightBundle {
    pub light: SkyLight,
    pub egress: SkyEgress,
    pub incoming: SkyIncoming,
    pub workspace: SkyLightWorkspace,
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
