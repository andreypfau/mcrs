use bevy_ecs::message::{Message, MessageWriter};
use bevy_ecs::prelude::*;

use crate::position::CombatPosition;

/// Calculates the experience points needed to advance from `level` to `level + 1`.
#[must_use]
pub const fn points_for_level(level: i32) -> i32 {
    if level < 0 {
        0
    } else if level <= 15 {
        2 * level + 7
    } else if level <= 30 {
        5 * level - 38
    } else {
        9 * level - 158
    }
}

/// Calculates the total experience points required from level 0 to reach `level`.
#[must_use]
pub fn total_points_for_level(level: i32) -> i32 {
    if level <= 0 {
        0
    } else if level <= 16 {
        level * level + 6 * level
    } else if level <= 31 {
        let l = level as f64;
        (2.5 * l * l - 40.5 * l + 360.0).floor() as i32
    } else {
        let l = level as f64;
        (4.5 * l * l - 162.5 * l + 2220.0).floor() as i32
    }
}

/// Player's experience component.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct PlayerExperience {
    /// Current experience level.
    pub level: i32,
    /// Progress towards next level, in range `[0.0, 1.0)`.
    pub progress: f32,
    /// Total experience points collected over lifetime.
    pub total_experience: i32,
}

impl Default for PlayerExperience {
    fn default() -> Self {
        Self {
            level: 0,
            progress: 0.0,
            total_experience: 0,
        }
    }
}

impl PlayerExperience {
    #[must_use]
    pub const fn new(level: i32, progress: f32, total_experience: i32) -> Self {
        Self {
            level,
            progress,
            total_experience,
        }
    }

    /// Adds experience points to this player, recalculating levels and progress.
    pub fn add_experience(&mut self, amount: i32) {
        if amount <= 0 {
            return;
        }

        self.total_experience = self.total_experience.saturating_add(amount);

        let cur_needed = points_for_level(self.level);
        let mut cur_points = if cur_needed > 0 {
            (self.progress * cur_needed as f32).round() as i32 + amount
        } else {
            amount
        };

        while cur_points >= points_for_level(self.level) {
            let cap = points_for_level(self.level);
            if cap <= 0 {
                break;
            }
            cur_points -= cap;
            self.level = self.level.saturating_add(1);
        }

        let next_needed = points_for_level(self.level);
        self.progress = if next_needed > 0 {
            (cur_points as f32 / next_needed as f32).clamp(0.0, 1.0)
        } else {
            0.0
        };
    }

    /// Spends experience levels (e.g. for enchanting or anvil repair).
    /// Returns `true` if levels were successfully deducted.
    pub fn spend_levels(&mut self, levels_to_spend: i32) -> bool {
        if levels_to_spend <= 0 {
            return true;
        }
        if self.level < levels_to_spend {
            return false;
        }

        self.level -= levels_to_spend;

        // Recalculate total points
        let base_points = total_points_for_level(self.level);
        let next_needed = points_for_level(self.level);
        let fractional_points = (self.progress * next_needed as f32).round() as i32;
        self.total_experience = base_points.saturating_add(fractional_points);

        true
    }
}

/// An experience orb in the world.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct ExperienceOrb {
    pub value: i32,
    pub age_ticks: u32,
}

impl ExperienceOrb {
    #[must_use]
    pub const fn new(value: i32) -> Self {
        Self {
            value,
            age_ticks: 0,
        }
    }
}

/// Message emitted when a player gains experience.
#[derive(Message, Clone, Copy, Debug, PartialEq)]
pub struct ExperienceGainedEvent {
    pub player: Entity,
    pub amount: i32,
    pub new_level: i32,
    pub new_progress: f32,
}

/// Ticks experience orbs, aging them and picking them up when within 1.25 blocks of a player.
pub fn pickup_experience_orbs(
    mut commands: Commands,
    mut players: Query<(Entity, &mut PlayerExperience, &CombatPosition)>,
    mut orbs: Query<(Entity, &mut ExperienceOrb, &mut CombatPosition), Without<PlayerExperience>>,
    mut events: MessageWriter<ExperienceGainedEvent>,
) {
    const PICKUP_RADIUS_SQ: f64 = 1.25 * 1.25;

    for (orb_entity, mut orb, mut orb_pos) in &mut orbs {
        orb.age_ticks = orb.age_ticks.saturating_add(1);
        if orb.age_ticks >= 6000 {
            commands.entity(orb_entity).despawn();
            continue;
        }

        let nearest = players
            .iter()
            .map(|(_, _, position)| position.0)
            .filter(|position| position.distance_squared(orb_pos.0) <= 64.0)
            .min_by(|left, right| {
                left.distance_squared(orb_pos.0)
                    .total_cmp(&right.distance_squared(orb_pos.0))
            });
        if let Some(target) = nearest {
            let direction = target - orb_pos.0;
            if direction.length_squared() > 1.0e-9 {
                orb_pos.0 += direction.normalize() * 0.1;
            }
        }

        for (player_entity, mut player_exp, player_pos) in &mut players {
            if player_pos.0.distance_squared(orb_pos.0) <= PICKUP_RADIUS_SQ {
                let amount = orb.value;
                player_exp.add_experience(amount);
                events.write(ExperienceGainedEvent {
                    player: player_entity,
                    amount,
                    new_level: player_exp.level,
                    new_progress: player_exp.progress,
                });
                commands.entity(orb_entity).despawn();
                break;
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_points_for_level() {
        assert_eq!(points_for_level(0), 7);
        assert_eq!(points_for_level(15), 37);
        assert_eq!(points_for_level(16), 42);
        assert_eq!(points_for_level(30), 112);
        assert_eq!(points_for_level(31), 121);
    }

    #[test]
    fn test_add_experience() {
        let mut exp = PlayerExperience::default();
        // Level 0 requires 7 points
        exp.add_experience(7);
        assert_eq!(exp.level, 1);
        assert_eq!(exp.progress, 0.0);
        assert_eq!(exp.total_experience, 7);

        // Level 1 requires 9 points. Add 4 points
        exp.add_experience(4);
        assert_eq!(exp.level, 1);
        assert!((exp.progress - (4.0 / 9.0)).abs() < 0.05);
    }

    #[test]
    fn test_spend_levels() {
        let mut exp = PlayerExperience::default();
        exp.add_experience(100);
        let start_level = exp.level;
        assert!(start_level > 3);

        assert!(exp.spend_levels(2));
        assert_eq!(exp.level, start_level - 2);

        // Cannot spend more than available
        assert!(!exp.spend_levels(100));
    }
}
