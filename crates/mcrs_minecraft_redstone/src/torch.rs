use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, BlockUpdateFlags, Cx, Kinds, SimulationVolume, TickPriority,
    neighbour,
};
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::memory::{HasRedstoneMemory, HasRedstoneSnapshot, ToggleLog};
use crate::power::{attached_of, has_signal_from};
use crate::table::RedstoneTable;
use crate::wire::LIMIT;

pub(crate) const TOGGLE_DELAY: i32 = 2;
pub(crate) const BURNOUT_DELAY: i32 = 160;

fn must_be_off<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
    state: BlockStateId,
) -> bool
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    attached_of(table, state).is_some_and(|attached| has_signal_from(cx, table, pos, attached))
}

fn update_all_neighbours<K, V, F, B>(cx: &mut Cx<'_, K, V, F, B>, pos: BlockPos, block: Id<Block>)
where
    K: Kinds<Block = Id<Block>>,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    for direction in Direction::all() {
        if let Some(next) = neighbour(pos, direction) {
            cx.update_neighbours(next, block, None);
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
{
    let _ = old;
    update_all_neighbours(cx, pos, table.block_of(state));
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
    if !moved_by_piston {
        update_all_neighbours(cx, pos, table.block_of(old));
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
    let block = table.block_of(state);
    let lit = table.lit(state);
    if lit != must_be_off(cx, table, pos, state) || cx.has_scheduled_block_tick(pos, block) {
        return;
    }
    let now = cx.now();
    let burnt = !lit
        && cx
            .memory()
            .is_some_and(|memory| memory.redstone().burns_out(pos, now, ToggleLog::OnlyCount));
    if !burnt {
        cx.schedule_block_tick(pos, block, TOGGLE_DELAY, TickPriority::Normal);
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
    let off = must_be_off(cx, table, pos, state);
    let now = cx.now();
    let block = table.block_of(state);
    if table.lit(state) {
        if !off {
            return;
        }
        let burnt = cx
            .memory()
            .is_some_and(|memory| memory.redstone().burns_out(pos, now, ToggleLog::Record));
        if let Some(dark) = table.with_lit(state, false) {
            cx.set(pos, dark, BlockUpdateFlags::ALL, LIMIT);
        }
        if burnt {
            cx.schedule_block_tick(pos, block, BURNOUT_DELAY, TickPriority::Normal);
        }
    } else if !off {
        let burnt = cx
            .memory()
            .is_some_and(|memory| memory.redstone().burns_out(pos, now, ToggleLog::OnlyCount));
        if !burnt && let Some(lit) = table.with_lit(state, true) {
            cx.set(pos, lit, BlockUpdateFlags::ALL, LIMIT);
        }
    }
}

#[must_use]
pub(crate) fn update_shape(
    table: &RedstoneTable,
    state: BlockStateId,
    from: Direction,
    neighbour_state: BlockStateId,
) -> BlockStateId {
    let Some(attached) = attached_of(table, state) else {
        return state;
    };
    if attached != from {
        return state;
    }
    let supported = match attached {
        Direction::Down => table.holds_wire(neighbour_state),
        other => table.full_face(neighbour_state, other.opposite()),
    };
    if supported { state } else { table.air() }
}
