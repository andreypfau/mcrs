use crate::Dimensions;
use crate::Scaling;

/// The posture of an entity, which selects its dimensions.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Pose {
    #[default]
    Standing,
    FallFlying,
    Sleeping,
    Swimming,
    SpinAttack,
    Crouching,
    Dying,
}

impl Pose {
    /// The dimensions of a player in this pose, before the `scale` attribute.
    #[must_use]
    pub fn player_dimensions(self) -> Dimensions {
        match self {
            Pose::Standing => Dimensions::scalable(0.6, 1.8).with_eye_height(1.62),
            Pose::Crouching => Dimensions::scalable(0.6, 1.5).with_eye_height(1.27),
            Pose::Swimming | Pose::FallFlying | Pose::SpinAttack => {
                Dimensions::scalable(0.6, 0.6).with_eye_height(0.4)
            }
            Pose::Sleeping => Dimensions {
                width: 0.2,
                height: 0.2,
                eye_height: 0.2,
                scaling: Scaling::Fixed,
            },
            Pose::Dying => Dimensions {
                width: 0.2,
                height: 0.2,
                eye_height: 1.62,
                scaling: Scaling::Fixed,
            },
        }
    }
}
