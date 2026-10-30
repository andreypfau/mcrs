use bevy_math::{BVec3, DVec3};
use mcrs_minecraft_core::Look;
use mcrs_minecraft_core::mth::wrap_degrees;
use mcrs_minecraft_protocol::{PositionFlag, PositionFlags};

/// The position and the rotation of a victim before or after a teleport.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub position: DVec3,
    pub look: Look,
}

/// Which angles of a teleport are relative to the current rotation of the victim.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RelativeRotation {
    pub yaw: bool,
    pub pitch: bool,
}

/// Whether a teleport moves the victim into another dimension.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DimensionChange {
    Stays,
    Changes,
}

/// What `player_position` carries for one move, and the rotation the move leaves the victim in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RelativeMove {
    pub position: DVec3,
    pub look: Look,
    pub flags: PositionFlags,
    pub final_look: Look,
}

/// The move of a victim from `from` to `to`: each relative
/// value is the destination minus the current value, the yaw and the pitch to apply are wrapped,
/// and the final pitch is clamped. A victim that changes dimension gets the absolute position
/// and keeps only the velocity flags of the relative axes.
#[must_use]
pub fn relative_move(
    from: Pose,
    to: Pose,
    relative_position: BVec3,
    relative_rotation: RelativeRotation,
    dimension_change: DimensionChange,
) -> RelativeMove {
    let stays = dimension_change == DimensionChange::Stays;
    let mut flags = PositionFlags::default();
    let axes = [
        (PositionFlag::X, PositionFlag::DeltaX),
        (PositionFlag::Y, PositionFlag::DeltaY),
        (PositionFlag::Z, PositionFlag::DeltaZ),
    ];
    for ((axis, velocity), relative) in axes.into_iter().zip(<[bool; 3]>::from(relative_position)) {
        if relative && stays {
            flags.insert(axis);
        }
        if relative {
            flags.insert(velocity);
        }
    }
    let position = DVec3::select(
        relative_position & BVec3::splat(stays),
        to.position - from.position,
        to.position,
    );
    let mut apply = |to: f32, from: f32, relative: bool, flag: PositionFlag| {
        if relative {
            flags.insert(flag);
            (wrap_degrees(to - from), from + wrap_degrees(to - from))
        } else {
            (wrap_degrees(to), wrap_degrees(to))
        }
    };
    let (yaw, final_yaw) = apply(
        to.look.yaw,
        from.look.yaw,
        relative_rotation.yaw,
        PositionFlag::YRot,
    );
    let (pitch, final_pitch) = apply(
        to.look.pitch,
        from.look.pitch,
        relative_rotation.pitch,
        PositionFlag::XRot,
    );
    RelativeMove {
        position,
        look: Look { yaw, pitch },
        flags,
        final_look: Look {
            yaw: final_yaw,
            pitch: final_pitch.clamp(-90.0, 90.0),
        },
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const AT: Pose = Pose {
        position: DVec3::new(0.5, 64.0, 0.5),
        look: Look {
            yaw: 30.0,
            pitch: 10.0,
        },
    };
    const ABSOLUTE: RelativeRotation = RelativeRotation {
        yaw: false,
        pitch: false,
    };
    const RELATIVE: RelativeRotation = RelativeRotation {
        yaw: true,
        pitch: true,
    };

    #[test]
    fn a_move_wraps_what_it_applies_and_clamps_the_final_pitch() {
        let absolute = |yaw, pitch| Pose {
            look: Look { yaw, pitch },
            ..AT
        };
        let cases = [
            (absolute(0.0, 270.0), ABSOLUTE, (0.0, -90.0), (0.0, -90.0)),
            (absolute(0.0, 100.0), ABSOLUTE, (0.0, 100.0), (0.0, 90.0)),
            (absolute(400.0, 0.0), ABSOLUTE, (40.0, 0.0), (40.0, 0.0)),
            (absolute(30.0, 10.0), RELATIVE, (0.0, 0.0), (30.0, 10.0)),
            (absolute(20.0, 95.0), RELATIVE, (-10.0, 85.0), (20.0, 90.0)),
        ];
        for (to, relative, look, final_look) in cases {
            let step = relative_move(AT, to, BVec3::FALSE, relative, DimensionChange::Stays);
            assert_eq!((step.look.yaw, step.look.pitch), look, "{to:?}");
            assert_eq!(
                (step.final_look.yaw, step.final_look.pitch),
                final_look,
                "{to:?}"
            );
        }
    }

    #[test]
    fn a_relative_axis_carries_the_difference_and_its_two_flags() {
        let to = Pose {
            position: DVec3::new(1.5, 64.0, 3.5),
            ..AT
        };
        let axes = BVec3::new(true, true, false);
        let step = relative_move(AT, to, axes, ABSOLUTE, DimensionChange::Stays);
        assert_eq!(step.position, DVec3::new(1.0, 0.0, 3.5));
        assert_eq!(
            step.flags,
            PositionFlags::from_iter([
                PositionFlag::X,
                PositionFlag::DeltaX,
                PositionFlag::Y,
                PositionFlag::DeltaY
            ])
        );

        let step = relative_move(AT, to, axes, ABSOLUTE, DimensionChange::Changes);
        assert_eq!(step.position, to.position);
        assert_eq!(
            step.flags,
            PositionFlags::from_iter([PositionFlag::DeltaX, PositionFlag::DeltaY])
        );
    }
}
