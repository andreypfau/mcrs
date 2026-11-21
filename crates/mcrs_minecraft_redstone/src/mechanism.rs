use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::schema::NoteBlockInstrument;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, BlockUpdateFlags, Cx, Kinds, SimulationVolume, TickPriority,
    neighbour,
};
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::component::{Kind, Trigger};
use crate::memory::{HasRedstoneMemory, HasRedstoneSnapshot};
use crate::power::{is_powered, neighbour_state};
use crate::rules::RedstoneRules;
use crate::table::RedstoneTable;
use crate::wire::LIMIT;

pub(crate) const TRIGGER_DELAY: i32 = 4;
pub(crate) const NOTE_COUNT: u8 = 25;
pub(crate) const NOTE_EVENT: i32 = 0;

fn set_if_changed<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    pos: BlockPos,
    state: BlockStateId,
    next: Option<BlockStateId>,
    flags: BlockUpdateFlags,
) -> bool
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    match next {
        Some(next) if next != state => {
            cx.set(pos, next, flags, LIMIT);
            true
        }
        _ => false,
    }
}

fn other_half<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
    state: BlockStateId,
) -> Option<(BlockPos, BlockStateId)>
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    let half = table.half(state)?;
    let (other, other_state) = neighbour_state(cx, pos, half.other_half())?;
    (table.block_of(other_state) == table.block_of(state)).then_some((other, other_state))
}

fn door_is_powered<K, V, F, B>(
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
    other_half(cx, table, pos, state).is_some_and(|(other, _)| is_powered(cx, table, other))
}

fn follow_power<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
    state: BlockStateId,
    powered: bool,
) where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    let next = table
        .with_powered(state, powered)
        .and_then(|next| table.with_open(next, powered));
    set_if_changed(cx, pos, state, next, BlockUpdateFlags::CLIENTS);
}

pub(crate) fn door_neighbour_changed<K, V, F, B>(
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
    let powered = door_is_powered(cx, table, pos, state);
    if powered == table.powered(state) {
        return;
    }
    follow_power(cx, table, pos, state, powered);
    if let Some((other, other_state)) = other_half(cx, table, pos, state) {
        follow_power(cx, table, other, other_state, powered);
    }
}

pub(crate) fn door_placed<K, V, F, B>(
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
        door_neighbour_changed(cx, table, pos, state);
        return;
    }
    if table.open(old) == table.open(state) {
        return;
    }
    if let Some((other, other_state)) = other_half(cx, table, pos, state) {
        let next = table.with_open(other_state, table.open(state));
        set_if_changed(cx, other, other_state, next, BlockUpdateFlags::CLIENTS);
    }
}

pub(crate) fn gate_neighbour_changed<K, V, F, B>(
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
    let powered = is_powered(cx, table, pos);
    if powered != table.powered(state) {
        follow_power(cx, table, pos, state, powered);
    }
}

pub(crate) fn hopper_neighbour_changed<K, V, F, B>(
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
    let enabled = !is_powered(cx, table, pos);
    if enabled != table.enabled(state) {
        let next = table.with_enabled(state, enabled);
        set_if_changed(cx, pos, state, next, BlockUpdateFlags::CLIENTS);
    }
}

fn trigger_is_powered<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    rules: &RedstoneRules,
    pos: BlockPos,
    trigger: Trigger,
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
    if trigger == Trigger::Crafter || !rules.quasi_connectivity {
        return false;
    }
    neighbour(pos, Direction::Up).is_some_and(|above| is_powered(cx, table, above))
}

