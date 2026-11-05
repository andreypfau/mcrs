use crate::encode::record;
use bytes::BytesMut;
use glam::DVec3;
use mcrs_minecraft_core::Look;
use mcrs_minecraft_protocol::{Result, VarInt};
use mcrs_minecraft_protocol_game::ByteAngle;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundEntityPositionSync, ClientboundMoveEntityPos, ClientboundMoveEntityPosRot,
    ClientboundMoveEntityRot, ClientboundRotateHead, DeltaStep, PositionPath, VecDelta,
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

/// A point of a path: the position that the subject reaches `ticks` ticks after the point before.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Waypoint {
    pub(crate) ticks: u32,
    pub(crate) position: DVec3,
}

/// How a move reaches its last position.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Path<'a> {
    /// One relative move, which the client interpolates over its own fixed time.
    Linear(DVec3),
    /// Relative steps with the ticks between them, which keep a slow tier in motion.
    Stepped(&'a [Waypoint]),
}

impl Path<'_> {
    fn end(&self) -> Option<DVec3> {
        match self {
            Self::Linear(position) => Some(*position),
            Self::Stepped(waypoints) => waypoints.last().map(|waypoint| waypoint.position),
        }
    }
}

/// What the viewers of one tier last received for a subject. All viewers of a subject at a tier
/// share one base, so a relative move of the subject is the same packet for each of them.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DeltaBase {
    pub position: DVec3,
    pub look: Look,
    /// The yaw of the body and of the head, which a player turns together.
    pub yaw: ByteAngle,
    pub pitch: ByteAngle,
    /// The tick of the last absolute position that the viewers received.
    keyframe_tick: u64,
}

impl DeltaBase {
    pub fn new(pose: &Pose, tick: u64) -> Self {
        Self {
            position: pose.position,
            look: pose.look,
            yaw: ByteAngle::from_degrees(pose.look.yaw),
            pitch: ByteAngle::from_degrees(pose.look.pitch),
            keyframe_tick: tick,
        }
    }

    /// Moves the base along `path` to its end, looking at `look`, at `tick`, and appends to
    /// `bytes` the packets that move the subject `id` on a client from the old base to the new
    /// one. A step that does not fit a relative move, or a keyframe due at `tick`, sends the
    /// absolute position. The base and `bytes` stay as they are when an encoding fails.
    pub(crate) fn advance(
        &mut self,
        id: VarInt,
        path: Path<'_>,
        look: Look,
        tick: u64,
        bytes: &mut BytesMut,
    ) -> Result<()> {
        let Some(end) = path.end() else {
            return Ok(());
        };
        let pose = Pose {
            position: end,
            look,
        };
        let keyframe_due = tick.wrapping_sub(self.keyframe_tick) >= KEYFRAME_INTERVAL_TICKS;
        let delta = match path {
            Path::Linear(position) => position_delta(self.position, position).map(VecDelta::Linear),
            Path::Stepped(waypoints) => stepped_delta(self.position, waypoints),
        };
        let Some(delta) = delta.filter(|_| !keyframe_due) else {
            return self.sync(id, &pose, tick, bytes);
        };
        let mut next = Self::new(&pose, self.keyframe_tick);
        let turned = (next.yaw, next.pitch) != (self.yaw, self.pitch);
        let still = delta == VecDelta::Linear([0; 3]) || delta == VecDelta::Stepped(Vec::new());
        let body = match delta {
            _ if still && !turned => {
                // Only the exact look changed, below what a client can see: keep the old one so
                // that a keyframe shows what the client has.
                next.look = self.look;
                None
            }
            _ if still => Some(record(&ClientboundMoveEntityRot {
                entity_id: id,
                y_rot: next.yaw,
                x_rot: next.pitch,
                on_ground: true,
            })?),
            delta if turned => Some(record(&ClientboundMoveEntityPosRot {
                entity_id: id,
                delta,
                y_rot: next.yaw,
                x_rot: next.pitch,
                on_ground: true,
            })?),
            delta => Some(record(&ClientboundMoveEntityPos {
                entity_id: id,
                delta,
                on_ground: true,
            })?),
        };
        let head = self.head_packet(id, next.yaw)?;
        *self = next;
        for packet in body.iter().chain(&head) {
            bytes.extend_from_slice(packet);
        }
        Ok(())
    }

