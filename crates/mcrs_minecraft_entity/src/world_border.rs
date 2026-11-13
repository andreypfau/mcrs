/// The settings of the world border of one dimension (ENV-BRD-002).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct WorldBorderSettings {
    pub center_x: f64,
    pub center_z: f64,
    /// The full diameter of the border.
    pub size: f64,
    pub safe_zone: f64,
    pub damage_per_block: f64,
}

impl Default for WorldBorderSettings {
    fn default() -> Self {
        Self {
            center_x: 0.0,
            center_z: 0.0,
            size: 59_999_968.0,
            safe_zone: 5.0,
            damage_per_block: 0.2,
        }
    }
}

impl WorldBorderSettings {
    #[must_use]
    pub fn min_x(&self) -> f64 {
        self.center_x - self.size / 2.0
    }

    #[must_use]
    pub fn max_x(&self) -> f64 {
        self.center_x + self.size / 2.0
    }

    #[must_use]
    pub fn min_z(&self) -> f64 {
        self.center_z - self.size / 2.0
    }

    #[must_use]
    pub fn max_z(&self) -> f64 {
        self.center_z + self.size / 2.0
    }
}
