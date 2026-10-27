use glam::{BVec3, DVec3};
use mcrs_minecraft_core::mth::wrap_degrees;
use mcrs_minecraft_protocol::{Look, PositionFlag};

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub position: DVec3,
    pub yaw: f32,
    pub pitch: f32,
}

/// What `player_position` carries for one move, and the pose the move leaves the victim in.
#[derive(Clone, Debug, PartialEq)]
pub struct RelativeMove {
    pub position: DVec3,
    pub look: Look,
    pub flags: Vec<PositionFlag>,
    pub result: Pose,
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
    let mut flags = Vec::new();
    let axes = [
        (PositionFlag::X, PositionFlag::DeltaX),
        (PositionFlag::Y, PositionFlag::DeltaY),
        (PositionFlag::Z, PositionFlag::DeltaZ),
    ];
    for ((axis, velocity), relative) in axes.into_iter().zip(relative_position) {
        if relative && !changes_dimension {
            flags.push(axis);
        }
        if relative {
            flags.push(velocity);
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
            flags.push(flag);
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
        result: Pose {
            position: to.position,
            yaw: final_yaw,
            pitch: final_pitch.clamp(-90.0, 90.0),
        },
    }
}

/// The yaw and the pitch that turn a look from `from` towards `to` (CMD-TP-011).
pub fn look_at(from: DVec3, to: DVec3) -> (f32, f32) {
    let d = to - from;
    let horizontal = (d.x * d.x + d.z * d.z).sqrt();
    let pitch = -(d.y.atan2(horizontal).to_degrees()) as f32;
    let yaw = (d.z.atan2(d.x).to_degrees() - 90.0) as f32;
    (wrap_degrees(yaw), wrap_degrees(pitch))
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
        for (to, relative, look, result) in cases {
            let step = relative_move(AT, to, [false; 3], relative, false);
            assert_eq!((step.look.yaw, step.look.pitch), look, "{to:?}");
            assert_eq!((step.result.yaw, step.result.pitch), result, "{to:?}");
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
            [
                PositionFlag::X,
                PositionFlag::DeltaX,
                PositionFlag::Y,
                PositionFlag::DeltaY
            ]
        );
        assert_eq!(step.result.position, to.position);

        let step = relative_move(AT, to, [true, true, false], [false; 2], true);
        assert_eq!(step.position, to.position);
        assert_eq!(step.flags, [PositionFlag::DeltaX, PositionFlag::DeltaY]);
    }
}
