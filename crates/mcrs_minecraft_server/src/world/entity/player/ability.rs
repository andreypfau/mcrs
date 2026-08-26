use bevy_ecs::bundle::Bundle;
use bevy_ecs::prelude::Component;
use derive_more::{Deref, DerefMut};
use mcrs_minecraft_protocol::GameMode;

pub use mcrs_minecraft_world::entity::player::{Flying, FlyingSpeed, InstantBuild};

#[derive(Component, Debug, Clone, Copy, Deref, DerefMut)]
pub struct PlayerGameMode(pub GameMode);

impl Default for PlayerGameMode {
    fn default() -> Self {
        Self(GameMode::Survival)
    }
}

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

#[derive(Component, Default, Debug, Clone, Copy, Deref, DerefMut)]
pub struct Invulnerable(pub bool);

#[derive(Component, Default, Debug, Clone, Copy, Deref, DerefMut)]
pub struct MayFly(pub bool);

#[derive(Component, Debug, Clone, Copy, Deref, DerefMut)]
pub struct MayBuild(pub bool);

impl Default for MayBuild {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Component, Debug, Clone, Copy, Deref, DerefMut)]
pub struct WalkSpeed(pub f32);

impl Default for WalkSpeed {
    fn default() -> Self {
        Self(0.1)
    }
}

#[derive(Bundle, Default)]
pub struct PlayerAbilitiesBundle {
    pub invulnerable: Invulnerable,
    pub may_fly: MayFly,
    pub may_build: MayBuild,
    pub fly_speed: FlyingSpeed,
    pub walk_speed: WalkSpeed,
}

/// Returns whether the player should carry the `Flying` marker afterwards;
/// membership is the caller's to apply, since a marker cannot be assigned.
#[must_use]
pub fn update_abilities_for_game_mode(
    game_mode: GameMode,
    invulnerable: &mut Invulnerable,
    may_fly: &mut MayFly,
    may_build: &mut MayBuild,
) -> bool {
    let flying = match game_mode {
        GameMode::Creative => {
            **may_fly = true;
            **invulnerable = true;
            false
        }
        GameMode::Spectator => {
            **may_fly = true;
            **invulnerable = true;
            true
        }
        _ => {
            **may_fly = false;
            **invulnerable = false;
            false
        }
    };
    **may_build = !game_mode.is_block_placing_restricted();
    flying
}
