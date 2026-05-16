use bevy_ecs::bundle::Bundle;
use bevy_ecs::prelude::Component;
use derive_more::{Deref, DerefMut};
use mcrs_protocol::GameMode;

#[derive(Component, Debug, Clone, Copy, Deref, DerefMut)]
pub struct PlayerGameMode(pub GameMode);

impl Default for PlayerGameMode {
    fn default() -> Self {
        Self(GameMode::Survival)
    }
}

#[derive(Component, Debug, Clone, Copy, Deref, DerefMut)]
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

impl Default for PlayerOpLevel {
    fn default() -> Self {
        Self(0)
    }
}

#[derive(Component, Default, Debug, Clone, Copy, Deref, DerefMut)]
pub struct Invulnerable(pub bool);

#[derive(Component, Default, Debug, Clone, Copy, Deref, DerefMut)]
pub struct Flying(pub bool);

#[derive(Component, Default, Debug, Clone, Copy, Deref, DerefMut)]
pub struct MayFly(pub bool);

#[derive(Component, Default, Debug, Clone, Copy)]
#[component(storage = "SparseSet")]
pub struct InstantBuild;

#[derive(Component, Debug, Clone, Copy, Deref, DerefMut)]
pub struct MayBuild(pub bool);

impl Default for MayBuild {
    fn default() -> Self {
        Self(true)
    }
}

#[derive(Component, Debug, Clone, Copy, Deref, DerefMut)]
pub struct FlySpeed(pub f32);

impl Default for FlySpeed {
    fn default() -> Self {
        Self(0.05)
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
    pub flying: Flying,
    pub may_fly: MayFly,
    pub may_build: MayBuild,
    pub fly_speed: FlySpeed,
    pub walk_speed: WalkSpeed,
}

impl PlayerAbilitiesBundle {
    fn update(&mut self, game_mode: &GameMode) -> &Self {
        if *game_mode == GameMode::Creative {
            *self.may_fly = true;
            // *self.instant_build = true;
            *self.invulnerable = true;
        } else if *game_mode == GameMode::Spectator {
            *self.may_fly = true;
            // *self.instant_build = false;
            *self.invulnerable = true;
            *self.flying = true;
        } else {
            *self.may_fly = false;
            // *self.instant_build = false;
            *self.invulnerable = false;
            *self.flying = false;
        }
        *self.may_build = !game_mode.is_block_placing_restricted();
        self
    }
}

pub fn update_abilities_for_game_mode(
    game_mode: GameMode,
    invulnerable: &mut Invulnerable,
    flying: &mut Flying,
    may_fly: &mut MayFly,
    may_build: &mut MayBuild,
) {
    match game_mode {
        GameMode::Creative => {
            **may_fly = true;
            **invulnerable = true;
        }
        GameMode::Spectator => {
            **may_fly = true;
            **invulnerable = true;
            **flying = true;
        }
        _ => {
            **may_fly = false;
            **invulnerable = false;
            **flying = false;
        }
    }
    **may_build = !game_mode.is_block_placing_restricted();
}
