use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, BlockUpdateFlags, Cx, Drops, Kinds, SimulationVolume, neighbour,
};
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::component::{Connection, Kind};
use crate::memory::{HasRedstoneMemory, HasRedstoneSnapshot};
use crate::power::{MAX_SIGNAL, WireSignals, best_neighbour_signal, neighbour_state, state_at};
use crate::rules::{RedstoneRules, WireEvaluator};
use crate::table::RedstoneTable;
use crate::wire_network;

pub(crate) const LIMIT: u16 = 512;

fn is_wire(table: &RedstoneTable, state: BlockStateId) -> bool {
    table.kind(state) == Some(Kind::Wire)
}

#[must_use]
pub fn is_dot(table: &RedstoneTable, state: BlockStateId) -> bool {
    Direction::HORIZONTAL
        .into_iter()
        .all(|side| !table.connection(state, side).is_connected())
}

#[must_use]
pub fn is_cross(table: &RedstoneTable, state: BlockStateId) -> bool {
    Direction::HORIZONTAL
        .into_iter()
        .all(|side| table.connection(state, side) == Connection::Side)
}

fn accepts_connection(
    table: &RedstoneTable,
    rules: &RedstoneRules,
    state: BlockStateId,
    from: Direction,
) -> bool {
    let info = table.info(state);
    match info.kind {
        Some(Kind::Wire | Kind::Comparator) => true,
        Some(Kind::Repeater) => table
            .facing(state)
            .is_some_and(|facing| facing.axis() == from.axis()),
        Some(Kind::Observer) => table
            .facing(state)
            .is_some_and(|facing| facing.opposite() == from),
        Some(Kind::Piston { .. }) => rules.dust_connects_to_pistons,
        _ => info.signal_source,
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Climb {
    Allowed,
    Blocked,
}

fn connecting_side<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    rules: &RedstoneRules,
    pos: BlockPos,
    direction: Direction,
    climb: Climb,
) -> Connection
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    let Some((next, state)) = neighbour_state(cx, pos, direction) else {
        return Connection::None;
    };
    if climb == Climb::Allowed
        && table.holds_wire(state)
        && neighbour_state(cx, next, Direction::Up).is_some_and(|(_, above)| is_wire(table, above))
    {
        return if table.full_face(state, direction.opposite()) {
            Connection::Up
        } else {
            Connection::Side
        };
    }
    if accepts_connection(table, rules, state, direction.opposite()) {
        return Connection::Side;
    }
    if !table.is_conductor(state)
        && neighbour_state(cx, next, Direction::Down)
            .is_some_and(|(_, below)| is_wire(table, below))
    {
        return Connection::Side;
    }
    Connection::None
}

fn straighten(mut sides: [Connection; 4]) -> [Connection; 4] {
    let connected = |connection: Connection| connection.is_connected();
    let [north, east, south, west] = sides;
    let no_z = !connected(north) && !connected(south);
    let no_x = !connected(east) && !connected(west);
    if no_z {
        if !connected(west) {
            sides[3] = Connection::Side;
        }
        if !connected(east) {
            sides[1] = Connection::Side;
        }
    }
    if no_x {
        if !connected(north) {
            sides[0] = Connection::Side;
        }
        if !connected(south) {
            sides[2] = Connection::Side;
        }
    }
    sides
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum KeepDot {
    Yes,
    No,
}

pub(crate) fn connection_state<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    rules: &RedstoneRules,
    pos: BlockPos,
    state: BlockStateId,
    keep_dot: KeepDot,
) -> BlockStateId
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    let climb = match neighbour_state(cx, pos, Direction::Up) {
        Some((_, above)) if table.is_conductor(above) => Climb::Blocked,
        _ => Climb::Allowed,
    };
    let sides = Direction::HORIZONTAL
        .map(|direction| connecting_side(cx, table, rules, pos, direction, climb));
    let none = sides.iter().all(|side| !side.is_connected());
    if keep_dot == KeepDot::Yes && none {
        return table
            .with_connections(state, [Connection::None; 4])
            .unwrap_or(state);
    }
    table
        .with_connections(state, straighten(sides))
        .unwrap_or(state)
}

pub(crate) fn target_power<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
) -> u8
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    let direct = best_neighbour_signal(cx, table, pos, WireSignals::Ignore);
    if direct == MAX_SIGNAL {
        return MAX_SIGNAL;
    }
    let above_conductor =
        neighbour_state(cx, pos, Direction::Up).is_some_and(|(_, above)| table.is_conductor(above));
    let mut wires = 0_u8;
    let mut read = |cx: &mut Cx<'_, K, V, F, B>, at: Option<BlockPos>| {
        let Some(at) = at else {
            return;
        };
        if let Some(state) = state_at(cx, at)
            && is_wire(table, state)
        {
            wires = wires.max(table.power(state));
        }
    };
    for direction in Direction::HORIZONTAL {
        let Some((next, state)) = neighbour_state(cx, pos, direction) else {
            continue;
        };
        read(cx, Some(next));
        if table.is_conductor(state) {
            if !above_conductor {
                read(cx, neighbour(next, Direction::Up));
            }
        } else {
            read(cx, neighbour(next, Direction::Down));
        }
    }
    direct.max(wires.saturating_sub(1))
}

