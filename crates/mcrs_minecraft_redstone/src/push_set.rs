use mcrs_minecraft_block::material::PushReaction;
use mcrs_minecraft_core::Direction;
use mcrs_minecraft_simulation::{
    BlockBehaviour, BlockFacts, Cx, Kinds, SimulationVolume, neighbour,
};
use mcrs_voxel::{BlockPos, BlockStateId};

use crate::component::{Kind, Stickiness};
use crate::power::offset_by;
use crate::rules::RedstoneRules;
use crate::table::RedstoneTable;

pub(crate) const PUSH_LIMIT: usize = 12;
const CAPACITY: usize = PUSH_LIMIT + 1;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Positions {
    items: [BlockPos; CAPACITY],
    len: usize,
}

impl Positions {
    const EMPTY: Self = Self {
        items: [BlockPos::new(0, 0, 0); CAPACITY],
        len: 0,
    };

    fn push(&mut self, pos: BlockPos) -> bool {
        let Some(slot) = self.items.get_mut(self.len) else {
            return false;
        };
        *slot = pos;
        self.len = self.len.saturating_add(1);
        true
    }

    pub(crate) fn as_slice(&self) -> &[BlockPos] {
        self.items.get(..self.len).unwrap_or_default()
    }

    pub(crate) fn len(&self) -> usize {
        self.len
    }

    fn contains(&self, pos: BlockPos) -> bool {
        self.as_slice().contains(&pos)
    }

    fn position(&self, pos: BlockPos) -> Option<usize> {
        self.as_slice().iter().position(|known| *known == pos)
    }

    fn get(&self, index: usize) -> Option<BlockPos> {
        self.as_slice().get(index).copied()
    }

