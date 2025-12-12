use bevy_ecs::bundle::Bundle;
use bevy_ecs::prelude::Component;
use derive_more::{Deref, DerefMut};
use mcrs_protocol::GameMode;
#[derive(Component, Default, Debug, Clone, Copy, Deref, DerefMut)]
pub struct Invulnerable(pub bool);

#[derive(Component, Default, Debug, Clone, Copy, Deref, DerefMut)]
pub struct Flying(pub bool);

#[derive(Component, Default, Debug, Clone, Copy, Deref, DerefMut)]
pub struct MayFly(pub bool);

#[derive(Component, Default, Debug, Clone, Copy, Deref, DerefMut)]
pub struct InstantBuild(pub bool);

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
    pub instant_build: InstantBuild,
    pub may_build: MayBuild,
    pub fly_speed: FlySpeed,
    pub walk_speed: WalkSpeed,
}

impl PlayerAbilitiesBundle {
    fn update(&mut self, game_mode: &GameMode) -> &Self {
        if *game_mode == GameMode::Creative {
            *self.may_fly = true;
            *self.instant_build = true;
            *self.invulnerable = true;
        } else if *game_mode == GameMode::Spectator {
            *self.may_fly = true;
            *self.instant_build = false;
            *self.invulnerable = true;
            *self.flying = true;
        } else {
            *self.may_fly = false;
            *self.instant_build = false;
            *self.invulnerable = false;
            *self.flying = false;
        }
        *self.may_build = !game_mode.is_block_placing_restricted();
        self
    }
}
