use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_core::{Axis, Direction};
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, BlockUpdateFlags, Cx, Kinds, SimulationVolume, neighbour,
};
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::component::RailShape;
use crate::memory::{HasRedstoneMemory, HasRedstoneSnapshot};
use crate::power::{is_powered, state_at};
use crate::table::RedstoneTable;
use crate::wire::LIMIT;

pub(crate) const CHAIN_LIMIT: u8 = 8;
pub(crate) const REACH: u8 = 10;

fn axis_directions(axis: Axis) -> Option<[Direction; 2]> {
    match axis {
        Axis::X => Some([Direction::East, Direction::West]),
        Axis::Z => Some([Direction::North, Direction::South]),
        Axis::Y => None,
    }
}

fn same_rail(table: &RedstoneTable, state: BlockStateId, other: BlockStateId) -> bool {
    table.block_of(state) == table.block_of(other)
}

fn next_in_line<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    rail: BlockStateId,
    pos: BlockPos,
    shape: RailShape,
    direction: Direction,
) -> Option<(BlockPos, BlockStateId)>
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    let axis = shape.axis()?;
    if axis != direction.axis() {
        return None;
    }
    let ahead = neighbour(pos, direction)?;
    let mut candidates = Vec::with_capacity(3);
    if shape == RailShape::Ascending(direction) {
        candidates.push(neighbour(ahead, Direction::Up));
    } else {
        candidates.push(Some(ahead));
        candidates.push(neighbour(ahead, Direction::Down));
    }
    for candidate in candidates.into_iter().flatten() {
        let Some(state) = state_at(cx, candidate) else {
            continue;
        };
        if !same_rail(table, rail, state) {
            continue;
        }
        let Some(next_shape) = table.rail_shape(state) else {
            continue;
        };
        if next_shape.axis() == Some(axis) {
            return Some((candidate, state));
        }
    }
    None
}

fn chain_is_powered<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    rail: BlockStateId,
    pos: BlockPos,
    shape: RailShape,
    direction: Direction,
    distance: u8,
) -> bool
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    if distance >= CHAIN_LIMIT {
        return false;
    }
    let Some((next, next_state)) = next_in_line(cx, table, rail, pos, shape, direction) else {
        return false;
    };
    if !table.powered(next_state) {
        return false;
    }
    let Some(next_shape) = table.rail_shape(next_state) else {
        return false;
    };
    is_powered(cx, table, next)
        || chain_is_powered(
            cx,
            table,
            rail,
            next,
            next_shape,
            direction,
            distance.saturating_add(1),
        )
}

pub(crate) fn rail_is_powered<K, V, F, B>(
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
    if is_powered(cx, table, pos) {
        return true;
    }
    let Some(shape) = table.rail_shape(state) else {
        return false;
    };
    let Some(directions) = shape.axis().and_then(axis_directions) else {
        return false;
    };
    directions
        .into_iter()
        .any(|direction| chain_is_powered(cx, table, state, pos, shape, direction, 0))
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
    let powered = rail_is_powered(cx, table, pos, state);
    if powered == table.powered(state) {
        return;
    }
    if let Some(next) = table.with_powered(state, powered) {
        cx.set(pos, next, BlockUpdateFlags::ALL, LIMIT);
        let block = table.block_of(state);
        if let Some(below) = neighbour(pos, Direction::Down) {
            cx.update_neighbours(below, block, None);
        }
        if let Some(shape) = table.rail_shape(state)
            && let RailShape::Ascending(_) = shape
            && let Some(above) = neighbour(pos, Direction::Up)
        {
            cx.update_neighbours(above, block, None);
        }
    }
}
