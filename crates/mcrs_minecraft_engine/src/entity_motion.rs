//! The moves of a subject relative to one delta base that all its viewers share.

use crate::bus::PacketPayload;
use bevy_math::DVec3;
use mcrs_minecraft_engine_world::entity::physics::Transform;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::ByteAngle;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundEntityPositionSync, ClientboundMoveEntityPos, ClientboundMoveEntityPosRot,
    ClientboundMoveEntityRot, ClientboundRotateHead, PositionPath, VecDelta,
};

/// The steps of one block in a relative move.
const STEPS_PER_BLOCK: f64 = 4096.0;

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

/// The packets that take a client from one delta base to the next; both are empty when nothing
/// that a client sees changed.
#[derive(Clone, Debug, Default)]
#[must_use]
pub struct Motion {
    pub body: Option<PacketPayload>,
    pub head: Option<PacketPayload>,
}

impl Motion {
    pub fn is_empty(&self) -> bool {
        self.body.is_none() && self.head.is_none()
    }
}

impl DeltaBase {
    pub fn new(transform: &Transform) -> Self {
        let yaw = ByteAngle::from_degrees(transform.rotation.yaw());
        Self {
            position: transform.translation,
            yaw,
            pitch: ByteAngle::from_degrees(transform.rotation.pitch()),
            head_yaw: yaw,
        }
    }

    /// Moves the base to `transform` and returns the packets that move the subject `id` on a
    /// client from the old base to the new one. A delta that does not fit a relative move, or a
    /// due keyframe, sends the absolute position.
    pub fn advance(&mut self, id: VarInt, transform: &Transform, keyframe: Keyframe) -> Motion {
        let next = Self::new(transform);
        let delta = position_delta(self.position, next.position);
        let turned = (next.yaw, next.pitch) != (self.yaw, self.pitch);
        let body = match (keyframe, delta) {
            (Keyframe::Due, _) | (_, None) => Some(PacketPayload::EntityPosSync(
                ClientboundEntityPositionSync {
                    entity_id: id,
                    position: PositionPath::Linear(next.position),
                    look: transform.rotation.into(),
                    on_ground: true,
                },
            )),
            (Keyframe::NotDue, Some([0, 0, 0])) if turned => {
                Some(PacketPayload::EntityRotate(ClientboundMoveEntityRot {
                    entity_id: id,
                    y_rot: next.yaw,
                    x_rot: next.pitch,
                    on_ground: true,
                }))
            }
            (Keyframe::NotDue, Some([0, 0, 0])) => None,
            (Keyframe::NotDue, Some(delta)) if turned => Some(PacketPayload::EntityMoveRotate(
                ClientboundMoveEntityPosRot {
                    entity_id: id,
                    delta: VecDelta::Linear(delta),
                    y_rot: next.yaw,
                    x_rot: next.pitch,
                    on_ground: true,
                },
            )),
            (Keyframe::NotDue, Some(delta)) => {
                Some(PacketPayload::EntityMove(ClientboundMoveEntityPos {
                    entity_id: id,
                    delta: VecDelta::Linear(delta),
                    on_ground: true,
                }))
            }
        };
        let head = (next.head_yaw != self.head_yaw).then_some(PacketPayload::HeadRotation(
            ClientboundRotateHead {
                entity_id: id,
                y_head_rot: next.head_yaw,
            },
        ));
        *self = next;
        Motion { body, head }
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
    use mcrs_minecraft_engine_world::entity::physics::Rotation;

    fn at(x: f64, yaw: f32) -> Transform {
        Transform::from_translation(DVec3::new(x, 64.0, 0.0)).with_rotation(Rotation::new(yaw, 0.0))
    }

    #[test]
    fn small_steps_are_relative_and_a_long_jump_is_absolute() {
        let mut base = DeltaBase::new(&at(0.0, 0.0));
        let id = VarInt(3);

        let walked = base.advance(id, &at(0.5, 0.0), Keyframe::NotDue);
        assert!(
            matches!(
                &walked.body,
                Some(PacketPayload::EntityMove(ClientboundMoveEntityPos {
                    delta: VecDelta::Linear([2048, 0, 0]),
                    ..
                }))
            ),
            "{walked:?}"
        );
        assert!(walked.head.is_none(), "the head did not turn");

        let jumped = base.advance(id, &at(20.0, 0.0), Keyframe::NotDue);
        assert!(
            matches!(&jumped.body, Some(PacketPayload::EntityPosSync(_))),
            "{jumped:?}"
        );
        assert!(
            base.advance(id, &at(20.0, 0.0), Keyframe::NotDue)
                .is_empty()
        );
    }

    #[test]
    fn a_turn_sends_the_look_in_degrees_and_the_head() {
        let mut base = DeltaBase::new(&at(0.0, 0.0));

        let turned = base.advance(VarInt(3), &at(0.0, 90.0), Keyframe::NotDue);

        let Some(PacketPayload::EntityRotate(rotate)) = &turned.body else {
            panic!("{turned:?}");
        };
        assert_eq!(rotate.y_rot, ByteAngle::from_degrees(90.0));
        assert!(
            matches!(&turned.head, Some(PacketPayload::HeadRotation(head)) if head.y_head_rot == ByteAngle::from_degrees(90.0)),
            "{turned:?}"
        );
    }
}
