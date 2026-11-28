use crate::GameMode;

#[derive(Clone, Copy, Debug, PartialEq)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct Abilities {
    pub invulnerable: bool,
    pub may_fly: bool,
    pub instant_build: bool,
    pub may_build: bool,
    pub flying_speed: f32,
    pub walking_speed: f32,
}

impl Abilities {
    #[must_use]
    pub fn new(mode: GameMode) -> Self {
        Self {
            invulnerable: false,
            may_fly: false,
            instant_build: false,
            may_build: false,
            flying_speed: 0.05_f32,
            walking_speed: 0.1_f32,
        }
        .changed_to(mode)
    }

    #[must_use]
    pub fn changed_to(self, mode: GameMode) -> Self {
        let (invulnerable, may_fly, instant_build, may_build) = match mode {
            GameMode::Survival => (false, false, false, true),
            GameMode::Creative => (true, true, true, true),
            GameMode::Adventure => (false, false, false, false),
            GameMode::Spectator => (true, true, false, false),
        };
        Self {
            invulnerable,
            may_fly,
            instant_build,
            may_build,
            ..self
        }
    }
}
