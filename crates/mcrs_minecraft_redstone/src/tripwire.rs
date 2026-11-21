use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, BlockUpdateFlags, Cx, Kinds, SimulationVolume, TickPriority,
    neighbour,
};
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::component::Kind;
use crate::memory::{HasRedstoneMemory, HasRedstoneSnapshot, Sensor};
use crate::power::{attached_of, offset_by, state_at};
use crate::table::RedstoneTable;
use crate::wire::LIMIT;

pub(crate) const LINE_LIMIT: i32 = 40;
pub(crate) const RECHECK: i32 = 10;
pub(crate) const REACH: u8 = 42;

struct Line {
    strings: Vec<(BlockPos, BlockStateId)>,
    end: Option<(BlockPos, BlockStateId)>,
}

fn has_presence<K, V, F, B>(cx: &Cx<'_, K, V, F, B>, pos: BlockPos) -> bool
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Shared: HasRedstoneSnapshot,
{
    cx.shared().redstone().presence(pos).entities > 0
}

fn scan_line<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    hook: BlockPos,
    facing: Direction,
) -> Line
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    let mut line = Line {
        strings: Vec::new(),
        end: None,
    };
    for step in 1..=LINE_LIMIT {
        let Some(pos) = offset_by(hook, facing, step) else {
            break;
        };
        let Some(state) = state_at(cx, pos) else {
            break;
        };
        match table.kind(state) {
            Some(Kind::Tripwire) => line.strings.push((pos, state)),
            Some(Kind::TripwireHook) if table.facing(state) == Some(facing.opposite()) => {
                line.end = Some((pos, state));
                break;
            }
            _ => break,
        }
    }
    line
}

fn set_hook<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
    state: BlockStateId,
    attached: bool,
    powered: bool,
) where
    K: Kinds<Block = Id<Block>>,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    let next = table
        .with_attached(state, attached)
        .and_then(|next| table.with_powered(next, powered));
    let Some(next) = next else {
        return;
    };
    if next != state {
        cx.set(pos, next, BlockUpdateFlags::ALL, LIMIT);
    }
    if powered != table.powered(state) {
        let block = table.block_of(state);
        cx.update_neighbours(pos, block, None);
        if let Some(attached_to) = attached_of(table, state).and_then(|side| neighbour(pos, side)) {
            cx.update_neighbours(attached_to, block, None);
        }
    }
}

pub(crate) fn hook_check<K, V, F, B>(
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
    let line = scan_line(cx, table, pos, facing);
    let attached = line.end.is_some();
    let mut powered = false;
    for &(string, string_state) in &line.strings {
        let pressed = !table.disarmed(string_state) && has_presence(cx, string);
        powered |= attached && pressed;
        let next = table
            .with_attached(string_state, attached)
            .and_then(|next| table.with_powered(next, pressed));
        if let Some(next) = next
            && next != string_state
        {
            cx.set(string, next, BlockUpdateFlags::CLIENTS, LIMIT);
        }
    }
    set_hook(cx, table, pos, state, attached, powered);
    if let Some((end, end_state)) = line.end {
        set_hook(cx, table, end, end_state, attached, powered);
    }
    if powered {
        let block = table.block_of(state);
        if !cx.has_scheduled_block_tick(pos, block) {
            cx.schedule_block_tick(pos, block, RECHECK, TickPriority::Normal);
        }
    }
}

fn hooks_of_string<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
) -> Vec<(BlockPos, BlockStateId)>
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    let mut hooks = Vec::new();
    for direction in Direction::HORIZONTAL {
        for step in 1..=LINE_LIMIT {
            let Some(at) = offset_by(pos, direction, step) else {
                break;
            };
            let Some(state) = state_at(cx, at) else {
                break;
            };
            match table.kind(state) {
                Some(Kind::Tripwire) => continue,
                Some(Kind::TripwireHook) if table.facing(state) == Some(direction.opposite()) => {
                    hooks.push((at, state));
                }
                _ => {}
            }
            break;
        }
    }
    hooks
}

fn update_hooks<K, V, F, B>(cx: &mut Cx<'_, K, V, F, B>, table: &RedstoneTable, pos: BlockPos)
where
    K: Kinds<Block = Id<Block>>,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    for (hook, state) in hooks_of_string(cx, table, pos) {
        hook_check(cx, table, hook, state);
    }
}

pub(crate) fn string_placed<K, V, F, B>(
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
    if let Some(memory) = cx.memory() {
        memory.redstone().sensors.insert(pos, Sensor::Tripwire);
    }
    update_hooks(cx, table, pos);
}

pub(crate) fn string_removed<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
    moved_by_piston: bool,
) where
    K: Kinds<Block = Id<Block>>,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    if let Some(memory) = cx.memory() {
        memory.redstone().sensors.remove(&pos);
    }
    if !moved_by_piston {
        update_hooks(cx, table, pos);
    }
}

pub(crate) fn string_neighbour_changed<K, V, F, B>(
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
    let pressed = !table.disarmed(state) && has_presence(cx, pos);
    if pressed == table.powered(state) {
        return;
    }
    if pressed {
        update_hooks(cx, table, pos);
    } else {
        let block = table.block_of(state);
        if !cx.has_scheduled_block_tick(pos, block) {
            cx.schedule_block_tick(pos, block, RECHECK, TickPriority::Normal);
        }
    }
}

pub(crate) fn string_tick<K, V, F, B>(
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
    let pressed = !table.disarmed(state) && has_presence(cx, pos);
    if pressed != table.powered(state) {
        update_hooks(cx, table, pos);
    }
}

pub(crate) fn hook_placed<K, V, F, B>(
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
    if table.block_of(old) != table.block_of(state) {
        hook_check(cx, table, pos, state);
    }
}

pub(crate) fn hook_removed<K, V, F, B>(
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
    if moved_by_piston {
        return;
    }
    let Some(facing) = table.facing(old) else {
        return;
    };
    let line = scan_line(cx, table, pos, facing);
    for (string, string_state) in line.strings {
        let next = table
            .with_attached(string_state, false)
            .and_then(|next| table.with_powered(next, false));
        if let Some(next) = next
            && next != string_state
        {
            cx.set(string, next, BlockUpdateFlags::CLIENTS, LIMIT);
        }
    }
    if let Some((end, end_state)) = line.end {
        set_hook(cx, table, end, end_state, false, false);
    }
    if table.powered(old) {
        let block = table.block_of(old);
        cx.update_neighbours(pos, block, None);
        if let Some(attached_to) = attached_of(table, old).and_then(|side| neighbour(pos, side)) {
            cx.update_neighbours(attached_to, block, None);
        }
    }
}

pub(crate) fn hook_tick<K, V, F, B>(
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
    hook_check(cx, table, pos, state);
}

#[must_use]
pub(crate) fn hook_update_shape(
    table: &RedstoneTable,
    state: BlockStateId,
    from: Direction,
    neighbour_state: BlockStateId,
) -> BlockStateId {
    if attached_of(table, state) == Some(from) && !table.full_face(neighbour_state, from.opposite())
    {
        table.air()
    } else {
        state
    }
}

#[must_use]
pub(crate) fn string_update_shape(
    table: &RedstoneTable,
    state: BlockStateId,
    from: Direction,
    neighbour_state: BlockStateId,
) -> BlockStateId {
    if from.is_vertical() {
        return state;
    }
    let linked = matches!(
        table.kind(neighbour_state),
        Some(Kind::Tripwire | Kind::TripwireHook)
    );
    table.with_link(state, from, linked).unwrap_or(state)
}
