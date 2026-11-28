use alloc::vec::Vec;

use bevy_ecs::entity::Entity;

use crate::SlotRef;

#[derive(Default)]
pub(crate) struct Settlement {
    pub(crate) changed: Vec<SlotRef>,
    pub(crate) placed: Vec<Placed>,
    pub(crate) dropped: Vec<Taken>,
    pub(crate) despawned: Vec<Entity>,
}

impl Settlement {
    pub(crate) fn clear(&mut self) {
        self.changed.clear();
        self.placed.clear();
        self.dropped.clear();
        self.despawned.clear();
    }
}

pub(crate) struct Taken {
    pub(crate) source: Source,
    pub(crate) count: u8,
}

pub(crate) struct Placed {
    pub(crate) slot: SlotRef,
    pub(crate) source: Source,
    pub(crate) count: u8,
}

#[derive(Clone, Copy)]
pub(crate) enum Source {
    Same(Entity),
    CopyOf(Entity),
}
