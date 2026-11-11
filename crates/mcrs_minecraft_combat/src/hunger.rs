use bevy_ecs::component::Component;

/// The hunger and saturation state of a player (DMG-PLR, #261).
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct FoodData {
    pub food_level: u32,
    pub saturation: f32,
    pub exhaustion: f32,
    pub tick_timer: u32,
}

impl Default for FoodData {
    fn default() -> Self {
        Self {
            food_level: 20,
            saturation: 5.0,
            exhaustion: 0.0,
            tick_timer: 0,
        }
    }
}

impl FoodData {
    #[must_use]
    pub const fn new(food_level: u32, saturation: f32) -> Self {
        Self {
            food_level,
            saturation,
            exhaustion: 0.0,
            tick_timer: 0,
        }
    }

    /// Adds exhaustion from player actions (sprinting, jumping, taking damage).
    pub fn add_exhaustion(&mut self, amount: f32) {
        self.exhaustion = (self.exhaustion + amount).min(40.0);
        while self.exhaustion >= 4.0 {
            self.exhaustion -= 4.0;
            if self.saturation > 0.0 {
                self.saturation = (self.saturation - 1.0).max(0.0);
            } else if self.food_level > 0 {
                self.food_level -= 1;
            }
        }
    }

    /// Eats food, restoring food level and saturation.
    pub fn eat(&mut self, food: u32, saturation_modifier: f32) {
        self.food_level = (self.food_level + food).min(20);
        let max_saturation = self.food_level as f32;
        let gained_saturation = (food as f32) * saturation_modifier * 2.0;
        self.saturation = (self.saturation + gained_saturation).min(max_saturation);
    }

    /// Advances one tick of hunger, returning `(heal_amount, starve_damage)`.
    pub fn tick(&mut self, current_health: f32, max_health: f32) -> (f32, f32) {
        self.tick_timer += 1;

        if self.food_level == 20 && self.saturation > 0.0 && current_health < max_health {
            if self.tick_timer >= 10 {
                self.tick_timer = 0;
                self.add_exhaustion(6.0);
                return (1.0, 0.0);
            }
        } else if self.food_level >= 18 && current_health < max_health {
            if self.tick_timer >= 80 {
                self.tick_timer = 0;
                self.add_exhaustion(6.0);
                return (1.0, 0.0);
            }
        } else if self.food_level == 0 {
            if self.tick_timer >= 80 {
                self.tick_timer = 0;
                return (0.0, 1.0);
            }
        } else {
            if self.tick_timer >= 80 {
                self.tick_timer = 0;
            }
        }

        (0.0, 0.0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn exhaustion_depletes_saturation_then_food() {
        let mut food = FoodData::new(20, 2.0);
        food.add_exhaustion(4.0);
        assert_eq!(food.saturation, 1.0);
        assert_eq!(food.food_level, 20);

        food.add_exhaustion(4.0);
        assert_eq!(food.saturation, 0.0);
        assert_eq!(food.food_level, 20);

        food.add_exhaustion(4.0);
        assert_eq!(food.saturation, 0.0);
        assert_eq!(food.food_level, 19);
    }

    #[test]
    fn fast_regeneration_when_full_and_saturated() {
        let mut food = FoodData::new(20, 5.0);
        let mut healed = 0.0;
        for _ in 0..10 {
            let (h, _) = food.tick(15.0, 20.0);
            healed += h;
        }
        assert_eq!(healed, 1.0);
    }

    #[test]
    fn starvation_damage_at_zero_food() {
        let mut food = FoodData::new(0, 0.0);
        let mut damage = 0.0;
        for _ in 0..80 {
            let (_, d) = food.tick(10.0, 20.0);
            damage += d;
        }
        assert_eq!(damage, 1.0);
    }
}
