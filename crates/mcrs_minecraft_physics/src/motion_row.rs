use crate::Category;
use crate::InsideEffect;
use crate::StuckIgnore;
use mcrs_minecraft_pose::Dimensions;

const LIFETIME: u32 = 6000;

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct MotionRow {
    pub category: Category,
    pub dimensions: Dimensions,
    pub gravity: f64,
    pub stuck_ignore: StuckIgnore,
    pub powder_snow_needs_inside: bool,
    pub powder_snow_solid: bool,
    pub block_bounce_factor: f32,
    pub step_height: f64,
    pub lifetime: Option<u32>,
}

impl MotionRow {
    #[must_use]
    pub fn living(category: Category, dimensions: Dimensions) -> Self {
        Self {
            category,
            dimensions,
            gravity: 0.08,
            stuck_ignore: StuckIgnore::None,
            powder_snow_needs_inside: true,
            powder_snow_solid: false,
            block_bounce_factor: 1.0,
            step_height: 0.6,
            lifetime: None,
        }
    }

    #[must_use]
    pub fn other(category: Category, dimensions: Dimensions, gravity: f64) -> Self {
        Self {
            category,
            dimensions,
            gravity,
            stuck_ignore: StuckIgnore::One(InsideEffect::SweetBerryBush),
            powder_snow_needs_inside: false,
            powder_snow_solid: false,
            block_bounce_factor: 0.8,
            step_height: 0.0,
            lifetime: None,
        }
    }

    #[must_use]
    pub fn zombie() -> Self {
        Self::living(
            Category::Walker,
            Dimensions::scalable(0.6, 1.95).with_eye_height(1.74),
        )
    }

    #[must_use]
    pub fn armor_stand() -> Self {
        Self {
            step_height: 0.0,
            ..Self::living(
                Category::Walker,
                Dimensions::scalable(0.5, 1.975).with_eye_height(1.7775),
            )
        }
    }

    #[must_use]
    pub fn item() -> Self {
        Self {
            lifetime: Some(LIFETIME),
            ..Self::other(
                Category::Item,
                Dimensions::scalable(0.25, 0.25).with_eye_height(0.2125),
                0.04,
            )
        }
    }

    #[must_use]
    pub fn falling_block() -> Self {
        Self {
            powder_snow_solid: true,
            ..Self::other(
                Category::FallingBlock,
                Dimensions::scalable(0.98, 0.98),
                0.04,
            )
        }
    }

    #[must_use]
    pub fn primed_tnt() -> Self {
        Self::other(
            Category::PrimedTnt,
            Dimensions::scalable(0.98, 0.98).with_eye_height(0.15),
            0.04,
        )
    }

    #[must_use]
    pub fn experience_orb() -> Self {
        Self {
            lifetime: Some(LIFETIME),
            ..Self::other(
                Category::ExperienceOrb,
                Dimensions::scalable(0.5, 0.5),
                0.03,
            )
        }
    }
}
