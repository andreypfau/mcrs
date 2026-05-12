use crate::components::{
    BlockEgress, BlockIncoming, BlockLight, BlockLightWorkspace, SkyEgress, SkyIncoming, SkyLight,
    SkyLightWorkspace,
};
use bevy_ecs::prelude::Bundle;

#[derive(Bundle, Default)]
pub struct BlockLightBundle {
    pub light: BlockLight,
    pub egress: BlockEgress,
    pub incoming: BlockIncoming,
    pub workspace: BlockLightWorkspace,
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
