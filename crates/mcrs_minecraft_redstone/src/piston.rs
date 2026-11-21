use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::material::PushReaction;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, BlockUpdateFlags, Cx, Drops, Kinds, SimulationVolume, TickPriority,
    neighbour,
};
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::component::Kind;
use crate::memory::{Carried, HasRedstoneMemory, HasRedstoneSnapshot, MovePhase, MovingPayload};
use crate::power::{has_signal_from, neighbour_state, offset_by, state_at};
use crate::push_set::{Destroy, Move, is_pushable, push_direction, resolve};
use crate::rules::RedstoneRules;
use crate::table::RedstoneTable;
use crate::wire::LIMIT;

pub(crate) const EVENT_EXTEND: i32 = 0;
pub(crate) const EVENT_RETRACT: i32 = 1;
pub(crate) const EVENT_RETRACT_WITHOUT_PULL: i32 = 2;
pub const MOVE_TICKS: i32 = 2;
pub(crate) const REACH: u8 = 14;

const MOVING_FLAGS: BlockUpdateFlags =
    BlockUpdateFlags::MOVE_BY_PISTON.union(BlockUpdateFlags::INVISIBLE);
const VACATED_FLAGS: BlockUpdateFlags = BlockUpdateFlags::MOVE_BY_PISTON
    .union(BlockUpdateFlags::KNOWN_SHAPE)
    .union(BlockUpdateFlags::CLIENTS);
const EXTENDED_FLAGS: BlockUpdateFlags =
    BlockUpdateFlags::MOVE_BY_PISTON.union(BlockUpdateFlags::ALL);
const RETRACTING_BASE_FLAGS: BlockUpdateFlags =
    BlockUpdateFlags::KNOWN_SHAPE.union(BlockUpdateFlags::INVISIBLE);

fn direction_index(direction: Direction) -> i32 {
    i32::try_from(direction.id()).unwrap_or(0)
}

fn direction_from_index(index: i32) -> Option<Direction> {
    Direction::all()
        .into_iter()
        .find(|direction| direction_index(*direction) == index & 7)
}

fn sticky_of(table: &RedstoneTable, state: BlockStateId) -> bool {
    matches!(table.kind(state), Some(Kind::Piston { sticky: true }))
}

pub(crate) fn is_powered<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    rules: &RedstoneRules,
    pos: BlockPos,
    facing: Direction,
) -> bool
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    for direction in Direction::all() {
        if direction != facing && has_signal_from(cx, table, pos, direction) {
            return true;
        }
    }
    if !rules.quasi_connectivity {
        return false;
    }
    let Some(above) = neighbour(pos, Direction::Up) else {
        return false;
    };
    Direction::all().into_iter().any(|direction| {
        direction != Direction::Down && has_signal_from(cx, table, above, direction)
    })
}

fn payload_at<K, V, F, B>(cx: &mut Cx<'_, K, V, F, B>, pos: BlockPos) -> Option<MovingPayload>
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    cx.memory()
        .and_then(|memory| memory.redstone().moving.get(&pos).copied())
}

fn store_payload<K, V, F, B>(cx: &mut Cx<'_, K, V, F, B>, pos: BlockPos, payload: MovingPayload)
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    if let Some(memory) = cx.memory() {
        memory.redstone().moving.insert(pos, payload);
    }
}

pub(crate) fn check_extend<K, V, F, B>(
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
    let Some(facing) = table.facing(state) else {
        return;
    };
    let block = table.block_of(state);
    let extended = table.extended(state);
    let powered = is_powered(cx, table, rules, pos, facing);
    if powered && !extended {
        if resolve(cx, table, rules, pos, facing, Move::Extend).is_some() {
            cx.queue_block_event(pos, block, EVENT_EXTEND, direction_index(facing));
        }
    } else if !powered && extended {
        let mut event = EVENT_RETRACT;
        if sticky_of(table, state)
            && rules.sticky_piston_short_pulse
            && let Some(front) = offset_by(pos, facing, 2)
            && let Some(payload) = payload_at(cx, front)
            && payload.facing == facing
            && payload.is_extending()
        {
            event = EVENT_RETRACT_WITHOUT_PULL;
        }
        cx.queue_block_event(pos, block, event, direction_index(facing));
    }
}

fn start_moving<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    at: BlockPos,
    moving: BlockStateId,
    payload: MovingPayload,
) where
    K: Kinds<Block = Id<Block>>,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    store_payload(cx, at, payload);
    cx.set(at, moving, MOVING_FLAGS, LIMIT);
    cx.schedule_block_tick(at, table.block_of(moving), MOVE_TICKS, TickPriority::Normal);
}

fn destinations_owned<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    moved: &[(BlockPos, BlockStateId)],
    push: Direction,
    movement: Move,
    pos: BlockPos,
    facing: Direction,
) -> bool
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    for &(at, _) in moved {
        let Some(destination) = neighbour(at, push) else {
            return false;
        };
        if !cx.owns(destination) {
            return false;
        }
    }
    movement == Move::Retract || neighbour(pos, facing).is_some_and(|front| cx.owns(front))
}

