use bevy_ecs::component::Component;
use bevy_ecs::prelude::Resource;
use mcrs_minecraft_protocol_game::Difficulty;

pub use mcrs_minecraft_damage::{fall_damage as compute_fall_damage, is_in_void};
pub use mcrs_minecraft_entity::WorldBorderSettings as WorldBorderState;

/// The difficulty that decides how far starvation can go.
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CombatDifficulty(pub Difficulty);

impl Default for CombatDifficulty {
    fn default() -> Self {
        Self(Difficulty::Normal)
    }
}

/// The air supply of an entity under water.
#[derive(Component, Clone, Copy, Debug, PartialEq, Eq)]
pub struct AirSupply {
    pub air: i32,
    pub max_air: i32,
}

impl Default for AirSupply {
    fn default() -> Self {
        Self {
            air: 300,
            max_air: 300,
        }
    }
}

impl AirSupply {
    #[must_use]
    pub const fn new(max_air: i32) -> Self {
        Self { air: max_air, max_air }
    }

    pub fn tick_underwater(&mut self) -> bool {
        self.air -= 1;
        if self.air <= -20 {
            self.air = 0;
            true
        } else {
            false
        }
    }

    pub fn tick_in_air(&mut self) {
        if self.air < self.max_air {
            self.air = (self.air + 4).min(self.max_air);
        }
    }
}

/// The burning state of an entity.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FireState {
    pub remaining_fire_ticks: i32,
}

impl FireState {
    pub fn ignite(&mut self, ticks: i32) {
        self.remaining_fire_ticks = self.remaining_fire_ticks.max(ticks);
    }

    pub fn extinguish(&mut self) {
        self.remaining_fire_ticks = 0;
    }

    pub fn tick(&mut self) -> bool {
        if self.remaining_fire_ticks > 0 {
            self.remaining_fire_ticks -= 1;
            self.remaining_fire_ticks % 20 == 0
        } else {
            false
        }
    }
}

/// The freezing state of an entity in powder snow.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct FreezeState {
    pub ticks_frozen: i32,
}

impl FreezeState {
    pub const FULLY_FROZEN_TICKS: i32 = 140;

    pub fn tick_in_powder_snow(&mut self) -> bool {
        self.ticks_frozen = (self.ticks_frozen + 1).min(300);
        if self.ticks_frozen >= Self::FULLY_FROZEN_TICKS {
            (self.ticks_frozen - Self::FULLY_FROZEN_TICKS) % 40 == 0
        } else {
            false
        }
    }

    pub fn tick_warm(&mut self) {
        if self.ticks_frozen > 0 {
            self.ticks_frozen = (self.ticks_frozen - 2).max(0);
        }
    }

    #[must_use]
    pub const fn is_frozen(&self) -> bool {
        self.ticks_frozen >= Self::FULLY_FROZEN_TICKS
    }
}
