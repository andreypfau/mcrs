use bevy_ecs::resource::Resource;

pub const NORMAL_TICK_RATE: f32 = 20.0;

#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct ServerTicking {
    pub tick_rate: f32,
    pub frozen: bool,
    pub steps: u32,
}

impl Default for ServerTicking {
    fn default() -> Self {
        Self {
            tick_rate: NORMAL_TICK_RATE,
            frozen: false,
            steps: 0,
        }
    }
}

impl ServerTicking {
    #[must_use]
    pub fn runs(&self) -> bool {
        !self.frozen || self.steps > 0
    }

    #[must_use]
    pub fn tick_speed(&self) -> f32 {
        if self.tick_rate > NORMAL_TICK_RATE {
            self.tick_rate / NORMAL_TICK_RATE
        } else {
            1.0
        }
    }

    #[must_use]
    pub fn partial_tick(&self, overstep_fraction: f32) -> f32 {
        if self.runs() { overstep_fraction } else { 1.0 }
    }
}
