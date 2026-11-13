/// The age at which an item entity ends (IEN-AGE-001).
pub const LIFETIME: i16 = 6000;
/// The pickup delay that never counts down and forbids a merge (IEN-MOV-002, IEN-MRG-002).
pub const NEVER_PICKED_UP: i16 = 32767;
/// The age that never counts up: the item entity stays (IEN-AGE-001).
pub const NEVER_AGES: i16 = -32768;
pub const DEFAULT_HEALTH: i16 = 5;

/// The values that an item entity adds to its stack (IEN-DAT-002). The owner and the thrower are
/// the host's, beside it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub struct ItemEntity {
    pub age: i16,
    pub pickup_delay: i16,
    pub health: i16,
}

/// Whether an item entity stays after a tick or a damage.
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
    /// An item entity of the defaults with `pickup_delay`.
    #[must_use]
    pub fn with_pickup_delay(pickup_delay: i16) -> Self {
        Self {
            pickup_delay,
            ..Self::default()
        }
    }

    /// The pickup delay of one tick, step 5 of IEN-MOV-006 (IEN-MOV-002).
    pub fn count_down(&mut self) {
        if self.pickup_delay > 0 && self.pickup_delay != NEVER_PICKED_UP {
            self.pickup_delay -= 1;
        }
    }

    /// The age of one tick, step 8 of IEN-MOV-006, and whether it ends the item entity, step 10
    /// (IEN-AGE-001).
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

    /// A damage of `amount`: the health becomes the integer part of the difference, toward zero
    /// (IEN-DMG-001).
    pub fn damage(&mut self, amount: f32) -> Lifetime {
        self.health = (f32::from(self.health) - amount) as i16;
        if self.health <= 0 {
            Lifetime::Ends
        } else {
            Lifetime::Lives
        }
    }

    /// True when a stack of `count` items whose maximum is `max` can merge (IEN-MRG-002). The
    /// host checks that the item entity is not removed.
    #[must_use]
    pub fn can_merge(&self, count: u8, max: u8) -> bool {
        self.pickup_delay != NEVER_PICKED_UP
            && self.age != NEVER_AGES
            && self.age < LIFETIME
            && count < max
    }

    /// Takes the values of `other` that a merge into `self` keeps: the larger pickup delay and
    /// the smaller age (IEN-MRG-005).
    pub fn absorb(&mut self, other: Self) {
        self.pickup_delay = self.pickup_delay.max(other.pickup_delay);
        self.age = self.age.min(other.age);
    }
}
