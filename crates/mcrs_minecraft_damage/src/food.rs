use mcrs_minecraft_entity::Difficulty;
use mcrs_minecraft_physics::Vitals;

/// What one hunger tick does to the health of its player.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct FoodTick {
    pub heal: f32,
    pub starve: bool,
}

/// The food level, saturation, exhaustion and hunger timer of a player (HNG-STA-001).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Food {
    pub food_level: u32,
    pub saturation: f32,
    pub exhaustion: f32,
    pub tick_timer: u32,
}

impl Default for Food {
    fn default() -> Self {
        Self {
            food_level: 20,
            saturation: 5.0,
            exhaustion: 0.0,
            tick_timer: 0,
        }
    }
}

impl Food {
    #[must_use]
    pub const fn new(food_level: u32, saturation: f32) -> Self {
        Self {
            food_level,
            saturation,
            exhaustion: 0.0,
            tick_timer: 0,
        }
    }

    pub fn add_exhaustion(&mut self, amount: f32) {
        if amount.is_finite() && amount > 0.0 {
            self.exhaustion = (self.exhaustion + amount).min(40.0);
        }
    }

    pub fn eat(&mut self, nutrition: u32, saturation: f32) {
        self.food_level = self.food_level.saturating_add(nutrition).min(20);
        self.saturation = (self.saturation + saturation).clamp(0.0, self.food_level as f32);
    }

    /// Runs the peaceful regeneration, the drain and the branch of the hunger tick
    /// (HNG-REG-004, HNG-TICK-001, HNG-REG-001, HNG-REG-002, HNG-STV-001).
    pub fn tick(
        &mut self,
        vitals: &Vitals,
        natural_regeneration: bool,
        difficulty: Difficulty,
        age: u64,
    ) -> FoodTick {
        let mut out = FoodTick::default();
        if natural_regeneration && difficulty == Difficulty::Peaceful {
            if age.is_multiple_of(20) {
                if vitals.health > 0.0 && vitals.health < vitals.max_health {
                    out.heal = 1.0;
                }
                if self.saturation < 20.0 {
                    self.saturation += 1.0;
                }
            }
            if age.is_multiple_of(10) && self.food_level < 20 {
                self.food_level += 1;
            }
        }

        if self.exhaustion > 4.0 {
            self.exhaustion -= 4.0;
            if self.saturation > 0.0 {
                self.saturation = (self.saturation - 1.0).max(0.0);
            } else if difficulty != Difficulty::Peaceful {
                self.food_level = self.food_level.saturating_sub(1);
            }
        }

        let hurt = vitals.health > 0.0 && vitals.health < vitals.max_health;
        if natural_regeneration && self.saturation > 0.0 && self.food_level >= 20 && hurt {
            self.tick_timer += 1;
            if self.tick_timer >= 10 {
                self.tick_timer = 0;
                let spent = self.saturation.min(6.0);
                out.heal = out.heal.max(spent / 6.0);
                self.add_exhaustion(spent);
            }
        } else if natural_regeneration && self.food_level >= 18 && hurt {
            self.tick_timer += 1;
            if self.tick_timer >= 80 {
                self.tick_timer = 0;
                out.heal = out.heal.max(1.0);
                self.add_exhaustion(6.0);
            }
        } else if self.food_level == 0 {
            self.tick_timer += 1;
            if self.tick_timer >= 80 {
                self.tick_timer = 0;
                out.starve = match difficulty {
                    Difficulty::Peaceful | Difficulty::Easy => vitals.health > 10.0,
                    Difficulty::Normal => vitals.health > 1.0,
                    Difficulty::Hard => true,
                };
            }
        } else {
            self.tick_timer = 0;
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn vitals(health: f32) -> Vitals {
        Vitals {
            health,
            ..Vitals::default()
        }
    }

    #[test]
    fn saturated_heal_costs_six_exhaustion_each_ten_ticks() {
        let mut food = Food::default();
        let mut vitals = vitals(10.0);
        let mut healed = 0.0;
        for tick in 0..10 {
            let out = food.tick(&vitals, true, Difficulty::Normal, tick);
            healed += out.heal;
            vitals.health += out.heal;
        }
        assert_eq!(healed, 1.0);
        assert_eq!(food.exhaustion, 6.0);
    }

    #[test]
    fn starvation_stops_at_the_difficulty_floor() {
        let mut food = Food::new(0, 0.0);
        let mut vitals = vitals(10.5);
        let mut hits = 0;
        for tick in 0..80 {
            let out = food.tick(&vitals, true, Difficulty::Easy, tick);
            if out.starve {
                hits += 1;
                vitals.health -= 1.0;
            }
        }
        assert_eq!(hits, 1);
        assert_eq!(vitals.health, 9.5);
    }

    #[test]
    fn peaceful_regeneration_heals_and_refills() {
        let mut food = Food::new(10, 1.0);
        let vitals = vitals(10.0);
        let out = food.tick(&vitals, true, Difficulty::Peaceful, 0);
        assert_eq!(out.heal, 1.0);
        assert_eq!(food.saturation, 2.0);
        let out = food.tick(&vitals, true, Difficulty::Peaceful, 10);
        assert_eq!(out.heal, 0.0);
        assert_eq!(food.food_level, 11);
    }
}
