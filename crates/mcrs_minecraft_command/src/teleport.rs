use glam::{BVec3, DVec3};
use mcrs_minecraft_core::mth::wrap_degrees;
use mcrs_minecraft_protocol::{Look, PositionFlag, PositionFlags};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub position: DVec3,
    pub yaw: f32,
    pub pitch: f32,
}

/// What `player_position` carries for one move, and the rotation the move leaves the victim in.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RelativeMove {
    pub position: DVec3,
    pub look: Look,
    pub flags: PositionFlags,
    pub final_look: Look,
}

/// The move of a victim from `from` to `to` (CMD-TP-005 step 2, CMD-TP-006): each relative
/// value is the destination minus the current value, the yaw and the pitch to apply are wrapped,
/// and the final pitch is clamped. A victim that changes dimension gets the absolute position
/// and keeps only the velocity flags of the relative axes (CMD-TP-013).
pub fn relative_move(
    from: Pose,
    to: Pose,
    relative_position: [bool; 3],
    relative_rotation: [bool; 2],
    changes_dimension: bool,
) -> RelativeMove {
    let mut flags = PositionFlags::default();
    let axes = [
        (PositionFlag::X, PositionFlag::DeltaX),
        (PositionFlag::Y, PositionFlag::DeltaY),
        (PositionFlag::Z, PositionFlag::DeltaZ),
    ];
    for ((axis, velocity), relative) in axes.into_iter().zip(relative_position) {
        if relative && !changes_dimension {
            flags.insert(axis);
        }
        if relative {
            flags.insert(velocity);
        }
    }
    let position = DVec3::select(
        BVec3::from(relative_position) & BVec3::splat(!changes_dimension),
        to.position - from.position,
        to.position,
    );
    let [yaw_relative, pitch_relative] = relative_rotation;
    let mut apply = |to: f32, from: f32, relative: bool, flag: PositionFlag| {
        if relative {
            flags.insert(flag);
            (wrap_degrees(to - from), from + wrap_degrees(to - from))
        } else {
            (wrap_degrees(to), wrap_degrees(to))
        }
    };
    let (yaw, final_yaw) = apply(to.yaw, from.yaw, yaw_relative, PositionFlag::YRot);
    let (pitch, final_pitch) = apply(to.pitch, from.pitch, pitch_relative, PositionFlag::XRot);
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
        yaw: 30.0,
        pitch: 10.0,
    };

    #[test]
    fn a_move_wraps_what_it_applies_and_clamps_the_final_pitch() {
        let absolute = |yaw, pitch| Pose { yaw, pitch, ..AT };
        let cases = [
            (absolute(0.0, 270.0), [false; 2], (0.0, -90.0), (0.0, -90.0)),
            (absolute(0.0, 100.0), [false; 2], (0.0, 100.0), (0.0, 90.0)),
            (absolute(400.0, 0.0), [false; 2], (40.0, 0.0), (40.0, 0.0)),
            (absolute(30.0, 10.0), [true; 2], (0.0, 0.0), (30.0, 10.0)),
            (absolute(20.0, 95.0), [true; 2], (-10.0, 85.0), (20.0, 90.0)),
        ];
        for (to, relative, look, final_look) in cases {
            let step = relative_move(AT, to, [false; 3], relative, false);
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
        let step = relative_move(AT, to, [true, true, false], [false; 2], false);
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

        let step = relative_move(AT, to, [true, true, false], [false; 2], true);
        assert_eq!(step.position, to.position);
        assert_eq!(
            step.flags,
            PositionFlags::from_iter([PositionFlag::DeltaX, PositionFlag::DeltaY])
        );
    }
}
