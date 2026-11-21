use glam::DVec3;
use mcrs_minecraft_entity_key::EntityId;
use mcrs_minecraft_entity_key::EntityUuid;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct PlayerRow {
    pub player: EntityId,
    pub key: EntityUuid,
    pub position: DVec3,
    pub eye_height: f32,
    pub flags: PlayerFlags,
    pub visibility: f32,
    pub vehicle: Option<EntityId>,
}

impl PlayerRow {
    #[must_use]
    pub fn eye(&self) -> DVec3 {
        DVec3::new(
            self.position.x,
            self.position.y + f64::from(self.eye_height),
            self.position.z,
        )
    }

    #[must_use]
    pub fn is_qualified(&self) -> bool {
        self.flags.contains(PlayerFlags::ALIVE) && !self.flags.contains(PlayerFlags::SPECTATOR)
    }
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct PlayerFlags(u8);

impl PlayerFlags {
    pub const NONE: Self = Self(0);
    pub const ALIVE: Self = Self(1);
    pub const SPECTATOR: Self = Self(1 << 1);

    #[must_use]
    pub const fn union(self, other: Self) -> Self {
        Self(self.0 | other.0)
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
