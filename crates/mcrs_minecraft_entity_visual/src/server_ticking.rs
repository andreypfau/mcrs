use bevy_ecs::resource::Resource;

/// The tick rate at which the client runs as fast as the server: 20 ticks a second.
pub const NORMAL_TICK_RATE: f32 = 20.0;

/// The tick rate of the server and whether it froze the world, as its last packets said.
#[derive(Resource, Clone, Copy, Debug, PartialEq)]
pub struct ServerTicking {
    pub tick_rate: f32,
    pub frozen: bool,
    /// The ticks that a frozen world still runs.
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
    /// Whether the entities that are not players tick in the next client tick.
    #[must_use]
    pub fn runs(&self) -> bool {
        !self.frozen || self.steps > 0
    }

    /// The factor by which an interpolation runs faster on a server faster than 20 ticks a
    /// second.
    #[must_use]
    pub fn tick_speed(&self) -> f32 {
        if self.tick_rate > NORMAL_TICK_RATE {
            self.tick_rate / NORMAL_TICK_RATE
        } else {
            1.0
        }
    }

    /// The partial tick at which a frame draws an entity that is not a player. A frozen entity
    /// shows the state of its latest tick.
    #[must_use]
    pub fn partial_tick(&self, overstep_fraction: f32) -> f32 {
        if self.runs() { overstep_fraction } else { 1.0 }
    }
}