    fn move_tail_before(&mut self, count: usize, index: usize) {
        if let Some(window) = self.items.get_mut(index..self.len)
            && count <= window.len()
        {
            window.rotate_right(count);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct PushSet {
    pub(crate) to_push: Positions,
    pub(crate) to_destroy: Positions,
}

impl PushSet {
    const EMPTY: Self = Self {
        to_push: Positions::EMPTY,
        to_destroy: Positions::EMPTY,
    };
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Destroy {
    Allowed,
    Refused,
}

fn sticks(a: Stickiness, b: Stickiness) -> bool {
    (a.is_sticky() || b.is_sticky()) && a.sticks_to(b)
}

pub(crate) fn is_pushable(
    table: &RedstoneTable,
    rules: &RedstoneRules,
    state: BlockStateId,
    move_direction: Direction,
    destroy: Destroy,
    piston_facing: Direction,
) -> bool {
    let info = table.info(state);
    if info.air {
        return true;
    }
    if info.unbreakable || state == table.void_air() {
        return false;
    }
    match info.kind {
        Some(Kind::Piston { .. }) => {
            if table.extended(state) {
                return false;
            }
        }
        Some(Kind::PistonHead | Kind::MovingPiston) => return false,
        _ => match info.push {
            PushReaction::Block => return false,
            PushReaction::Destroy => return destroy == Destroy::Allowed,
            PushReaction::PushOnly => return move_direction == piston_facing,
            PushReaction::Normal | PushReaction::Ignore => {}
        },
    }
    !info.block_entity || rules.pistons_move_block_entities
}

struct Resolver<'a> {
    table: &'a RedstoneTable,
    rules: &'a RedstoneRules,
    piston: BlockPos,
    facing: Direction,
    push_direction: Direction,
    unreadable: bool,
    set: PushSet,
}

impl Resolver<'_> {
    fn state<K, V, F, B>(&mut self, cx: &mut Cx<'_, K, V, F, B>, pos: BlockPos) -> BlockStateId
    where
        K: Kinds,
        V: SimulationVolume,
        F: BlockFacts<K>,
        B: BlockBehaviour<K>,
    {
        match cx.state(pos) {
            Ok(state) => state,
            Err(_) => {
                self.unreadable = true;
                self.table.void_air()
            }
        }
    }

    fn stickiness(&self, state: BlockStateId) -> Stickiness {
        self.table.info(state).stickiness
    }

    fn over_limit(&self, more: usize) -> bool {
        more.saturating_add(self.set.to_push.len()) > PUSH_LIMIT
    }

    fn pushable(&self, state: BlockStateId, destroy: Destroy) -> bool {
        is_pushable(
            self.table,
            self.rules,
            state,
            self.push_direction,
            destroy,
            self.facing,
        )
    }

    fn add_block_line<K, V, F, B>(
        &mut self,
        cx: &mut Cx<'_, K, V, F, B>,
        origin: BlockPos,
        direction: Direction,
    ) -> bool
    where
        K: Kinds,
        V: SimulationVolume,
        F: BlockFacts<K>,
        B: BlockBehaviour<K>,
    {
        let _ = direction;
        let state = self.state(cx, origin);
        if state == self.table.void_air() {
            return false;
        }
        if self.table.is_air(state) || !self.pushable(state, Destroy::Refused) {
            return true;
        }
        if origin == self.piston || self.set.to_push.contains(origin) {
            return true;
        }
        let Some(behind) = self.count_behind(cx, origin, state) else {
            return false;
        };
        let mut added: usize = 0;
        for step in (0..behind).rev() {
            let Some(count) = i32::try_from(step).ok() else {
                return false;
            };
            let Some(pos) = offset_by(origin, self.push_direction.opposite(), count) else {
                return false;
            };
            if !self.set.to_push.push(pos) {
                return false;
            }
            added = added.saturating_add(1);
        }
        let mut ahead: i32 = 1;
        loop {
            let Some(pos) = offset_by(origin, self.push_direction, ahead) else {
                return false;
            };
            if let Some(index) = self.set.to_push.position(pos) {
                self.set.to_push.move_tail_before(added, index);
                let end = index.saturating_add(added);
                for slot in 0..=end {
                    let Some(known) = self.set.to_push.get(slot) else {
                        break;
                    };
                    let known_state = self.state(cx, known);
                    if self.stickiness(known_state).is_sticky() && !self.add_branching(cx, known) {
                        return false;
                    }
                }
                return true;
            }
            let next = self.state(cx, pos);
            if next == self.table.void_air() {
                return false;
            }
            if self.table.is_air(next) {
                return true;
            }
            if !self.pushable(next, Destroy::Allowed) || pos == self.piston {
                return false;
            }
            if self.table.info(next).push == PushReaction::Destroy {
                return self.set.to_destroy.push(pos);
            }
            if self.set.to_push.len() >= PUSH_LIMIT || !self.set.to_push.push(pos) {
                return false;
            }
            added = added.saturating_add(1);
            ahead = ahead.saturating_add(1);
        }
    }

    fn count_behind<K, V, F, B>(
        &mut self,
        cx: &mut Cx<'_, K, V, F, B>,
        origin: BlockPos,
        state: BlockStateId,
    ) -> Option<usize>
    where
        K: Kinds,
        V: SimulationVolume,
        F: BlockFacts<K>,
        B: BlockBehaviour<K>,
    {
        let mut behind: usize = 1;
        if self.over_limit(behind) {
            return None;
        }
        let mut current = state;
        while self.stickiness(current).is_sticky() {
            let count = i32::try_from(behind).ok()?;
            let Some(back) = offset_by(origin, self.push_direction.opposite(), count) else {
                break;
            };
            let next = self.state(cx, back);
            if self.table.is_air(next)
                || !sticks(self.stickiness(current), self.stickiness(next))
                || !self.pushable(next, Destroy::Refused)
                || back == self.piston
            {
                break;
            }
            behind = behind.saturating_add(1);
            if self.over_limit(behind) {
                return None;
            }
            current = next;
        }
        Some(behind)
    }

    fn add_branching<K, V, F, B>(&mut self, cx: &mut Cx<'_, K, V, F, B>, pos: BlockPos) -> bool
    where
        K: Kinds,
        V: SimulationVolume,
        F: BlockFacts<K>,
        B: BlockBehaviour<K>,
    {
        let state = self.state(cx, pos);
        for direction in Direction::all() {
            if direction.axis() == self.push_direction.axis() {
                continue;
            }
            let Some(adjacent) = neighbour(pos, direction) else {
                continue;
            };
            let next = self.state(cx, adjacent);
            if sticks(self.stickiness(state), self.stickiness(next))
                && !self.add_block_line(cx, adjacent, direction)
            {
                return false;
            }
        }
        true
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Move {
    Extend,
    Retract,
}

pub(crate) fn start_of(piston: BlockPos, facing: Direction, movement: Move) -> Option<BlockPos> {
    match movement {
        Move::Extend => neighbour(piston, facing),
        Move::Retract => offset_by(piston, facing, 2),
    }
}

pub(crate) fn push_direction(facing: Direction, movement: Move) -> Direction {
    match movement {
        Move::Extend => facing,
        Move::Retract => facing.opposite(),
    }
}

pub(crate) fn resolve<K, V, F, B>(
    cx: &mut Cx<'_, K, V, F, B>,
    table: &RedstoneTable,
    rules: &RedstoneRules,
    piston: BlockPos,
    facing: Direction,
    movement: Move,
) -> Option<PushSet>
where
    K: Kinds,
    V: SimulationVolume,
    F: BlockFacts<K>,
    B: BlockBehaviour<K>,
{
    let push = push_direction(facing, movement);
    let start = start_of(piston, facing, movement)?;
    let mut resolver = Resolver {
        table,
        rules,
        piston,
        facing,
        push_direction: push,
        unreadable: false,
        set: PushSet::EMPTY,
    };
    let state = resolver.state(cx, start);
    if !is_pushable(table, rules, state, push, Destroy::Refused, facing) {
        if movement == Move::Extend && table.info(state).push == PushReaction::Destroy {
            resolver.set.to_destroy.push(start);
            return Some(resolver.set);
        }
        return None;
    }
    if !resolver.add_block_line(cx, start, push) {
        return None;
    }
    let mut index = 0;
    while let Some(pos) = resolver.set.to_push.get(index) {
        let state = resolver.state(cx, pos);
        if resolver.stickiness(state).is_sticky() && !resolver.add_branching(cx, pos) {
            return None;
        }
        index = index.saturating_add(1);
    }
    if resolver.unreadable {
        return None;
    }
    Some(resolver.set)
}
