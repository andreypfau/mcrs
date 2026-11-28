pub const LIFETIME: i16 = 6000;
pub const NEVER_PICKED_UP: i16 = 32767;
pub const NEVER_AGES: i16 = -32768;
pub const DEFAULT_HEALTH: i16 = 5;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct ItemEntity {
    pub age: i16,
    pub pickup_delay: i16,
    pub health: i16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Lifetime {
    Lives,
    Ends,
}

impl Default for ItemEntity {
    fn default() -> Self {
        Self {
            age: 0,
            pickup_delay: 0,
            health: DEFAULT_HEALTH,
        }
    }
}

impl ItemEntity {
    #[must_use]
    pub fn with_pickup_delay(pickup_delay: i16) -> Self {
        Self {
            pickup_delay,
            ..Self::default()
        }
    }

    pub fn count_down(&mut self) {
        if self.pickup_delay > 0 && self.pickup_delay != NEVER_PICKED_UP {
            self.pickup_delay -= 1;
        }
    }

    pub fn grow(&mut self) -> Lifetime {
        if self.age != NEVER_AGES {
            self.age = self.age.saturating_add(1);
        }
        if self.age >= LIFETIME {
            Lifetime::Ends
        } else {
            Lifetime::Lives
        }
    }

    pub fn damage(&mut self, amount: f32) -> Lifetime {
        self.health = (f32::from(self.health) - amount) as i16;
        if self.health <= 0 {
            Lifetime::Ends
        } else {
            Lifetime::Lives
        }
    }

    #[must_use]
    pub fn can_merge(&self, count: u8, max: u8) -> bool {
        self.pickup_delay != NEVER_PICKED_UP
            && self.age != NEVER_AGES
            && self.age < LIFETIME
            && count < max
    }

    pub fn absorb(&mut self, other: Self) {
        self.pickup_delay = self.pickup_delay.max(other.pickup_delay);
        self.age = self.age.min(other.age);
    }
}
