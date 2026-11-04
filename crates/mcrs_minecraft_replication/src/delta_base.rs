use crate::encode::record;
use bytes::BytesMut;
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

/// A moving subject sends its absolute position at least this often, in ticks, so a client that
/// lost a relative move under the drop policy of its queue comes back to the right place.
pub const KEYFRAME_INTERVAL_TICKS: u64 = 100;

/// What the viewers of a subject last received for it. All viewers of a subject share one base,
/// so a relative move of the subject is the same packet for each viewer.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeltaBase {
    pub position: DVec3,
    pub yaw: ByteAngle,
    pub pitch: ByteAngle,
    pub head_yaw: ByteAngle,
    /// The tick of the last absolute position that the viewers received.
    keyframe_tick: u64,
}

impl DeltaBase {
    pub fn new(pose: &Pose, tick: u64) -> Self {
        let yaw = ByteAngle::from_degrees(pose.look.yaw);
        Self {
            position: pose.position,
            yaw,
            pitch: ByteAngle::from_degrees(pose.look.pitch),
            head_yaw: yaw,
            keyframe_tick: tick,
        }
    }

    /// Moves the base to `pose` at `tick` and appends to `bytes` the packets that move the
    /// subject `id` on a client from the old base to the new one. A delta that does not fit a
    /// relative move, or a keyframe due at `tick`, sends the absolute position. The base and
    /// `bytes` stay as they are when an encoding fails.
    pub fn advance(
        &mut self,
        id: VarInt,
        pose: &Pose,
        tick: u64,
        bytes: &mut BytesMut,
    ) -> Result<()> {
        let mut next = Self::new(pose, self.keyframe_tick);
        let delta = position_delta(self.position, next.position)
            .filter(|_| tick.wrapping_sub(self.keyframe_tick) < KEYFRAME_INTERVAL_TICKS);
        let turned = (next.yaw, next.pitch) != (self.yaw, self.pitch);
        let body = match delta {
            None => {
                next.keyframe_tick = tick;
                Some(record(&ClientboundEntityPositionSync {
                    entity_id: id,
                    position: PositionPath::Linear(next.position),
                    look: pose.look,
                    on_ground: true,
                })?)
            }
            Some([0, 0, 0]) if turned => Some(record(&ClientboundMoveEntityRot {
                entity_id: id,
                y_rot: next.yaw,
                x_rot: next.pitch,
                on_ground: true,
            })?),
            Some([0, 0, 0]) => None,
            Some(delta) if turned => Some(record(&ClientboundMoveEntityPosRot {
                entity_id: id,
                delta: VecDelta::Linear(delta),
                y_rot: next.yaw,
                x_rot: next.pitch,
                on_ground: true,
            })?),
            Some(delta) => Some(record(&ClientboundMoveEntityPos {
                entity_id: id,
                delta: VecDelta::Linear(delta),
                on_ground: true,
            })?),
        };
        let head = if next.head_yaw == self.head_yaw {
            None
        } else {
            Some(record(&ClientboundRotateHead {
                entity_id: id,
                y_head_rot: next.head_yaw,
            })?)
        };
        *self = next;
        for packet in body.iter().chain(&head) {
            bytes.extend_from_slice(packet);
        }
        Ok(())
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
    use bytes::Bytes;

    fn at(x: f64, yaw: f32) -> Pose {
        Pose {
            position: DVec3::new(x, 64.0, 0.0),
            look: Look { yaw, pitch: 0.0 },
        }
    }

    fn sync(id: VarInt, x: f64) -> Result<Bytes> {
        record(&ClientboundEntityPositionSync {
            entity_id: id,
            position: PositionPath::Linear(DVec3::new(x, 64.0, 0.0)),
            look: Look::default(),
            on_ground: true,
        })
    }

    fn advance(base: &mut DeltaBase, pose: &Pose, tick: u64) -> Result<Bytes> {
        let mut bytes = BytesMut::new();
        base.advance(VarInt(3), pose, tick, &mut bytes)?;
        Ok(bytes.freeze())
    }

    #[test]
    fn small_steps_are_relative_and_a_long_jump_is_absolute() -> Result<()> {
        let mut base = DeltaBase::new(&at(0.0, 0.0), 0);

        let walked = advance(&mut base, &at(0.5, 0.0), 1)?;
        let expected = record(&ClientboundMoveEntityPos {
            entity_id: VarInt(3),
            delta: VecDelta::Linear([2048, 0, 0]),
            on_ground: true,
        })?;
        assert_eq!(
            walked, expected,
            "a small step without a turn is one relative move"
        );

        let jumped = advance(&mut base, &at(20.0, 0.0), 2)?;
        assert_eq!(jumped, sync(VarInt(3), 20.0)?);
        assert!(advance(&mut base, &at(20.0, 0.0), 3)?.is_empty());
        Ok(())
    }

    #[test]
    fn a_due_keyframe_is_absolute_even_for_a_small_step() -> Result<()> {
        let mut base = DeltaBase::new(&at(0.0, 0.0), 0);

        let moved = advance(&mut base, &at(0.5, 0.0), KEYFRAME_INTERVAL_TICKS)?;

        assert_eq!(moved, sync(VarInt(3), 0.5)?);
        Ok(())
    }

    #[test]
    fn a_turn_sends_the_look_in_degrees_and_the_head() -> Result<()> {
        let mut base = DeltaBase::new(&at(0.0, 0.0), 0);
        let angle = ByteAngle::from_degrees(90.0);

        let turned = advance(&mut base, &at(0.0, 90.0), 1)?;

        let rotate = record(&ClientboundMoveEntityRot {
            entity_id: VarInt(3),
            y_rot: angle,
            x_rot: ByteAngle(0),
            on_ground: true,
        })?;
        let head = record(&ClientboundRotateHead {
            entity_id: VarInt(3),
            y_head_rot: angle,
        })?;
        assert_eq!(turned, [rotate, head].concat());
        Ok(())
    }
}