fn move_blocks<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    rules: &RedstoneRules,
    pos: BlockPos,
    facing: Direction,
    movement: Move,
) -> bool
where
    K: Kinds<Block = Id<Block>>,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    if movement == Move::Retract
        && let Some(front) = neighbour(pos, facing)
        && state_at(cx, front).is_some_and(|state| table.kind(state) == Some(Kind::PistonHead))
    {
        cx.set(front, table.air(), RETRACTING_BASE_FLAGS, LIMIT);
    }
    let Some(set) = resolve(cx, table, rules, pos, facing, movement) else {
        return false;
    };
    let push = push_direction(facing, movement);
    let sticky = state_at(cx, pos).is_some_and(|state| sticky_of(table, state));
    let now = cx.now();
    let mut destroyed: Vec<(BlockPos, BlockStateId)> = Vec::with_capacity(set.to_destroy.len());
    for &at in set.to_destroy.as_slice().iter().rev() {
        if let Some(state) = state_at(cx, at) {
            destroyed.push((at, state));
            cx.destroy(at, Drops::Released, LIMIT);
        }
    }
    let mut moved: Vec<(BlockPos, BlockStateId)> = Vec::with_capacity(set.to_push.len());
    for &at in set.to_push.as_slice() {
        let Some(state) = state_at(cx, at) else {
            return false;
        };
        moved.push((at, state));
    }
    let Some(moving) = table.moving_piston(push, sticky) else {
        return false;
    };
    let phase = match movement {
        Move::Extend => MovePhase::Extending,
        Move::Retract => MovePhase::Retracting,
    };
    if !destinations_owned(cx, &moved, push, movement, pos, facing) {
        return false;
    }
    let mut occupied: Vec<BlockPos> = Vec::with_capacity(moved.len().saturating_add(1));
    for &(at, state) in moved.iter().rev() {
        let Some(destination) = neighbour(at, push) else {
            return false;
        };
        start_moving(
            cx,
            table,
            destination,
            moving,
            MovingPayload {
                state,
                facing: push,
                carried: Carried::Block,
                phase,
                started: now,
            },
        );
        occupied.push(destination);
    }
    if movement == Move::Extend
        && let Some(front) = neighbour(pos, facing)
        && let Some(head) = table.piston_head(facing, sticky)
        && let Some(moving_head) = table.moving_piston(facing, sticky)
    {
        start_moving(
            cx,
            table,
            front,
            moving_head,
            MovingPayload {
                state: head,
                facing,
                carried: Carried::Head,
                phase: MovePhase::Extending,
                started: now,
            },
        );
        occupied.push(front);
    }
    for &(at, _) in &moved {
        if !occupied.contains(&at) {
            cx.set(at, table.air(), VACATED_FLAGS, LIMIT);
        }
    }
    for &(at, state) in &destroyed {
        cx.update_neighbours(at, table.block_of(state), None);
    }
    for &(at, state) in &moved {
        cx.update_neighbours(at, table.block_of(state), None);
    }
    if movement == Move::Extend
        && let Some(front) = neighbour(pos, facing)
    {
        cx.update_neighbours(front, Block::PistonHead.id(), None);
    }
    true
}

