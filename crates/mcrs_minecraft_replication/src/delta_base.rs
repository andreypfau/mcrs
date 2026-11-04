use crate::encode::encode;
use bytes::Bytes;
use glam::DVec3;
use mcrs_minecraft_core::Look;
use mcrs_minecraft_protocol::{Result, VarInt};
use mcrs_minecraft_protocol_game::ByteAngle;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundEntityPositionSync, ClientboundMoveEntityPos, ClientboundMoveEntityPosRot,
    ClientboundMoveEntityRot, ClientboundRotateHead, PositionPath, VecDelta,
};

/// The steps of one block in a relative move.
const STEPS_PER_BLOCK: f64 = 4096.0;

/// Where a subject is and where it looks, in degrees.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    pub position: DVec3,
    pub look: Look,
}

/// What the viewers of a subject last received for it. All viewers of a subject share one base,
/// so a relative move of the subject is the same packet for each viewer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeltaBase {
    pub position: DVec3,
    pub yaw: ByteAngle,
    pub pitch: ByteAngle,
    pub head_yaw: ByteAngle,
}

/// Whether a move must carry the absolute position.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Keyframe {
    Due,
    NotDue,
}

/// The packets that take a client from one delta base to the next; both are `None` when nothing
/// that a client sees changed.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
#[must_use]
pub struct Motion {
    pub body: Option<Bytes>,
    pub head: Option<Bytes>,
}

impl Motion {
    pub fn is_empty(&self) -> bool {
        self.body.is_none() && self.head.is_none()
    }
}

impl DeltaBase {
    pub fn new(pose: &Pose) -> Self {
        let yaw = ByteAngle::from_degrees(pose.look.yaw);
        Self {
            position: pose.position,
            yaw,
            pitch: ByteAngle::from_degrees(pose.look.pitch),
            head_yaw: yaw,
        }
    }

    /// Moves the base to `pose` and returns the packets that move the subject `id` on a client
    /// from the old base to the new one. A delta that does not fit a relative move, or a due
    /// keyframe, sends the absolute position. The base stays as it is when an encoding fails.
    pub fn advance(&mut self, id: VarInt, pose: &Pose, keyframe: Keyframe) -> Result<Motion> {
        let next = Self::new(pose);
        let delta = position_delta(self.position, next.position);
        let turned = (next.yaw, next.pitch) != (self.yaw, self.pitch);
        let body = match (keyframe, delta) {
            (Keyframe::Due, _) | (_, None) => Some(encode(&ClientboundEntityPositionSync {
                entity_id: id,
                position: PositionPath::Linear(next.position),
                look: pose.look,
                on_ground: true,
            })?),
            (Keyframe::NotDue, Some([0, 0, 0])) if turned => {
                Some(encode(&ClientboundMoveEntityRot {
                    entity_id: id,
                    y_rot: next.yaw,
                    x_rot: next.pitch,
                    on_ground: true,
                })?)
            }
            (Keyframe::NotDue, Some([0, 0, 0])) => None,
            (Keyframe::NotDue, Some(delta)) if turned => {
                Some(encode(&ClientboundMoveEntityPosRot {
                    entity_id: id,
                    delta: VecDelta::Linear(delta),
                    y_rot: next.yaw,
                    x_rot: next.pitch,
                    on_ground: true,
                })?)
            }
            (Keyframe::NotDue, Some(delta)) => Some(encode(&ClientboundMoveEntityPos {
                entity_id: id,
                delta: VecDelta::Linear(delta),
                on_ground: true,
            })?),
        };
        let head = if next.head_yaw == self.head_yaw {
            None
        } else {
            Some(encode(&ClientboundRotateHead {
                entity_id: id,
                y_head_rot: next.head_yaw,
            })?)
        };
        *self = next;
        Ok(Motion { body, head })
    }
}

/// The delta from `from` to `to` in steps, or `None` when an axis moves more than a relative
/// move holds. Both ends round to the step grid, as the client does, so the rounding of one
/// delta never carries into the next.
fn position_delta(from: DVec3, to: DVec3) -> Option<[i16; 3]> {
    let step = |from: f64, to: f64| {
        let steps = (to * STEPS_PER_BLOCK).round() - (from * STEPS_PER_BLOCK).round();
        (steps >= f64::from(i16::MIN) && steps <= f64::from(i16::MAX)).then_some(steps as i16)
    };
    Some([
        step(from.x, to.x)?,
        step(from.y, to.y)?,
        step(from.z, to.z)?,
    ])
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(x: f64, yaw: f32) -> Pose {
        Pose {
            position: DVec3::new(x, 64.0, 0.0),
            look: Look { yaw, pitch: 0.0 },
        }
    }

    fn sync(id: VarInt, x: f64) -> Result<Bytes> {
        encode(&ClientboundEntityPositionSync {
            entity_id: id,
            position: PositionPath::Linear(DVec3::new(x, 64.0, 0.0)),
            look: Look::default(),
            on_ground: true,
        })
    }

    #[test]
    fn small_steps_are_relative_and_a_long_jump_is_absolute() -> Result<()> {
        let mut base = DeltaBase::new(&at(0.0, 0.0));
        let id = VarInt(3);

        let walked = base.advance(id, &at(0.5, 0.0), Keyframe::NotDue)?;
        let expected = encode(&ClientboundMoveEntityPos {
            entity_id: id,
            delta: VecDelta::Linear([2048, 0, 0]),
            on_ground: true,
        })?;
        assert_eq!(walked.body, Some(expected));
        assert!(walked.head.is_none(), "the head did not turn");

        let jumped = base.advance(id, &at(20.0, 0.0), Keyframe::NotDue)?;
        assert_eq!(jumped.body, Some(sync(id, 20.0)?));
        assert!(
            base.advance(id, &at(20.0, 0.0), Keyframe::NotDue)?
                .is_empty()
        );
        Ok(())
    }

    #[test]
    fn a_due_keyframe_is_absolute_even_for_a_small_step() -> Result<()> {
        let mut base = DeltaBase::new(&at(0.0, 0.0));

        let motion = base.advance(VarInt(3), &at(0.5, 0.0), Keyframe::Due)?;

        assert_eq!(motion.body, Some(sync(VarInt(3), 0.5)?));
        Ok(())
    }

    #[test]
    fn a_turn_sends_the_look_in_degrees_and_the_head() -> Result<()> {
        let mut base = DeltaBase::new(&at(0.0, 0.0));
        let angle = ByteAngle::from_degrees(90.0);

        let turned = base.advance(VarInt(3), &at(0.0, 90.0), Keyframe::NotDue)?;

        let rotate = encode(&ClientboundMoveEntityRot {
            entity_id: VarInt(3),
            y_rot: angle,
            x_rot: ByteAngle(0),
            on_ground: true,
        })?;
        let head = encode(&ClientboundRotateHead {
            entity_id: VarInt(3),
            y_head_rot: angle,
        })?;
        let expected = Motion {
            body: Some(rotate),
            head: Some(head),
        };
        assert_eq!(turned, expected);
        Ok(())
    }
}