pub(crate) fn trigger_neighbour_changed<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    rules: &RedstoneRules,
    pos: BlockPos,
    state: BlockStateId,
    trigger: Trigger,
) where
    K: Kinds<Block = Id<Block>>,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    let powered = trigger_is_powered(cx, table, rules, pos, trigger);
    let triggered = table.triggered(state);
    if powered && !triggered {
        let next = table.with_triggered(state, true);
        if set_if_changed(cx, pos, state, next, BlockUpdateFlags::CLIENTS) {
            cx.schedule_block_tick(
                pos,
                table.block_of(state),
                TRIGGER_DELAY,
                TickPriority::Normal,
            );
        }
    } else if !powered && triggered {
        let next = table.with_triggered(state, false);
        set_if_changed(cx, pos, state, next, BlockUpdateFlags::CLIENTS);
    }
}

pub(crate) fn bulb_neighbour_changed<K, V, F, B>(
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
    let powered = is_powered(cx, table, pos);
    if powered == table.powered(state) {
        return;
    }
    let mut next = table.with_powered(state, powered);
    if powered {
        next = next.and_then(|next| table.with_lit(next, !table.lit(state)));
    }
    set_if_changed(cx, pos, state, next, BlockUpdateFlags::ALL);
}

fn instrument_around<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
) -> NoteBlockInstrument
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    let above =
        neighbour_state(cx, pos, Direction::Up).map(|(_, state)| table.info(state).instrument);
    if let Some(instrument) = above.filter(|instrument| instrument.works_above_note_block()) {
        return instrument;
    }
    neighbour_state(cx, pos, Direction::Down)
        .map(|(_, state)| table.info(state).instrument)
        .filter(|instrument| !instrument.works_above_note_block())
        .unwrap_or(NoteBlockInstrument::Harp)
}

pub(crate) fn note_neighbour_changed<K, V, F, B>(
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
    let instrument = instrument_around(cx, table, pos);
    let mut next = if table.instrument(state) == Some(instrument) {
        Some(state)
    } else {
        table.with_instrument(state, instrument)
    };
    let powered = is_powered(cx, table, pos);
    if powered != table.powered(state) {
        next = next.and_then(|next| table.with_powered(next, powered));
        if powered {
            cx.queue_block_event(pos, table.block_of(state), NOTE_EVENT, 0);
        }
    }
    set_if_changed(cx, pos, state, next, BlockUpdateFlags::ALL);
}

pub(crate) fn note_placed<K, V, F, B>(
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
        note_neighbour_changed(cx, table, pos, state);
    } else if table.note(old) != table.note(state) {
        cx.queue_block_event(pos, table.block_of(state), NOTE_EVENT, 0);
    }
}

#[must_use]
pub(crate) fn note_event(table: &RedstoneTable, state: BlockStateId, event: i32) -> bool {
    event == NOTE_EVENT && table.kind(state) == Some(Kind::NoteBlock)
}

#[must_use]
pub(crate) fn toggled_by_use(table: &RedstoneTable, state: BlockStateId) -> Option<BlockStateId> {
    match table.kind(state)? {
        Kind::Door { wooden: true } | Kind::Trapdoor { wooden: true } | Kind::FenceGate => {
            table.with_open(state, !table.open(state))
        }
        Kind::NoteBlock => {
            table.with_note(state, (table.note(state).saturating_add(1)) % NOTE_COUNT)
        }
        Kind::DaylightDetector => table.with_inverted(state, !table.inverted(state)),
        _ => None,
    }
}

pub(crate) fn placed_powered<K, V, F, B>(
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
    if table.block_of(old) == table.block_of(state) {
        return;
    }
    match table.kind(state) {
        Some(Kind::Trapdoor { .. } | Kind::FenceGate) => {
            gate_neighbour_changed(cx, table, pos, state)
        }
        Some(Kind::Hopper) => hopper_neighbour_changed(cx, table, pos, state),
        Some(Kind::Trigger(trigger)) => {
            trigger_neighbour_changed(cx, table, rules, pos, state, trigger);
        }
        Some(Kind::CopperBulb) => {
            let powered = is_powered(cx, table, pos);
            let next = table.with_powered(state, powered);
            set_if_changed(cx, pos, state, next, BlockUpdateFlags::ALL);
        }
        _ => {}
    }
}
