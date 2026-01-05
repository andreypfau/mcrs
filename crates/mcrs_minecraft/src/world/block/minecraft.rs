use bevy_app::{App, Plugin};

pub mod air;
pub mod bedrock;
pub mod dirt;
pub mod grass_block;
pub mod stone;
pub mod tnt;

use mcrs_protocol::{BlockStateId, Ident};

use crate::world::block::Block;
pub use air::BLOCK as AIR;
pub use bedrock::BLOCK as BEDROCK;
pub use dirt::BLOCK as DIRT;
pub use grass_block::BLOCK as GRASS_BLOCK;
use mcrs_registry::Registry;
pub use stone::BLOCK as STONE;
pub use tnt::BLOCK as TNT;

pub struct MinecraftBlockPlugin;

impl Plugin for MinecraftBlockPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(tnt::TntBlockPlugin);

        let mut registry = Registry::<&'static Block>::default();
        registry.insert(AIR.identifier, &AIR);
        registry.insert(GRASS_BLOCK.identifier, &GRASS_BLOCK);
        registry.insert(DIRT.identifier, &DIRT);
        registry.insert(STONE.identifier, &STONE);
        registry.insert(BEDROCK.identifier, &BEDROCK);
        registry.insert(TNT.identifier, &TNT);

        app.insert_resource(registry);
    }
}

const STATE_TABLE_LEN: usize = 1 << 16;

static STATE_TO_BLOCK: [Option<&'static Block>; STATE_TABLE_LEN] = {
    let mut t: [Option<&'static Block>; STATE_TABLE_LEN] = [None; STATE_TABLE_LEN];
    t[air::DEFAULT_STATE.id.0 as usize] = Some(&AIR);
    t[grass_block::DEFAULT_STATE.id.0 as usize] = Some(&GRASS_BLOCK);
    t[dirt::DEFAULT_STATE.id.0 as usize] = Some(&DIRT);
    t[stone::DEFAULT_STATE.id.0 as usize] = Some(&STONE);
    t[bedrock::DEFAULT_STATE.id.0 as usize] = Some(&BEDROCK);
    t[tnt::UNSTABLE_STATE.id.0 as usize] = Some(&TNT);
    t[tnt::DEFAULT_STATE.id.0 as usize] = Some(&TNT);
    t
};

impl TryFrom<BlockStateId> for &'static Block {
    type Error = ();

    #[inline]
    fn try_from(v: BlockStateId) -> Result<Self, Self::Error> {
        STATE_TO_BLOCK.get(v.0 as usize).and_then(|x| *x).ok_or(())
    }
}

impl TryFrom<Ident<String>> for &'static Block {
    type Error = ();

    fn try_from(value: Ident<String>) -> Result<Self, Self::Error> {
        STATE_TO_BLOCK
            .iter()
            .find(|block_opt| {
                if let Some(block) = block_opt {
                    block.identifier.as_str() == value.as_str()
                } else {
                    false
                }
            })
            .and_then(|block_opt| *block_opt)
            .ok_or(())
    }
}

impl AsRef<Block> for BlockStateId {
    #[inline]
    fn as_ref(&self) -> &Block {
        STATE_TO_BLOCK[self.0 as usize].expect(&format!("Invalid block state id: {}", self.0))
    }
}
