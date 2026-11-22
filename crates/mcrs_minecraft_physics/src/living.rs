use crate::MotionAttributes;
use crate::MotionEffects;
use crate::MotionRow;
use crate::Orientation;

pub const CORPSE_REMOVAL_TICKS: u8 = 20;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Living {
    pub attributes: MotionAttributes,
    pub effects: MotionEffects,
    pub input: MoveInput,
    pub jump: bool,
    pub speed: f32,
    pub jump_delay: u8,
    pub orientation: Orientation,
    pub travel: Travel,
    pub in_powder_snow: bool,
    pub walks_on_powder_snow: bool,
    pub steering: Option<Steering>,
    pub health: f32,
    pub last_damage: f32,
    pub damage_cooldown: u8,
    pub hurt_time: u8,
    pub death_time: Option<u8>,
}

const _: () = assert!(
    size_of::<Living>() == 160,
    "a mob row pays nothing for its attribute instances: they live in a side table"
);

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MoveInput {
    pub sideways: f32,
    pub vertical: f32,
    pub forward: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Steering {
    pub input: MoveInput,
    pub jump: bool,
    pub speed: f32,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Travel {
    #[default]
    Open,
    Closed,
}

impl Living {
    #[must_use]
    pub fn of(row: &MotionRow) -> Self {
        let mut living = Self::default();
        living.attributes.step_height = row.step_height;
        living
    }

    #[must_use]
    pub fn is_alive(&self) -> bool {
        self.death_time.is_none()
    }
}

impl MoveInput {
    #[must_use]
    pub fn length_squared(&self) -> f64 {
        let [x, y, z] = [self.sideways, self.vertical, self.forward].map(f64::from);
        x * x + y * y + z * z
    }
}
