use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, BlockUpdateFlags, Cx, Kinds, SimulationVolume, TickPriority,
};
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::memory::{HasRedstoneMemory, HasRedstoneSnapshot};
use crate::power::is_powered;
use crate::table::RedstoneTable;
use crate::wire::LIMIT;

pub(crate) const OFF_DELAY: i32 = 4;

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
    if table.block_of(old) == table.block_of(state) {
        return;
    }
    let powered = is_powered(cx, table, pos);
    if powered != table.lit(state)
        && let Some(next) = table.with_lit(state, powered)
    {
        cx.set(pos, next, BlockUpdateFlags::CLIENTS, LIMIT);
    }
}

pub(crate) fn neighbour_changed<K, V, F, B>(
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
    let lit = table.lit(state);
    let powered = is_powered(cx, table, pos);
    if lit == powered {
        return;
    }
    if lit {
        let block = table.block_of(state);
        if !cx.has_scheduled_block_tick(pos, block) {
            cx.schedule_block_tick(pos, block, OFF_DELAY, TickPriority::Normal);
        }
    } else if let Some(next) = table.with_lit(state, true) {
        cx.set(pos, next, BlockUpdateFlags::CLIENTS, LIMIT);
    }
}

pub(crate) fn block_tick<K, V, F, B>(
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
    if table.lit(state)
        && !is_powered(cx, table, pos)
        && let Some(next) = table.with_lit(state, false)
    {
        cx.set(pos, next, BlockUpdateFlags::CLIENTS, LIMIT);
    }
}
