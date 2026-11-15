use alloc::vec::Vec;

use bevy_ecs::component::Component;

use crate::Gate;
use crate::SlotAccess;
use crate::capacity::SLOT_LIMIT;

/// The [`SlotAccess`] of each slot of a holder, read by each request that names a slot of it. A
/// holder without it, and a slot without an entry, has the limit 99 and permits.
///
/// [`uniform`](Self::uniform) and [`locked`](Self::locked) give an entry to each index; a policy
/// collected from an iterator has an entry for each item of it.
#[derive(Component, Clone, Debug, PartialEq)]
pub struct SlotPolicy {
    entries: Vec<SlotAccess>,
    rest: Option<SlotAccess>,
}

impl SlotPolicy {
    /// The same access for each slot.
    #[must_use]
    pub fn uniform(access: SlotAccess) -> Self {
        Self {
            entries: Vec::new(),
            rest: Some(access),
        }
    }

    /// Each slot refuses each take and each place.
    #[must_use]
    pub fn locked() -> Self {
        Self::uniform(SlotAccess {
            limit: SLOT_LIMIT,
            take: Gate::Refuses,
            place: Gate::Refuses,
        })
    }

    /// The access of the slot `index`, with its limit clamped.
    #[must_use]
    pub fn access(&self, index: u16) -> SlotAccess {
        let entry = self.entries.get(usize::from(index)).copied().or(self.rest);
        entry.unwrap_or(SlotAccess::OPEN).clamped()
    }

    /// Changes the entry of the slot `index`. An index without an entry changes nothing.
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
