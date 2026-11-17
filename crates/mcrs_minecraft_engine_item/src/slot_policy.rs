use alloc::vec::Vec;

use bevy_ecs::component::Component;

use crate::Gate;
use crate::SlotAccess;
use crate::capacity::SLOT_LIMIT;

#[derive(Component, Clone, Debug, PartialEq)]
pub struct SlotPolicy {
    entries: Vec<SlotAccess>,
    rest: Option<SlotAccess>,
}

impl SlotPolicy {
    #[must_use]
    pub fn uniform(access: SlotAccess) -> Self {
        Self {
            entries: Vec::new(),
            rest: Some(access),
        }
    }

    #[must_use]
    pub fn locked() -> Self {
        Self::uniform(SlotAccess {
            limit: SLOT_LIMIT,
            take: Gate::Refuses,
            place: Gate::Refuses,
        })
    }

    #[must_use]
    pub fn access(&self, index: u16) -> SlotAccess {
        let entry = self.entries.get(usize::from(index)).copied().or(self.rest);
        entry.unwrap_or(SlotAccess::OPEN).clamped()
    }

    pub fn set(&mut self, index: u16, access: SlotAccess) {
        let index = usize::from(index);
        if index >= self.entries.len()
            && let Some(rest) = self.rest
        {
            self.entries.resize(index.saturating_add(1), rest);
        }
        if let Some(entry) = self.entries.get_mut(index) {
            *entry = access;
        }
    }
}

impl FromIterator<SlotAccess> for SlotPolicy {
    fn from_iter<I: IntoIterator<Item = SlotAccess>>(entries: I) -> Self {
        Self {
            entries: entries.into_iter().collect(),
            rest: None,
        }
    }
}
