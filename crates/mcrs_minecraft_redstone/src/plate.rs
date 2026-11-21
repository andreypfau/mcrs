use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, BlockUpdateFlags, Cx, Kinds, SimulationVolume, TickPriority,
    neighbour,
};
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::component::{Kind, PlateKind};
use crate::memory::{HasRedstoneMemory, HasRedstoneSnapshot, Presence, Sensor};
use crate::power::{MAX_SIGNAL, plate_signal};
use crate::table::RedstoneTable;
use crate::wire::LIMIT;

pub(crate) const BINARY_CHECK: i32 = 20;
pub(crate) const WEIGHTED_CHECK: i32 = 10;
pub(crate) const HEAVY_PER_SIGNAL: u32 = 10;

#[must_use]
pub fn signal_for(kind: PlateKind, presence: Presence) -> u8 {
    match kind {
        PlateKind::Wooden => {
            if presence.entities > 0 {
                MAX_SIGNAL
            } else {
                0
            }
        }
        PlateKind::Stone => {
            if presence.living > 0 {
                MAX_SIGNAL
            } else {
                0
            }
        }
        PlateKind::LightWeighted => u8::try_from(presence.entities)
            .unwrap_or(MAX_SIGNAL)
            .min(MAX_SIGNAL),
        PlateKind::HeavyWeighted => {
            let count = u32::from(presence.entities);
            let signal = count.saturating_mul(u32::from(MAX_SIGNAL)).saturating_add(
                HEAVY_PER_SIGNAL
                    .saturating_mul(u32::from(MAX_SIGNAL))
                    .saturating_sub(1),
            ) / (HEAVY_PER_SIGNAL.saturating_mul(u32::from(MAX_SIGNAL)));
            u8::try_from(signal).unwrap_or(MAX_SIGNAL).min(MAX_SIGNAL)
        }
    }
}

fn check_time(kind: PlateKind) -> i32 {
    match kind {
        PlateKind::Wooden | PlateKind::Stone => BINARY_CHECK,
        PlateKind::LightWeighted | PlateKind::HeavyWeighted => WEIGHTED_CHECK,
    }
}

fn with_signal(
    table: &RedstoneTable,
    state: BlockStateId,
    kind: PlateKind,
    signal: u8,
) -> Option<BlockStateId> {
    match kind {
        PlateKind::Wooden | PlateKind::Stone => table.with_powered(state, signal > 0),
        PlateKind::LightWeighted | PlateKind::HeavyWeighted => table.with_power(state, signal),
    }
}

fn presence_at<K, V, F, B>(cx: &Cx<'_, K, V, F, B>, pos: BlockPos) -> Presence
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Shared: HasRedstoneSnapshot,
{
    cx.shared().redstone().presence(pos)
}

fn check_pressed<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
    state: BlockStateId,
    kind: PlateKind,
    current: u8,
) where
    K: Kinds<Block = Id<Block>>,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    let presence = presence_at(cx, pos);
    let next = signal_for(kind, presence);
    let block = table.block_of(state);
    if next != current
        && let Some(next_state) = with_signal(table, state, kind, next)
    {
        cx.set(pos, next_state, BlockUpdateFlags::CLIENTS, LIMIT);
        cx.update_neighbours(pos, block, None);
        if let Some(below) = neighbour(pos, Direction::Down) {
            cx.update_neighbours(below, block, None);
        }
    }
    if next > 0 {
        cx.schedule_block_tick(pos, block, check_time(kind), TickPriority::Normal);
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
    let Some(Kind::Plate(kind)) = table.kind(state) else {
        return;
    };
    let current = plate_signal(table, state);
    if current == 0 {
        check_pressed(cx, table, pos, state, kind, current);
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
    let Some(Kind::Plate(kind)) = table.kind(state) else {
        return;
    };
    let current = plate_signal(table, state);
    if current > 0 {
        check_pressed(cx, table, pos, state, kind, current);
    }
}

pub(crate) fn placed<K, V, F, B>(cx: &mut Cx<'_, K, V, F, B>, pos: BlockPos)
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    if let Some(memory) = cx.memory_at(pos) {
        memory.redstone().add_sensor(pos, Sensor::Plate);
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
    B::Shared: HasRedstoneSnapshot,
{
    if let Some(memory) = cx.memory_at(pos) {
        memory.redstone().remove_sensor(pos);
    }
    if moved_by_piston || plate_signal(table, old) == 0 {
        return;
    }
    let block = table.block_of(old);
    cx.update_neighbours(pos, block, None);
    if let Some(below) = neighbour(pos, Direction::Down) {
        cx.update_neighbours(below, block, None);
    }
}

#[must_use]
pub(crate) fn update_shape(
    table: &RedstoneTable,
    state: BlockStateId,
    from: Direction,
    neighbour_state: BlockStateId,
) -> BlockStateId {
    if from == Direction::Down && !table.rigid_top(neighbour_state) {
        table.air()
    } else {
        state
    }
}
