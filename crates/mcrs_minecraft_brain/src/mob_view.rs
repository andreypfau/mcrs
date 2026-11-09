use crate::ControlFlags;
use crate::Ticket;
use glam::DVec3;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityKey;

/// The state of one mob that the decisions of one issue tick read: the row and the guidance state
/// at the start of that tick.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MobView {
    pub mob: EntityId,
    pub key: EntityKey,
    pub position: DVec3,
    pub eye_height: f32,
    pub flags: MobFlags,
    /// The idle time: it increases by 1 in each AI step and is 0 after a hit or while a player is
    /// within 32 blocks.
    pub idle_time: u32,
    pub damage: Option<DamageRecord>,
    /// The flags that the mob does not let any goal hold now, with the flags that its reflexes
    /// hold (the float in a fluid holds `jump`).
    pub disabled: ControlFlags,
    /// The last path intent that the host applied, and whether that path still runs.
    pub path: Applied,
}

/// A set of the boolean states of a mob that the goals read.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct MobFlags(u16);

impl MobFlags {
    pub const NONE: Self = Self(0);
    pub const ALIVE: Self = Self(1);
    pub const ON_GROUND: Self = Self(1 << 1);
    pub const IN_WATER: Self = Self(1 << 2);
    /// The fire ticks are more than 0 and the type is not immune to fire.
    pub const ON_FIRE: Self = Self(1 << 3);
    /// A passenger controls the mob.
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

/// The last hit on a living entity that the damage cooldown accepted.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct DamageRecord {
    pub tick: u64,
    pub cause: DamageCause,
}

/// Whether the damage type of a hit is in the tag `panic_causes`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum DamageCause {
    Panic,
    Other,
}

/// The progress of the last path intent that the host applied.
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
