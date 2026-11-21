use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, Cx, Kinds, SimulationVolume, neighbour,
};
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::component::{Kind, left_of, right_of};
use crate::memory::{HasRedstoneMemory, HasRedstoneSnapshot};
use crate::power::{
    MAX_SIGNAL, WireSignals, comparator_output, neighbour_state, signal_toward, strong_output,
};
use crate::table::RedstoneTable;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum LateralInputs {
    Diodes,
    DiodesWiresAndBlocks,
}

pub(crate) fn input_signal<K, V, F, B>(
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
    B::Shared: HasRedstoneSnapshot,
{
    let Some((input, state)) = neighbour_state(cx, pos, facing) else {
        return 0;
    };
    match table.kind(state) {
        Some(Kind::Wire) => table.power(state),
        Some(Kind::RedstoneBlock) => MAX_SIGNAL,
        _ => signal_toward(cx, table, input, facing.opposite(), WireSignals::Count),
    }
}

fn lateral_input<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
    side: Direction,
    inputs: LateralInputs,
) -> u8
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    let Some((at, state)) = neighbour_state(cx, pos, side) else {
        return 0;
    };
    match table.kind(state) {
        Some(Kind::Repeater) => strong_output(table, state, side.opposite(), 0),
        Some(Kind::Comparator) => {
            let output = comparator_output(cx, at);
            strong_output(table, state, side.opposite(), output)
        }
        Some(Kind::Wire) if inputs == LateralInputs::DiodesWiresAndBlocks => table.power(state),
        Some(Kind::RedstoneBlock) if inputs == LateralInputs::DiodesWiresAndBlocks => MAX_SIGNAL,
        _ => 0,
    }
}

pub(crate) fn lateral_signal<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
    facing: Direction,
    inputs: LateralInputs,
) -> u8
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    let left = lateral_input(cx, table, pos, left_of(facing), inputs);
    let right = lateral_input(cx, table, pos, right_of(facing), inputs);
    left.max(right)
}

pub(crate) fn feeds_a_diode<K, V, F, B>(
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
{
    let output_side = facing.opposite();
    let Some((_, front)) = neighbour_state(cx, pos, output_side) else {
        return false;
    };
    matches!(table.kind(front), Some(Kind::Repeater | Kind::Comparator))
        && table.facing(front) != Some(output_side)
}

pub(crate) fn update_front<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    pos: BlockPos,
    facing: Direction,
    block: Id<Block>,
) where
    K: Kinds<Block = Id<Block>>,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    let Some(front) = neighbour(pos, facing.opposite()) else {
        return;
    };
    cx.update_neighbour(front, block);
    cx.update_neighbours(front, block, Some(facing));
}

#[must_use]
pub(crate) fn survives_on(table: &RedstoneTable, below: BlockStateId) -> bool {
    table.rigid_top(below)
}
