use bevy_ecs::prelude::Component;
use derive_more::{Deref, DerefMut};
use mcrs_minecraft_protocol::GameMode;

#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Deref, DerefMut)]
pub struct PlayerGameMode(pub GameMode);

impl Default for PlayerGameMode {
    fn default() -> Self {
        Self(GameMode::Survival)
    }
}

/// Whether the player flies, as its client reports it and its game mode allows (CMD-GM-007).
#[derive(Component, Debug, Clone, Copy, PartialEq, Eq, Deref, DerefMut, Default)]
pub struct PlayerFlying(pub bool);

#[derive(Component, Debug, Clone, Copy, Deref, DerefMut, Default)]
pub struct PlayerOpLevel(pub u8);

impl PlayerOpLevel {
    pub const MAX: u8 = 4;

    pub fn clamped(self) -> u8 {
        self.0.min(Self::MAX)
    }

    pub fn entity_status(self) -> i8 {
        24i8 + self.clamped() as i8
    }
}
