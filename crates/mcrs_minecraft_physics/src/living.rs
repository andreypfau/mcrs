use crate::MotionAttributes;
use crate::MotionEffects;
use crate::MotionRow;
use crate::Orientation;

/// The state of a living entity that is not a player, besides its [`Body`](crate::Body).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Living {
    pub attributes: MotionAttributes,
    pub effects: MotionEffects,
    /// The input vector, which decays by `0.98F` in each tick until the AI sets it again.
    pub input: MoveInput,
    pub jump: bool,
    /// The speed of the input acceleration on the ground, as the AI sets it.
    pub speed: f32,
    pub jump_delay: u8,
    pub orientation: Orientation,
    pub travel: Travel,
    /// True when the entity was inside powder snow in the previous tick.
    pub in_powder_snow: bool,
    pub walks_on_powder_snow: bool,
    /// The new input of the AI in this tick, if it sets one.
    pub steering: Option<Steering>,
    pub vitals: Vitals,
    pub fire_immune: bool,
    pub fall_damage_immune: bool,
}

const _: () = assert!(core::mem::size_of::<Living>() == 184);

/// The state of a living entity that a hit, the fire, the air and the powder snow change
/// (DMG-ATT-001, DMG-HIT-003, DMG-PLR-002).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Vitals {
    /// The health, from 0 to `max_health`.
    pub health: f32,
    pub max_health: f32,
    pub absorption: f32,
    /// The amount of the last full or partial hit, which the damage cooldown compares against.
    pub last_damage: f32,
    pub damage_cooldown: u8,
    pub hurt_time: u8,
    /// The ticks since the death; `None` while the entity is alive.
    pub death_time: Option<u8>,
    /// The remaining fire ticks; the entity burns while this is positive.
    pub fire_ticks: i32,
    /// The remaining air ticks underwater; damage is dealt at `-20`.
    pub air: i32,
    /// The ticks in powder snow, from 0 to `140` and above when the damage starts.
    pub frozen_ticks: i32,
}

impl Default for Vitals {
    fn default() -> Self {
        Self {
            health: 0.0,
            max_health: 20.0,
            absorption: 0.0,
            last_damage: 0.0,
            damage_cooldown: 0,
            hurt_time: 0,
            death_time: None,
            fire_ticks: 0,
            air: 300,
            frozen_ticks: 0,
        }
    }
}

/// The movement intent of a living entity in its own frame, as f32 values.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct MoveInput {
    pub sideways: f32,
    pub vertical: f32,
    pub forward: f32,
}

/// What the AI sets for one tick.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Steering {
    pub input: MoveInput,
    pub jump: bool,
    pub speed: f32,
}

/// The travel gate of a living entity: when it is closed, the entity does not do the travel
/// step, for example a mob with `NoAI` or an armor stand with `Marker`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Travel {
    #[default]
    Open,
    Closed,
}

impl Living {
    /// A living entity of the type with the default attributes of the type.
    #[must_use]
    pub fn of(row: &MotionRow) -> Self {
        let mut living = Self::default();
        living.attributes.step_height = row.step_height;
        living
    }

    /// Whether the entity is alive: not in the dying state.
    #[must_use]
    pub fn is_alive(&self) -> bool {
        self.vitals.death_time.is_none()
    }

    /// Whether the entity is drawn on fire.
    #[must_use]
    pub fn on_fire(&self) -> bool {
        self.vitals.fire_ticks > 0
    }
}

impl MoveInput {
    /// The squared length of the vector, widened.
    #[must_use]
    pub fn length_squared(&self) -> f64 {
        let [x, y, z] = [self.sideways, self.vertical, self.forward].map(f64::from);
        x * x + y * y + z * z
    }
}
