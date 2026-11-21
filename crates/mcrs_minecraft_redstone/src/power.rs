use mcrs_minecraft_core::Direction;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, Cx, Kinds, SimulationVolume, neighbour,
};
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::component::{Kind, PlateKind, attached_direction};
use crate::memory::{HasRedstoneMemory, HasRedstoneSnapshot};
use crate::table::RedstoneTable;

pub const MAX_SIGNAL: u8 = 15;

#[must_use]
pub(crate) fn offset_by(pos: BlockPos, direction: Direction, count: i32) -> Option<BlockPos> {
    let normal = direction.normal();
    Some(BlockPos::new(
        pos.x.checked_add(normal.x.checked_mul(count)?)?,
        pos.y.checked_add(normal.y.checked_mul(count)?)?,
        pos.z.checked_add(normal.z.checked_mul(count)?)?,
    ))
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum WireSignals {
    Count,
    Ignore,
}

#[must_use]
pub(crate) fn attached_of(table: &RedstoneTable, state: BlockStateId) -> Option<Direction> {
    match table.kind(state)? {
        Kind::Lever | Kind::Button { .. } => {
            let face = table.face(state)?;
            let facing = table.facing(state)?;
            Some(attached_direction(face, facing))
        }
        Kind::Torch { wall: false } => Some(Direction::Down),
        Kind::Torch { wall: true } => Some(table.facing(state)?.opposite()),
        Kind::TripwireHook => Some(table.facing(state)?.opposite()),
        _ => None,
    }
}

#[must_use]
pub(crate) fn plate_signal(table: &RedstoneTable, state: BlockStateId) -> u8 {
    match table.kind(state) {
        Some(Kind::Plate(PlateKind::Wooden | PlateKind::Stone)) => {
            if table.powered(state) {
                MAX_SIGNAL
            } else {
                0
            }
        }
        Some(Kind::Plate(_)) => table.power(state),
        _ => 0,
    }
}

#[must_use]
pub(crate) fn output_side(table: &RedstoneTable, state: BlockStateId) -> Option<Direction> {
    Some(table.facing(state)?.opposite())
}

#[must_use]
pub(crate) fn wire_connects(table: &RedstoneTable, state: BlockStateId, toward: Direction) -> bool {
    if toward.is_vertical() {
        return false;
    }
    table.connection(state, toward).is_connected()
}

#[must_use]
pub(crate) fn output_signal(
    table: &RedstoneTable,
    state: BlockStateId,
    toward: Direction,
    comparator: u8,
) -> u8 {
    let info = table.info(state);
    let Some(kind) = info.kind else {
        return info.producer;
    };
    match kind {
        Kind::Lever | Kind::Button { .. } => {
            if table.powered(state) {
                MAX_SIGNAL
            } else {
                0
            }
        }
        Kind::Plate(_) => plate_signal(table, state),
        Kind::Torch { .. } => {
            let attached = attached_of(table, state);
            if table.lit(state) && attached != Some(toward) {
                MAX_SIGNAL
            } else {
                0
            }
        }
        Kind::RedstoneBlock => MAX_SIGNAL,
        Kind::TripwireHook => {
            if table.powered(state) {
                MAX_SIGNAL
            } else {
                0
            }
        }
        Kind::DaylightDetector => table.power(state),
        Kind::Wire => match toward {
            Direction::Down => table.power(state),
            Direction::Up => 0,
            horizontal => {
                if wire_connects(table, state, horizontal) {
                    table.power(state)
                } else {
                    0
                }
            }
        },
        Kind::Repeater | Kind::Observer => {
            if table.powered(state) && output_side(table, state) == Some(toward) {
                MAX_SIGNAL
            } else {
                0
            }
        }
        Kind::Comparator => {
            if table.powered(state) && output_side(table, state) == Some(toward) {
                comparator
            } else {
                0
            }
        }
        Kind::Target => table.power(state),
        Kind::Lamp
        | Kind::Piston { .. }
        | Kind::PistonHead
        | Kind::MovingPiston
        | Kind::Door { .. }
        | Kind::Trapdoor { .. }
        | Kind::FenceGate
        | Kind::Rail { .. }
        | Kind::NoteBlock
        | Kind::Hopper
        | Kind::Trigger(_)
        | Kind::CopperBulb
        | Kind::Tripwire => 0,
    }
}

#[must_use]
pub(crate) fn strong_output(
    table: &RedstoneTable,
    state: BlockStateId,
    into: Direction,
    comparator: u8,
) -> u8 {
    match table.kind(state) {
        Some(Kind::Lever | Kind::Button { .. }) => {
            if table.powered(state) && attached_of(table, state) == Some(into) {
                MAX_SIGNAL
            } else {
                0
            }
        }
        Some(Kind::Plate(_)) if into == Direction::Down => plate_signal(table, state),
        Some(Kind::TripwireHook)
            if table.powered(state) && attached_of(table, state) == Some(into) =>
        {
            MAX_SIGNAL
        }
        Some(Kind::Torch { .. }) if into == Direction::Up && table.lit(state) => MAX_SIGNAL,
        Some(Kind::Repeater | Kind::Comparator | Kind::Observer | Kind::Wire) => {
            output_signal(table, state, into, comparator)
        }
        _ => 0,
    }
}

pub(crate) fn comparator_output<K, V, F, B>(cx: &mut Cx<'_, K, V, F, B>, pos: BlockPos) -> u8
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    cx.memory()
        .map_or(0, |memory| memory.redstone().comparator_output(pos))
}

pub(crate) fn state_at<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    pos: BlockPos,
) -> Option<BlockStateId>
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    cx.state(pos).ok()
}

