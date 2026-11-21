use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, BlockUpdateFlags, Cx, Kinds, SimulationVolume,
};
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::memory::{HasRedstoneMemory, HasRedstoneSnapshot, Sensor};
use crate::power::MAX_SIGNAL;
use crate::table::RedstoneTable;
use crate::wire::LIMIT;

#[must_use]
pub fn detector_power(inverted: bool, sun: u8, sky: u8) -> u8 {
    if inverted {
        MAX_SIGNAL.saturating_sub(sky.min(MAX_SIGNAL))
    } else {
        sun.min(MAX_SIGNAL)
    }
}

pub(crate) fn refresh<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
    state: BlockStateId,
) where
    K: Kinds<Block = Id<Block>>,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    let Some((sun, sky)) = cx.shared().redstone().daylight(pos) else {
        return;
    };
    let power = detector_power(table.inverted(state), sun, sky);
    if power != table.power(state)
        && let Some(next) = table.with_power(state, power)
    {
        cx.set(pos, next, BlockUpdateFlags::ALL, LIMIT);
    }
}

pub(crate) fn placed<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
    state: BlockStateId,
    old: BlockStateId,
) where
    K: Kinds<Block = Id<Block>>,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    if table.block_of(old) != table.block_of(state)
        && let Some(memory) = cx.memory()
    {
        memory.redstone().sensors.insert(pos, Sensor::Daylight);
    }
    refresh(cx, table, pos, state);
}

pub(crate) fn removed<K, V, F, B>(cx: &mut Cx<'_, K, V, F, B>, pos: BlockPos)
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
{
    if let Some(memory) = cx.memory() {
        memory.redstone().sensors.remove(&pos);
    }
}