pub(crate) fn can_survive<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
) -> bool
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    neighbour_state(cx, pos, Direction::Down).is_some_and(|(_, below)| table.holds_wire(below))
}

pub(crate) fn update_power<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    rules: &RedstoneRules,
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
    let target = target_power(cx, table, pos);
    if table.power(state) == target {
        return;
    }
    if rules.wire_evaluator == WireEvaluator::Experimental
        && wire_network::evaluate(cx, table, pos, state)
    {
        return;
    }
    if state_at(cx, pos) == Some(state)
        && let Some(next) = table.with_power(state, target)
    {
        cx.set(pos, next, BlockUpdateFlags::CLIENTS, LIMIT);
    }
    let wire = Block::RedstoneWire.id();
    cx.update_neighbours(pos, wire, None);
    for direction in Direction::all() {
        if let Some(next) = neighbour(pos, direction) {
            cx.update_neighbours(next, wire, None);
        }
    }
}

fn check_corner<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: Option<BlockPos>,
) where
    K: Kinds<Block = Id<Block>>,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    let Some(pos) = pos else {
        return;
    };
    if !state_at(cx, pos).is_some_and(|state| is_wire(table, state)) {
        return;
    }
    let wire = Block::RedstoneWire.id();
    cx.update_neighbours(pos, wire, None);
    for direction in Direction::all() {
        if let Some(next) = neighbour(pos, direction) {
            cx.update_neighbours(next, wire, None);
        }
    }
}

fn update_neighbouring_wires<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
) where
    K: Kinds<Block = Id<Block>>,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    for direction in Direction::HORIZONTAL {
        check_corner(cx, table, neighbour(pos, direction));
    }
    for direction in Direction::HORIZONTAL {
        let Some((next, state)) = neighbour_state(cx, pos, direction) else {
            continue;
        };
        let vertical = if table.is_conductor(state) {
            Direction::Up
        } else {
            Direction::Down
        };
        check_corner(cx, table, neighbour(next, vertical));
    }
}

pub(crate) fn placed<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    rules: &RedstoneRules,
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
    if is_wire(table, old) {
        return;
    }
    let connected = connection_state(cx, table, rules, pos, state, KeepDot::No);
    let state = if connected == state {
        state
    } else {
        cx.set(pos, connected, BlockUpdateFlags::CLIENTS, LIMIT);
        connected
    };
    update_power(cx, table, rules, pos, state);
    let wire = Block::RedstoneWire.id();
    for direction in [Direction::Down, Direction::Up] {
        if let Some(next) = neighbour(pos, direction) {
            cx.update_neighbours(next, wire, None);
        }
    }
    update_neighbouring_wires(cx, table, pos);
}

pub(crate) fn removed<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
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
    let wire = Block::RedstoneWire.id();
    for direction in Direction::all() {
        if let Some(next) = neighbour(pos, direction) {
            cx.update_neighbours(next, wire, None);
        }
    }
    update_neighbouring_wires(cx, table, pos);
}

pub(crate) fn neighbour_changed<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    rules: &RedstoneRules,
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
    if can_survive(cx, table, pos) {
        update_power(cx, table, rules, pos, state);
    } else {
        cx.destroy(pos, Drops::Released, LIMIT);
    }
}

pub(crate) fn update_shape<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    rules: &RedstoneRules,
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
{
    match from {
        Direction::Down => {
            if table.holds_wire(neighbour_state) {
                state
            } else {
                table.air()
            }
        }
        Direction::Up => {
            let keep = if is_dot(table, state) {
                KeepDot::Yes
            } else {
                KeepDot::No
            };
            connection_state(cx, table, rules, pos, state, keep)
        }
        horizontal => {
            let climb = match neighbour_state_above(cx, table, pos) {
                true => Climb::Blocked,
                false => Climb::Allowed,
            };
            let side = connecting_side(cx, table, rules, pos, horizontal, climb);
            let current = table.connection(state, horizontal);
            if side.is_connected() == current.is_connected() && !is_cross(table, state) {
                table
                    .with_connection(state, horizontal, side)
                    .unwrap_or(state)
            } else {
                let cross = table
                    .with_connections(state, [Connection::Side; 4])
                    .and_then(|cross| table.with_connection(cross, horizontal, side))
                    .unwrap_or(state);
                connection_state(cx, table, rules, pos, cross, KeepDot::No)
            }
        }
    }
}

fn neighbour_state_above<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
) -> bool
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    neighbour_state(cx, pos, Direction::Up).is_some_and(|(_, above)| table.is_conductor(above))
}

#[must_use]
pub(crate) fn toggled_by_use(table: &RedstoneTable, state: BlockStateId) -> Option<BlockStateId> {
    if is_cross(table, state) {
        table.with_connections(state, [Connection::None; 4])
    } else if is_dot(table, state) {
        table.with_connections(state, [Connection::Side; 4])
    } else {
        None
    }
}