pub(crate) fn block_event<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    rules: &RedstoneRules,
    pos: BlockPos,
    state: BlockStateId,
    event: i32,
    facing_index: i32,
) -> bool
where
    K: Kinds<Block = Id<Block>>,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    let Some(facing) = direction_from_index(facing_index) else {
        return false;
    };
    if table.facing(state) != Some(facing) {
        check_extend(cx, table, rules, pos, state);
        return false;
    }
    let sticky = sticky_of(table, state);
    let powered = is_powered(cx, table, rules, pos, facing);
    if powered && (event == EVENT_RETRACT || event == EVENT_RETRACT_WITHOUT_PULL) {
        if let Some(extended) = table.with_extended(state, true) {
            cx.set(pos, extended, BlockUpdateFlags::CLIENTS, LIMIT);
        }
        return false;
    }
    if !powered && event == EVENT_EXTEND {
        return false;
    }
    if event == EVENT_EXTEND {
        if !move_blocks(cx, table, rules, pos, facing, Move::Extend) {
            return false;
        }
        if let Some(extended) = table.with_extended(state, true) {
            cx.set(pos, extended, EXTENDED_FLAGS, LIMIT);
        }
        return true;
    }
    let Some(front) = neighbour(pos, facing) else {
        return false;
    };
    if payload_at(cx, front).is_some() {
        complete_move(cx, table, front);
    }
    let Some(moving_base) = table.moving_piston(facing, sticky) else {
        return false;
    };
    let Some(base) = table.piston(sticky, facing, false) else {
        return false;
    };
    store_payload(
        cx,
        pos,
        MovingPayload {
            state: base,
            facing,
            carried: Carried::Base,
            phase: MovePhase::Retracting,
            started: cx.now(),
        },
    );
    cx.set(pos, moving_base, RETRACTING_BASE_FLAGS, LIMIT);
    let moving_block = table.block_of(moving_base);
    cx.schedule_block_tick(pos, moving_block, MOVE_TICKS, TickPriority::Normal);
    cx.update_neighbours(pos, moving_block, None);
    if sticky {
        let pulled = offset_by(pos, facing, 2).and_then(|far| {
            let state = state_at(cx, far)?;
            Some((far, state))
        });
        let mut finished = false;
        if let Some((far, _)) = pulled
            && let Some(payload) = payload_at(cx, far)
            && payload.facing == facing
            && payload.is_extending()
        {
            complete_move(cx, table, far);
            finished = rules.sticky_piston_short_pulse;
        }
        let pulled = pulled.and_then(|(far, _)| Some((far, state_at(cx, far)?)));
        if !finished {
            let pulls = pulled.is_some_and(|(_, far_state)| {
                let info = table.info(far_state);
                let piston = matches!(info.kind, Some(Kind::Piston { .. }));
                event == EVENT_RETRACT
                    && !info.air
                    && is_pushable(
                        table,
                        rules,
                        far_state,
                        facing.opposite(),
                        Destroy::Refused,
                        facing,
                    )
                    && (info.push == PushReaction::Normal || piston)
            });
            if pulls {
                move_blocks(cx, table, rules, pos, facing, Move::Retract);
            } else {
                cx.set(front, table.air(), BlockUpdateFlags::ALL, LIMIT);
            }
        } else {
            cx.set(front, table.air(), BlockUpdateFlags::ALL, LIMIT);
        }
    } else {
        cx.set(front, table.air(), BlockUpdateFlags::ALL, LIMIT);
    }
    true
}

pub(crate) fn tick_complete<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
) where
    K: Kinds<Block = Id<Block>>,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    let now = cx.now().0;
    let age = payload_at(cx, pos).map(|payload| now.saturating_sub(payload.started.0));
    match age {
        Some(age) if age < i64::from(MOVE_TICKS) => {
            let left = i32::try_from(i64::from(MOVE_TICKS).saturating_sub(age)).unwrap_or(1);
            cx.schedule_block_tick(pos, Block::MovingPiston.id(), left, TickPriority::Normal);
        }
        _ => complete_move(cx, table, pos),
    }
}

pub(crate) fn complete_move<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
) where
    K: Kinds<Block = Id<Block>>,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    let payload = cx
        .memory()
        .and_then(|memory| memory.redstone().moving.remove(&pos));
    if !state_at(cx, pos).is_some_and(|state| table.kind(state) == Some(Kind::MovingPiston)) {
        return;
    }
    let Some(payload) = payload else {
        return;
    };
    cx.set(pos, payload.state, EXTENDED_FLAGS, LIMIT);
    cx.update_neighbour(pos, table.block_of(payload.state));
}

pub(crate) fn moving_neighbour_changed<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    pos: BlockPos,
) where
    K: Kinds<Block = Id<Block>>,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    let due = payload_at(cx, pos).is_some_and(|payload| {
        cx.now().0.saturating_sub(payload.started.0) >= i64::from(MOVE_TICKS)
    });
    if due && !cx.has_scheduled_block_tick(pos, Block::MovingPiston.id()) {
        complete_move(cx, table, pos);
    }
}

pub(crate) fn moving_removed<K, V, F, B>(cx: &mut Cx<'_, K, V, F, B>, pos: BlockPos)
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
    B::Memory: HasRedstoneMemory,
    B::Shared: HasRedstoneSnapshot,
{
    if let Some(memory) = cx.memory() {
        memory.redstone().moving.remove(&pos);
    }
}

#[must_use]
pub(crate) fn head_update_shape(
    table: &RedstoneTable,
    state: BlockStateId,
    from: Direction,
    neighbour_state: BlockStateId,
) -> BlockStateId {
    let Some(facing) = table.facing(state) else {
        return state;
    };
    if from != facing.opposite() {
        return state;
    }
    let sticky = table.is_sticky_type(state);
    let fits = match table.kind(neighbour_state) {
        Some(Kind::Piston {
            sticky: base_sticky,
        }) => {
            base_sticky == sticky
                && table.extended(neighbour_state)
                && table.facing(neighbour_state) == Some(facing)
        }
        Some(Kind::MovingPiston) => table.facing(neighbour_state) == Some(facing),
        _ => false,
    };
    if fits { state } else { table.air() }
}

pub(crate) fn head_neighbour_changed<K, V, F, B>(
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
    let Some(facing) = table.facing(state) else {
        return;
    };
    if let Some((base, base_state)) = neighbour_state(cx, pos, facing.opposite()) {
        cx.update_neighbour(base, table.block_of(base_state));
    }
}
