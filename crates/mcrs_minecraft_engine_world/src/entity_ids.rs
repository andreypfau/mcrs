use std::collections::VecDeque;

use bevy_ecs::component::Component;
use bevy_ecs::resource::Resource;
use mcrs_minecraft_entity_key::EntityId;

pub const ENTITY_ID_RANGE_LEN: u64 = 4096;

#[derive(Component, Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct GlobalEntityId(pub EntityId);

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct EntityIdRange {
    next: u64,
    end: u64,
}

impl EntityIdRange {
    #[must_use]
    pub fn starting_at(first: u64) -> Option<Self> {
        let end = first.checked_add(ENTITY_ID_RANGE_LEN.checked_mul(2)?)?;
        Some(Self { next: first, end })
    }

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

#[derive(Resource, Debug, Default)]
pub struct CellEntityIds {
    ranges: VecDeque<EntityIdRange>,
}

impl CellEntityIds {
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

    #[must_use]
    pub fn wants_range(&self) -> bool {
        let remaining: u64 = self.ranges.iter().map(|range| range.remaining()).sum();
        remaining < ENTITY_ID_RANGE_LEN / 2
    }
}
