use bevy_ecs::component::Component;

/// The health of a living entity (DMG-ATT-001).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct Health {
    pub current: f32,
    pub max: f32,
    pub absorption: f32,
}

impl Health {
    #[must_use]
    pub const fn new(max: f32) -> Self {
        Self {
            current: max,
            max,
            absorption: 0.0,
        }
    }

    #[must_use]
    pub fn is_alive(&self) -> bool {
        self.current > 0.0
    }

    #[must_use]
    pub const fn is_dead(&self) -> bool {
        self.current <= 0.0
    }

    pub fn set_health(&mut self, health: f32) {
        self.current = health.clamp(0.0, self.max);
    }

    pub fn heal(&mut self, amount: f32) {
        if amount > 0.0 {
            self.current = (self.current + amount).min(self.max);
        }
    }
}

impl Default for Health {
    fn default() -> Self {
        Self::new(20.0)
    }
}

/// The cooldown and hurt time counters of an entity (DMG-HIT-003, DMG-HIT-006).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct DamageCooldown {
    pub cooldown: u8,
    pub hurt_time: u8,
    pub last_damage: f32,
}

impl DamageCooldown {
    pub fn tick(&mut self) {
        if self.hurt_time > 0 {
            self.hurt_time -= 1;
        }
        if self.cooldown > 0 {
            self.cooldown -= 1;
        }
        if self.cooldown == 0 {
            self.last_damage = 0.0;
        }
    }
}

/// The armor value of an entity (DMG-RED-001).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct Armor(pub f32);

/// The armor toughness of an entity (DMG-RED-001).
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct ArmorToughness(pub f32);

/// The knockback resistance factor of an entity, between 0.0 and 1.0.
#[derive(Component, Clone, Copy, Debug, Default, PartialEq)]
pub struct KnockbackResistance(pub f64);
