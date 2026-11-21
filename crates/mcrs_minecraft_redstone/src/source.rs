use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, BlockUpdateFlags, Cx, Kinds, SimulationVolume, TickPriority,
    neighbour,
};
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::component::Kind;
use crate::memory::HasRedstoneSnapshot;
use crate::power::attached_of;
use crate::table::RedstoneTable;
use crate::wire::LIMIT;

pub(crate) const STONE_BUTTON_PRESS: i32 = 20;
pub(crate) const WOODEN_BUTTON_PRESS: i32 = 30;

fn press_time(kind: Kind) -> Option<i32> {
    match kind {
        Kind::Button { wooden: true } => Some(WOODEN_BUTTON_PRESS),
        Kind::Button { wooden: false } => Some(STONE_BUTTON_PRESS),
        _ => None,
    }
}

pub(crate) fn update_attached<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
    state: BlockStateId,
) where
    K: Kinds<Block = Id<Block>>,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    let block = table.block_of(state);
    cx.update_neighbours(pos, block, None);
    if let Some(attached) = attached_of(table, state)
        && let Some(target) = neighbour(pos, attached)
    {
        cx.update_neighbours(target, block, None);
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
    if table.block_of(old) != table.block_of(state) || table.powered(old) == table.powered(state) {
        return;
    }
    update_attached(cx, table, pos, state);
    if table.powered(state)
        && let Some(kind) = table.kind(state)
        && let Some(delay) = press_time(kind)
    {
        cx.schedule_block_tick(pos, table.block_of(state), delay, TickPriority::Normal);
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
    if !moved_by_piston && table.powered(old) {
        update_attached(cx, table, pos, old);
    }
}

fn projectiles_in<K, V, F, B>(cx: &Cx<'_, K, V, F, B>, pos: BlockPos) -> u16
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Shared: HasRedstoneSnapshot,
{
    cx.shared().redstone().projectiles_in(pos)
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
    B::Shared: HasRedstoneSnapshot,
{
    let Some(kind @ Kind::Button { wooden }) = table.kind(state) else {
        return;
    };
    if !table.powered(state) {
        return;
    }
    if wooden
        && projectiles_in(cx, pos) > 0
        && let Some(delay) = press_time(kind)
    {
        cx.schedule_block_tick(pos, table.block_of(state), delay, TickPriority::Normal);
        return;
    }
    if let Some(released) = table.with_powered(state, false) {
        cx.set(pos, released, BlockUpdateFlags::ALL, LIMIT);
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
    B::Shared: HasRedstoneSnapshot,
{
    if !matches!(table.kind(state), Some(Kind::Button { wooden: true })) || table.powered(state) {
        return;
    }
    if projectiles_in(cx, pos) > 0
        && let Some(pressed) = table.with_powered(state, true)
    {
        cx.set(pos, pressed, BlockUpdateFlags::ALL, LIMIT);
    }
}

#[must_use]
pub(crate) fn update_shape(
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
pub(crate) fn toggled_by_use(table: &RedstoneTable, state: BlockStateId) -> Option<BlockStateId> {
    match table.kind(state)? {
        Kind::Lever => table.with_powered(state, !table.powered(state)),
        Kind::Button { .. } if !table.powered(state) => table.with_powered(state, true),
        _ => None,
    }
}
