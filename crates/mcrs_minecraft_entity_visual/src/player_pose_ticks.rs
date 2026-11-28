use bevy_ecs::prelude::*;
use mcrs_minecraft_numeric as numeric;
use mcrs_minecraft_pose::Pose;

use crate::synced_data::SyncedData;

const SWIM_AMOUNT_STEP: f32 = 0.09;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct PlayerPoseTicks {
    pub swim_amount: f32,
    pub previous_swim_amount: f32,
    pub fall_flying_ticks: u32,
}

impl PlayerPoseTicks {
    pub fn advance(&mut self, data: &SyncedData) {
        self.previous_swim_amount = self.swim_amount;
        self.swim_amount = if data.pose == Pose::Swimming {
            (self.swim_amount + SWIM_AMOUNT_STEP).min(1.0)
        } else {
            (self.swim_amount - SWIM_AMOUNT_STEP).max(0.0)
        };
        self.fall_flying_ticks = if data.is_fall_flying() {
            self.fall_flying_ticks.saturating_add(1)
        } else {
            0
        };
    }

    #[must_use]
    pub fn frame_swim_amount(&self, partial_tick: f32) -> f32 {
        numeric::lerp(partial_tick, self.previous_swim_amount, self.swim_amount)
    }
}
