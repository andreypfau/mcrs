use bevy_app::{App, Plugin};

pub mod air;
pub mod stone;
pub mod tnt;
use mcrs_protocol::BlockStateId;

use crate::world::block::Block;
pub use air::BLOCK as AIR;
pub use stone::BLOCK as STONE;
pub use tnt::BLOCK as TNT;

pub struct MinecraftBlockPlugin;

impl Plugin for MinecraftBlockPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(tnt::TntBlockPlugin);
    }
}

const STATE_TABLE_LEN: usize = 1 << 16;

static STATE_TO_BLOCK: [Option<&'static Block>; STATE_TABLE_LEN] = {
    const fn build() -> [Option<&'static Block>; STATE_TABLE_LEN] {
        let mut t: [Option<&'static Block>; STATE_TABLE_LEN] = [None; STATE_TABLE_LEN];
        t[air::DEFAULT_STATE.id.0 as usize] = Some(&AIR);
        t[stone::DEFAULT_STATE.id.0 as usize] = Some(&STONE);
        t[tnt::UNSTABLE_STATE.id.0 as usize] = Some(&TNT);
        t[tnt::DEFAULT_STATE.id.0 as usize] = Some(&TNT);
        t
    }
    build()
};

impl TryFrom<BlockStateId> for &'static Block {
    type Error = ();

    #[inline]
    fn try_from(v: BlockStateId) -> Result<Self, Self::Error> {
        STATE_TO_BLOCK.get(v.0 as usize).and_then(|x| *x).ok_or(())
    }
}

impl AsRef<Block> for BlockStateId {
    #[inline]
    fn as_ref(&self) -> &Block {
        STATE_TO_BLOCK[self.0 as usize].expect("Invalid BlockStateId")
    }
}
