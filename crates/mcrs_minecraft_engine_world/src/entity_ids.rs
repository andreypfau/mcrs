//! The global ids of the entities that are not players, which a cell takes from the ranges that
//! the coordinator grants it.

use std::collections::VecDeque;

use bevy_ecs::component::Component;
use bevy_ecs::resource::Resource;
use mcrs_minecraft_entity_key::EntityId;

/// The number of ids in one range. A cell asks for the next range when fewer than half of this
/// remain, and the grant arrives before its next tick, so a cell can take this many ids in a
/// tick.
pub const ENTITY_ID_RANGE_LEN: u64 = 4096;

/// The global id of an entity that is not a player. It is the same in every cell that owns the
/// entity, so the entity keeps its wire id when it moves.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GlobalEntityId(pub EntityId);

/// A range of [`ENTITY_ID_RANGE_LEN`] global ids that the coordinator grants to one cell: the
/// even ids from `next` up to `end`. A player id is odd, so the two never meet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntityIdRange {
    next: u64,
    end: u64,
}

impl EntityIdRange {
    /// The range that starts at the even id `first`. Gives `None` when its end passes `u64::MAX`.
    #[must_use]
    pub fn starting_at(first: u64) -> Option<Self> {
        let end = first.checked_add(ENTITY_ID_RANGE_LEN.checked_mul(2)?)?;
        Some(Self { next: first, end })
    }

    /// The first id after the range.
    #[must_use]
    pub fn end(self) -> u64 {
        self.end
    }

    fn remaining(self) -> u64 {
        self.end.saturating_sub(self.next) / 2
    }

    fn take(&mut self) -> Option<EntityId> {
        if self.next >= self.end {
            return None;
        }
        let id = EntityId(self.next);
        self.next = self.next.saturating_add(2);
        Some(id)
    }
}

/// The ranges that the cell holds, in the order the coordinator granted them.
#[derive(Resource, Debug, Default)]
pub struct CellEntityIds {
    ranges: VecDeque<EntityIdRange>,
}

impl CellEntityIds {
    /// The next free id of the cell. Gives `None` when the cell used all its ranges before the
    /// next grant arrived.
    pub fn allocate(&mut self) -> Option<EntityId> {
        loop {
            let range = self.ranges.front_mut()?;
            if let Some(id) = range.take() {
                return Some(id);
            }
            self.ranges.pop_front();
        }
    }

    pub fn add(&mut self, range: EntityIdRange) {
        self.ranges.push_back(range);
    }

    /// Whether the cell asks the coordinator for one more [`EntityIdRange`]: fewer than half of
    /// [`ENTITY_ID_RANGE_LEN`] ids remain.
    #[must_use]
    pub fn wants_range(&self) -> bool {
        let remaining: u64 = self.ranges.iter().map(|range| range.remaining()).sum();
        remaining < ENTITY_ID_RANGE_LEN / 2
    }
}
