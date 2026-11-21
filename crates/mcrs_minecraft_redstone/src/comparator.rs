use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, BlockUpdateFlags, Cx, Kinds, SimulationVolume, TickPriority,
};
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::component::DiodeMode;
use crate::diode::{
    LateralInputs, feeds_a_diode, input_signal, lateral_signal, survives_on, update_front,
};
use crate::memory::HasRedstoneMemory;
use crate::power::{MAX_SIGNAL, neighbour_state};
use crate::table::RedstoneTable;
use crate::wire::LIMIT;

pub(crate) const DELAY: i32 = 2;

fn container_at<K, V, F, B>(cx: &mut Cx<'_, K, V, F, B>, pos: BlockPos) -> Option<u8>
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
{
    cx.memory()
        .and_then(|memory| memory.redstone().container_output(pos))
}

fn frame_at<K, V, F, B>(cx: &mut Cx<'_, K, V, F, B>, pos: BlockPos, facing: Direction) -> Option<u8>
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
{
    cx.memory()
        .and_then(|memory| memory.redstone().frame_output(pos, facing))
}

fn full_input<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
    facing: Direction,
) -> u8
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
{
    let direct = input_signal(cx, table, pos, facing);
    let Some((behind, state)) = neighbour_state(cx, pos, facing) else {
        return direct;
    };
    if table.info(state).container
        && let Some(analog) = container_at(cx, behind)
    {
        return analog;
    }
    if direct < MAX_SIGNAL
        && table.is_conductor(state)
        && let Some((farther, far_state)) = neighbour_state(cx, behind, facing)
    {
        let frame = frame_at(cx, farther, facing);
        let container = if table.info(far_state).container {
            container_at(cx, farther)
        } else {
            None
        };
        if let Some(analog) = frame.into_iter().chain(container).max() {
            return analog;
        }
    }
    direct
}

pub(crate) fn output_signal<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
    state: BlockStateId,
    facing: Direction,
) -> u8
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
{
    let input = full_input(cx, table, pos, facing);
    if input == 0 {
        return 0;
    }
    let side = lateral_signal(cx, table, pos, facing, LateralInputs::DiodesWiresAndBlocks);
    if side > input {
        return 0;
    }
    match table.mode(state) {
        DiodeMode::Compare => input,
        DiodeMode::Subtract => input.saturating_sub(side),
    }
}

fn stored_output<K, V, F, B>(cx: &mut Cx<'_, K, V, F, B>, pos: BlockPos) -> u8
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
{
    cx.memory()
        .map_or(0, |memory| memory.redstone().comparator_output(pos))
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
{
    let Some(facing) = table.facing(state) else {
        return;
    };
    let block = table.block_of(state);
    if cx.will_tick_this_tick(pos, block) {
        return;
    }
    let output = output_signal(cx, table, pos, state, facing);
    let stored = stored_output(cx, pos);
    if output == stored && table.powered(state) == (output > 0) {
        return;
    }
    let priority = if feeds_a_diode(cx, table, pos, facing) {
        TickPriority::High
    } else {
        TickPriority::Normal
    };
    cx.schedule_block_tick(pos, block, DELAY, priority);
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
{
    let Some(facing) = table.facing(state) else {
        return;
    };
    let output = output_signal(cx, table, pos, state, facing);
    let stored = stored_output(cx, pos);
    if let Some(memory) = cx.memory() {
        let redstone = memory.redstone();
        redstone.comparators.insert(pos, output);
        if stored != output && !redstone.comparator_dirty.contains(&pos) {
            redstone.comparator_dirty.push(pos);
        }
    }
    if stored == output && table.mode(state) == DiodeMode::Subtract {
        return;
    }
    let powered = table.powered(state);
    let on = output > 0;
    if powered != on
        && let Some(next) = table.with_powered(state, on)
    {
        cx.set(pos, next, BlockUpdateFlags::CLIENTS, LIMIT);
    }
    update_front(cx, pos, facing, table.block_of(state));
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
{
    let Some(facing) = table.facing(state) else {
        return;
    };
    let block = table.block_of(state);
    update_front(cx, pos, facing, block);
    if table.block_of(old) != block {
        if output_signal(cx, table, pos, state, facing) > 0 {
            cx.schedule_block_tick(pos, block, 1, TickPriority::Normal);
        }
    } else if table.mode(old) != table.mode(state) {
        refresh(cx, table, pos, state);
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
    B::Memory: HasRedstoneMemory,
{
    if let Some(memory) = cx.memory() {
        memory.redstone().comparators.remove(&pos);
    }
    if moved_by_piston {
        return;
    }
    if let Some(facing) = table.facing(old) {
        update_front(cx, pos, facing, table.block_of(old));
    }
}

#[must_use]
pub(crate) fn update_shape(
    table: &RedstoneTable,
    state: BlockStateId,
    from: Direction,
    neighbour_state: BlockStateId,
) -> BlockStateId {
    if from == Direction::Down && !survives_on(table, neighbour_state) {
        table.air()
    } else {
        state
    }
}

#[must_use]
pub(crate) fn toggled_by_use(table: &RedstoneTable, state: BlockStateId) -> Option<BlockStateId> {
    table.with_mode(state, table.mode(state).other())
}
