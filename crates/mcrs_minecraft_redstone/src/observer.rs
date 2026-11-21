use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, BlockUpdateFlags, Cx, Kinds, SimulationVolume, TickPriority,
};
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::diode::update_front;
use crate::table::RedstoneTable;
use crate::wire::LIMIT;

pub(crate) const PULSE: i32 = 2;

pub(crate) fn update_shape<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
    state: BlockStateId,
    from: Direction,
) -> BlockStateId
where
    K: Kinds<Block = Id<Block>>,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    let block = table.block_of(state);
    if table.facing(state) == Some(from)
        && !table.powered(state)
        && !cx.has_scheduled_block_tick(pos, block)
    {
        cx.schedule_block_tick(pos, block, PULSE, TickPriority::Normal);
    }
    state
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
{
    let Some(facing) = table.facing(state) else {
        return;
    };
    let block = table.block_of(state);
    if table.powered(state) {
        if let Some(next) = table.with_powered(state, false) {
            cx.set(pos, next, BlockUpdateFlags::CLIENTS, LIMIT);
        }
    } else {
        if let Some(next) = table.with_powered(state, true) {
            cx.set(pos, next, BlockUpdateFlags::CLIENTS, LIMIT);
        }
        cx.schedule_block_tick(pos, block, PULSE, TickPriority::Normal);
    }
    update_front(cx, pos, facing, block);
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
{
    let block = table.block_of(state);
    if table.block_of(old) != block
        && table.powered(state)
        && !cx.has_scheduled_block_tick(pos, block)
        && let Some(next) = table.with_powered(state, false)
    {
        cx.set(
            pos,
            next,
            BlockUpdateFlags::CLIENTS | BlockUpdateFlags::KNOWN_SHAPE,
            LIMIT,
        );
    }
}

pub(crate) fn removed<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
    old: BlockStateId,
    moved_by_piston: bool,
) where
    K: Kinds<Block = Id<Block>>,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    if moved_by_piston || !table.powered(old) {
        return;
    }
    let block = table.block_of(old);
    if cx.has_scheduled_block_tick(pos, block)
        && let Some(facing) = table.facing(old)
    {
        update_front(cx, pos, facing, block);
    }
}