pub(crate) fn neighbour_state<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    pos: BlockPos,
    direction: Direction,
) -> Option<(BlockPos, BlockStateId)>
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    let target = neighbour(pos, direction)?;
    Some((target, state_at(cx, target)?))
}

pub(crate) fn strong_power_of<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
    wires: WireSignals,
) -> u8
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    let mut best = 0;
    for direction in Direction::all() {
        let Some((at, state)) = neighbour_state(cx, pos, direction) else {
            continue;
        };
        let kind = table.kind(state);
        if wires == WireSignals::Ignore && kind == Some(Kind::Wire) {
            continue;
        }
        let comparator = match kind {
            Some(Kind::Comparator) => comparator_output(cx, at),
            _ => 0,
        };
        best = best.max(strong_output(
            table,
            state,
            direction.opposite(),
            comparator,
        ));
        if best == MAX_SIGNAL {
            break;
        }
    }
    best
}

pub(crate) fn signal_toward<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
    toward: Direction,
    wires: WireSignals,
) -> u8
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    let Some(state) = state_at(cx, pos) else {
        return 0;
    };
    let info = table.info(state);
    if wires == WireSignals::Ignore && info.kind == Some(Kind::Wire) {
        return 0;
    }
    let comparator = match info.kind {
        Some(Kind::Comparator) => comparator_output(cx, pos),
        _ => 0,
    };
    let own = output_signal(table, state, toward, comparator);
    if !info.conductor || own == MAX_SIGNAL {
        return own;
    }
    own.max(strong_power_of(cx, table, pos, wires))
}

pub(crate) fn best_neighbour_signal<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
    wires: WireSignals,
) -> u8
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    let mut best = 0;
    for direction in Direction::all() {
        let Some(next) = neighbour(pos, direction) else {
            continue;
        };
        best = best.max(signal_toward(cx, table, next, direction.opposite(), wires));
        if best == MAX_SIGNAL {
            break;
        }
    }
    best
}

pub(crate) fn is_powered<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
) -> bool
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    best_neighbour_signal(cx, table, pos, WireSignals::Count) > 0
}

pub(crate) fn has_signal_from<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
    direction: Direction,
) -> bool
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    let Some(next) = neighbour(pos, direction) else {
        return false;
    };
    signal_toward(cx, table, next, direction.opposite(), WireSignals::Count) > 0
}
