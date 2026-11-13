/// The experience values of a player (XP-STA-001).
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Experience {
    pub level: i32,
    pub progress: f32,
    pub total: i32,
    pub score: i32,
}

/// The points that the level `level` needs (XP-LVL-001).
#[must_use]
pub fn points_for_level(level: i32) -> i32 {
    match level {
        0..=14 => 2 * level + 7,
        15..=29 => 5 * level - 38,
        _ => 9 * level - 158,
    }
}

/// The least number of points that reaches the level `level` (XP-LVL-002).
#[must_use]
pub fn total_points_for_level(level: i32) -> i32 {
    match level {
        0..=16 => level * level + 6 * level,
        17..=31 => (5 * level * level - 81 * level + 720) / 2,
        _ => (9 * level * level - 325 * level + 4440) / 2,
    }
}

impl Experience {
    #[must_use]
    pub const fn new(level: i32, progress: f32) -> Self {
        Self {
            level,
            progress,
            total: 0,
            score: 0,
        }
    }

    /// Adds `amount` points (XP-ADD-001).
    pub fn add_points(&mut self, amount: i32) {
        self.score = self.score.saturating_add(amount);
        let need = points_for_level(self.level);
        if need > 0 {
            self.progress += amount as f32 / need as f32;
        }
        self.total = self.total.saturating_add(amount).max(0);
        while self.progress < 0.0 {
            let before = self.level;
            self.add_levels(-1);
            let need = points_for_level(self.level);
            self.progress = if before > 0 && need > 0 {
                1.0 + self.progress * points_for_level(before) as f32 / need as f32
            } else {
                0.0
            };
        }
        while self.progress >= 1.0 {
            let need = points_for_level(self.level);
            self.progress = if need > 0 {
                (self.progress - 1.0) * need as f32
            } else {
                0.0
            };
            self.add_levels(1);
            let need = points_for_level(self.level);
            if need > 0 {
                self.progress /= need as f32;
            }
        }
    }

    pub fn add_levels(&mut self, amount: i32) {
        self.level = self.level.saturating_add(amount);
        if self.level < 0 {
            self.level = 0;
            self.progress = 0.0;
            self.total = 0;
        }
    }

    /// Sets the points of the current level, or does nothing when `points` is not below the
    /// points of the level (XP-ADD-003).
    pub fn set_points(&mut self, points: i32) -> bool {
        let need = points_for_level(self.level);
        if points < 0 || points >= need {
            return false;
        }
        self.progress = points as f32 / need as f32;
        true
    }

    pub fn set_level(&mut self, level: i32) -> bool {
        let old = self.level;
        self.level = level;
        old != level
    }

    #[must_use]
    pub fn points(&self) -> i32 {
        (self.progress * points_for_level(self.level) as f32).floor() as i32
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn levels_follow_the_table() {
        assert_eq!(points_for_level(0), 7);
        assert_eq!(points_for_level(15), 37);
        assert_eq!(points_for_level(30), 112);
        assert_eq!(total_points_for_level(0), 0);
        assert_eq!(total_points_for_level(16), 352);
        assert_eq!(total_points_for_level(17), 394);
        assert_eq!(total_points_for_level(32), 1628);
    }

    #[test]
    fn seven_points_at_level_zero_give_level_one() {
        let mut xp = Experience::default();
        xp.add_points(7);
        assert_eq!(xp.level, 1);
        assert_eq!(xp.progress, 0.0);
        assert_eq!(xp.total, 7);
        assert_eq!(xp.score, 7);
    }

    #[test]
    fn a_negative_add_goes_down_a_level() {
        let mut xp = Experience::new(1, 0.0);
        xp.add_points(-1);
        assert_eq!(xp.level, 0);
        assert_eq!(xp.total, 0);
    }
}
