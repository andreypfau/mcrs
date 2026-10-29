//! The game mode, the flight and the operator level of a player.

use bevy_ecs::prelude::Component;
use derive_more::{Deref, DerefMut};
use mcrs_minecraft_protocol::GameMode;

/// The game mode of a player.
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

/// The operator permission level of a player, from 0 for a player who is no operator.
#[derive(Component, Debug, Clone, Copy, Deref, DerefMut, Default)]
pub struct PlayerOpLevel(pub u8);

impl PlayerOpLevel {
    /// The highest level; a higher stored value counts as this one.
    pub const MAX: u8 = 4;

    /// The level, at most [`Self::MAX`].
    pub fn clamped(self) -> u8 {
        self.0.min(Self::MAX)
    }

    /// The `entity_event` status that tells a client its operator level.
    pub fn entity_status(self) -> i8 {
        24i8 + self.clamped() as i8
    }
}