    /// Moves the base to `pose` at `tick` and appends the absolute position of `id` there.
    pub(crate) fn sync(
        &mut self,
        id: VarInt,
        pose: &Pose,
        tick: u64,
        bytes: &mut BytesMut,
    ) -> Result<()> {
        let next = Self::new(pose, tick);
        let sync = record(&ClientboundEntityPositionSync {
            entity_id: id,
            position: PositionPath::Linear(next.position),
            look: pose.look,
            on_ground: true,
        })?;
        let head = self.head_packet(id, next.yaw)?;
        *self = next;
        bytes.extend_from_slice(&sync);
        if let Some(head) = &head {
            bytes.extend_from_slice(head);
        }
        Ok(())
    }

    /// Appends the absolute position and head of `id` at this base, so a client that holds the
    /// subject at another base comes to this one.
    pub(crate) fn keyframe(&self, id: VarInt, bytes: &mut BytesMut) -> Result<()> {
        let sync = record(&ClientboundEntityPositionSync {
            entity_id: id,
            position: PositionPath::Linear(self.position),
            look: self.look,
            on_ground: true,
        })?;
        let head = record(&ClientboundRotateHead {
            entity_id: id,
            y_head_rot: self.yaw,
        })?;
        bytes.extend_from_slice(&sync);
        bytes.extend_from_slice(&head);
        Ok(())
    }

    fn head_packet(&self, id: VarInt, head_yaw: ByteAngle) -> Result<Option<bytes::Bytes>> {
        if head_yaw == self.yaw {
            return Ok(None);
        }
        Ok(Some(record(&ClientboundRotateHead {
            entity_id: id,
            y_head_rot: head_yaw,
        })?))
    }
}

/// The steps from `from` along `waypoints`, without the steps that do not move, or `None` when
/// a step moves more than a relative move holds.
fn stepped_delta(from: DVec3, waypoints: &[Waypoint]) -> Option<VecDelta> {
    let mut steps = Vec::with_capacity(waypoints.len());
    let mut at = from;
    let mut waited = 0u32;
    for waypoint in waypoints {
        waited = waited.saturating_add(waypoint.ticks);
        let delta = position_delta(at, waypoint.position)?;
        if delta == [0; 3] {
            continue;
        }
        steps.push(DeltaStep {
            ticks: VarInt(i32::try_from(waited).unwrap_or(i32::MAX)),
            delta,
        });
        at = waypoint.position;
        waited = 0;
    }
    Some(VecDelta::Stepped(steps))
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
        base.advance(
            VarInt(3),
            Path::Linear(pose.position),
            pose.look,
            tick,
            &mut bytes,
        )?;
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

    #[test]
    fn a_path_sends_the_steps_that_move_with_the_ticks_they_waited() -> Result<()> {
        let mut base = DeltaBase::new(&at(0.0, 0.0), 0);
        let waypoints = [
            Waypoint {
                ticks: 4,
                position: DVec3::new(0.0, 64.0, 0.0),
            },
            Waypoint {
                ticks: 3,
                position: DVec3::new(1.0, 64.0, 0.0),
            },
            Waypoint {
                ticks: 3,
                position: DVec3::new(2.5, 64.0, 0.0),
            },
        ];
        let mut bytes = BytesMut::new();

        base.advance(
            VarInt(3),
            Path::Stepped(&waypoints),
            Look::default(),
            10,
            &mut bytes,
        )?;

        let expected = record(&ClientboundMoveEntityPos {
            entity_id: VarInt(3),
            delta: VecDelta::Stepped(vec![
                DeltaStep {
                    ticks: VarInt(7),
                    delta: [4096, 0, 0],
                },
                DeltaStep {
                    ticks: VarInt(3),
                    delta: [6144, 0, 0],
                },
            ]),
            on_ground: true,
        })?;
        assert_eq!(
            bytes.freeze(),
            expected,
            "the step that stands still waits in the ticks of the next one"
        );
        Ok(())
    }
}
