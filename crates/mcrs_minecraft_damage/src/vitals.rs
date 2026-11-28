#[derive(Clone, Copy, Debug, Default, PartialEq)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct Vitals {
    pub health: f32,
    pub absorption: f32,
    pub last_damage: f32,
    pub damage_cooldown: u8,
    pub hurt_time: u8,
    pub death_time: Option<u8>,
}

const _: () = assert!(
    size_of::<Vitals>() == 16,
    "a living entity pays 16 bytes for its health state"
);

impl Vitals {
    #[must_use]
    pub const fn new(health: f32) -> Self {
        Self {
            health,
            absorption: 0.0,
            last_damage: 0.0,
            damage_cooldown: 0,
            hurt_time: 0,
            death_time: None,
        }
    }

    #[must_use]
    pub const fn is_alive(&self) -> bool {
        self.death_time.is_none()
    }

    #[must_use]
    pub const fn is_counting(&self) -> bool {
        self.hurt_time > 0 || self.damage_cooldown > 0 || !self.is_alive()
    }
}
