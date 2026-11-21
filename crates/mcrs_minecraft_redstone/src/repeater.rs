use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, BlockUpdateFlags, Cx, Kinds, SimulationVolume, TickPriority,
};
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::diode::{
    LateralInputs, feeds_a_diode, input_signal, lateral_signal, survives_on, update_front,
};
use crate::memory::{HasRedstoneMemory, HasRedstoneSnapshot};
use crate::table::RedstoneTable;
use crate::wire::LIMIT;

pub(crate) const MAX_DELAY: u8 = 4;

fn tick_delay(table: &RedstoneTable, state: BlockStateId) -> i32 {
    i32::from(table.delay(state)).saturating_mul(2)
}

fn is_locked<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
    facing: Direction,
) -> bool
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    lateral_signal(cx, table, pos, facing, LateralInputs::Diodes) > 0
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
    let Some(facing) = table.facing(state) else {
        return;
    };
    if is_locked(cx, table, pos, facing) {
        return;
    }
    let block = table.block_of(state);
    if cx.will_tick_this_tick(pos, block) {
        return;
    }
    let powered = table.powered(state);
    let on = input_signal(cx, table, pos, facing) > 0;
    if powered == on {
        return;
    }
    let priority = if feeds_a_diode(cx, table, pos, facing) {
        TickPriority::ExtremelyHigh
    } else if powered {
        TickPriority::VeryHigh
    } else {
        TickPriority::High
    };
    cx.schedule_block_tick(pos, block, tick_delay(table, state), priority);
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
    let Some(facing) = table.facing(state) else {
        return;
    };
    if is_locked(cx, table, pos, facing) {
        return;
    }
    let powered = table.powered(state);
    let on = input_signal(cx, table, pos, facing) > 0;
    if powered && !on {
        if let Some(next) = table.with_powered(state, false) {
            cx.set(pos, next, BlockUpdateFlags::CLIENTS, LIMIT);
        }
    } else if !powered {
        if let Some(next) = table.with_powered(state, true) {
            cx.set(pos, next, BlockUpdateFlags::CLIENTS, LIMIT);
        }
        if !on {
            cx.schedule_block_tick(
                pos,
                table.block_of(state),
                tick_delay(table, state),
                TickPriority::VeryHigh,
            );
        }
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
    let Some(facing) = table.facing(state) else {
        return;
    };
    let block = table.block_of(state);
    update_front(cx, pos, facing, block);
    if table.block_of(old) != block && input_signal(cx, table, pos, facing) > 0 {
        cx.schedule_block_tick(pos, block, 1, TickPriority::Normal);
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
    if moved_by_piston {
        return;
    }
    if let Some(facing) = table.facing(old) {
        update_front(cx, pos, facing, table.block_of(old));
    }
}

pub(crate) fn update_shape<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
    state: BlockStateId,
    from: Direction,
    neighbour_state: BlockStateId,
) -> BlockStateId
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    match from {
        Direction::Down => {
            if survives_on(table, neighbour_state) {
                state
            } else {
                table.air()
            }
        }
        Direction::Up => state,
        _ => {
            let Some(facing) = table.facing(state) else {
                return state;
            };
            let locked = is_locked(cx, table, pos, facing);
            table.with_locked(state, locked).unwrap_or(state)
        }
    }
}

#[must_use]
pub(crate) fn toggled_by_use(table: &RedstoneTable, state: BlockStateId) -> Option<BlockStateId> {
    let delay = table.delay(state);
    let next = if delay >= MAX_DELAY {
        1
    } else {
        delay.saturating_add(1)
    };
    table.with_delay(state, next)
}
