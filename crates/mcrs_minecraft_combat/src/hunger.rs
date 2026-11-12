use bevy_ecs::component::Component;
use bevy_ecs::prelude::Resource;
use mcrs_minecraft_protocol_game::Difficulty;

/// The difficulty that decides how far starvation can go (#261).
#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub struct CombatDifficulty(pub Difficulty);

impl Default for CombatDifficulty {
    fn default() -> Self {
        Self(Difficulty::Normal)
    }
}

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
        while self.exhaustion > 4.0 {
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
    pub fn tick(
        &mut self,
        current_health: f32,
        max_health: f32,
        natural_regeneration: bool,
        difficulty: Difficulty,
    ) -> (f32, f32) {
        self.tick_timer += 1;

        if difficulty == Difficulty::Peaceful && natural_regeneration {
            if self.tick_timer >= 10 {
                self.tick_timer = 0;
                let mut heal = 0.0;
                if current_health < max_health {
                    heal = 1.0;
                }
                if self.food_level < 20 {
                    self.food_level += 1;
                }
                return (heal, 0.0);
            }
            return (0.0, 0.0);
        }

        if natural_regeneration
            && self.food_level == 20
            && self.saturation > 0.0
            && current_health < max_health
        {
            if self.tick_timer >= 10 {
                self.tick_timer = 0;
                let spent = self.saturation.min(6.0);
                let heal = (spent / 6.0).min(max_health - current_health);
                self.add_exhaustion(spent);
                return (heal, 0.0);
            }
        } else if self.food_level >= 18 && current_health < max_health {
            if self.tick_timer >= 80 {
                self.tick_timer = 0;
                if natural_regeneration {
                    self.add_exhaustion(6.0);
                    return (1.0, 0.0);
                }
            }
        } else if self.food_level == 0 {
            if self.tick_timer >= 80 {
                self.tick_timer = 0;
                let starves = match difficulty {
                    Difficulty::Peaceful => false,
                    Difficulty::Easy => current_health > 10.0,
                    Difficulty::Normal => current_health > 1.0,
                    Difficulty::Hard => true,
                };
                if starves {
                    return (0.0, 1.0);
                }
            }
        } else if self.tick_timer >= 80 {
            self.tick_timer = 0;
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
        food.add_exhaustion(4.1);
        assert_eq!(food.saturation, 1.0);
        assert_eq!(food.food_level, 20);

        food.add_exhaustion(4.1);
        assert_eq!(food.saturation, 0.0);
        assert_eq!(food.food_level, 20);

        food.add_exhaustion(4.1);
        assert_eq!(food.saturation, 0.0);
        assert_eq!(food.food_level, 19);
    }

    #[test]
    fn fast_regeneration_when_full_and_saturated() {
        let mut food = FoodData::new(20, 5.0);
        let mut healed = 0.0;
        for _ in 0..10 {
            let (h, _) = food.tick(15.0, 20.0, true, Difficulty::Normal);
            healed += h;
        }
        assert!(
            (healed - 5.0 / 6.0).abs() < 1.0e-5,
            "saturation is spent as health"
        );
        assert_eq!(food.saturation, 4.0);
    }

    #[test]
    fn starvation_stops_at_the_easy_floor() {
        let mut food = FoodData::new(0, 0.0);
        let mut damage = 0.0;
        for _ in 0..240 {
            let (_, d) = food.tick(10.0, 20.0, true, Difficulty::Easy);
            damage += d;
        }
        assert_eq!(damage, 0.0, "easy starvation stops at ten health");
    }

    #[test]
    fn natural_regeneration_off_stops_fast_healing_too() {
        let mut food = FoodData::new(20, 5.0);
        for _ in 0..80 {
            assert_eq!(food.tick(10.0, 20.0, false, Difficulty::Normal), (0.0, 0.0));
        }
        assert_eq!(food.saturation, 5.0);
    }

    #[test]
    fn natural_regeneration_off_stops_slow_healing() {
        let mut food = FoodData::new(18, 0.0);
        for _ in 0..160 {
            assert_eq!(food.tick(10.0, 20.0, false, Difficulty::Normal), (0.0, 0.0));
        }
    }

    #[test]
    fn starvation_damage_at_zero_food() {
        let mut food = FoodData::new(0, 0.0);
        let mut damage = 0.0;
        for _ in 0..80 {
            let (_, d) = food.tick(10.0, 20.0, true, Difficulty::Normal);
            damage += d;
        }
        assert_eq!(damage, 1.0);
    }
}
