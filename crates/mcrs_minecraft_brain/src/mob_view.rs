use crate::ControlFlags;
use crate::Ticket;
use glam::DVec3;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityUuid;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MobView {
    pub mob: EntityId,
    pub key: EntityUuid,
    pub position: DVec3,
    pub eye_height: f32,
    pub flags: MobFlags,
    pub idle_time: u32,
    pub damage: Option<DamageRecord>,
    pub disabled: ControlFlags,
    pub path: Applied,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MobFlags(u16);

impl MobFlags {
    pub const NONE: Self = Self(0);
    pub const ALIVE: Self = Self(1);
    pub const ON_GROUND: Self = Self(1 << 1);
    pub const IN_WATER: Self = Self(1 << 2);
    pub const ON_FIRE: Self = Self(1 << 3);
    pub const CONTROLLED: Self = Self(1 << 4);
    pub const BABY: Self = Self(1 << 5);
    pub const IN_PANIC: Self = Self(1 << 6);

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }

    #[must_use]
    pub const fn without(self, other: Self) -> Self {
        Self(self.0 & !other.0)
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DamageRecord {
    pub tick: u64,
    pub cause: DamageCause,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DamageCause {
    Panic,
    Other,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct Applied {
    pub ticket: Ticket,
    pub progress: Progress,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Progress {
    #[default]
    Ended,
    Running,
}
