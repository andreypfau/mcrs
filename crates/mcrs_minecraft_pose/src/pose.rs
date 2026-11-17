use crate::Dimensions;
use crate::Scaling;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
#[cfg_attr(feature = "bevy", derive(bevy_ecs::component::Component))]
pub enum Pose {
    #[default]
    Standing,
    FallFlying,
    Sleeping,
    Swimming,
    SpinAttack,
    Crouching,
    LongJumping,
    Dying,
    Croaking,
    UsingTongue,
    Sitting,
    Roaring,
    Sniffing,
    Emerging,
    Digging,
    Sliding,
    Shooting,
    Inhaling,
}

impl Pose {
    #[must_use]
    pub fn player_dimensions(self) -> Dimensions {
        match self {
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
            Pose::Standing
            | Pose::LongJumping
            | Pose::Croaking
            | Pose::UsingTongue
            | Pose::Sitting
            | Pose::Roaring
            | Pose::Sniffing
            | Pose::Emerging
            | Pose::Digging
            | Pose::Sliding
            | Pose::Shooting
            | Pose::Inhaling => Dimensions::scalable(0.6, 1.8).with_eye_height(1.62),
        }
    }
}
