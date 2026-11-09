use bytes::BytesMut;
use mcrs_minecraft_protocol::{Result, VarInt, write_record};
use mcrs_minecraft_protocol_game::ByteAngle;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundMoveEntityPos, ClientboundMoveEntityPosRot, ClientboundMoveEntityRot, VecDelta,
};

/// The squared move, in blocks, from which the position of a mob counts as changed
/// (TRK-UPD-003).
pub(crate) const MOVED: f64 = 7.629_394_5e-6_f32 as f64;

/// A mob sends its position in each update whose tracker tick count is a multiple of this, also
/// at rest (TRK-UPD-003).
pub(crate) const POSITION_REFRESH: u64 = 60;

/// A mob sends its absolute position in the next update after this many updates without one
/// (TRK-UPD-003).
pub(crate) const UPDATES_PER_SYNC: u32 = 400;

/// A moving player sends its absolute position at least this often, in ticks, so a client that
/// lost a relative move under the drop policy of its queue comes back to the right place
/// (`entity-tracking.md`, section 15.1, step 6).
pub(crate) const KEYFRAME_INTERVAL_TICKS: u64 = 100;

/// Whether an update sends the absolute position, whatever moved.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Resync {
    Due,
    NotDue,
}

/// What changed since the viewers last received the subject.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Due {
    pub(crate) position: bool,
    pub(crate) rotation: bool,
}

/// The move packet of an update, in the order of the table of TRK-UPD-003.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MoveChoice {
    /// No move packet.
    Still,
    Relative(RelativeMove),
    /// The absolute position and look.
    Sync,
}

/// A move packet relative to the position base.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RelativeMove {
    Pos(VecDelta),
    PosRot(VecDelta),
    Rot,
}

impl MoveChoice {
    /// The packet for `due`. `delta` gives the relative path from the base to the new position,
    /// or `None` when it does not fit a relative move; it runs only when the position is due.
    pub(crate) fn choose(
        resync: Resync,
        due: Due,
        delta: impl FnOnce() -> Option<VecDelta>,
    ) -> Self {
        if resync == Resync::Due {
            return Self::Sync;
        }
        if due.position {
            return match delta() {
                None => Self::Sync,
                Some(delta) if due.rotation => Self::Relative(RelativeMove::PosRot(delta)),
                Some(delta) => Self::Relative(RelativeMove::Pos(delta)),
            };
        }
        if due.rotation {
            Self::Relative(RelativeMove::Rot)
        } else {
            Self::Still
        }
    }
}

impl RelativeMove {
    /// Appends the packet of the move of `id`, which looks at `yaw` and `pitch` after it.
    pub(crate) fn write(
        self,
        id: VarInt,
        yaw: ByteAngle,
        pitch: ByteAngle,
        on_ground: bool,
        bytes: &mut BytesMut,
    ) -> Result<()> {
        match self {
            Self::Pos(delta) => write_record(
                bytes,
                &ClientboundMoveEntityPos {
                    entity_id: id,
                    delta,
                    on_ground,
                },
            ),
            Self::PosRot(delta) => write_record(
                bytes,
                &ClientboundMoveEntityPosRot {
                    entity_id: id,
                    delta,
                    y_rot: yaw,
                    x_rot: pitch,
                    on_ground,
                },
            ),
            Self::Rot => write_record(
                bytes,
                &ClientboundMoveEntityRot {
                    entity_id: id,
                    y_rot: yaw,
                    x_rot: pitch,
                    on_ground,
                },
            ),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const STEP: VecDelta = VecDelta::Linear([1, 0, 0]);

    fn choose(resync: Resync, position: bool, rotation: bool, fits: bool) -> MoveChoice {
        let due = Due { position, rotation };
        MoveChoice::choose(resync, due, || fits.then_some(STEP))
    }

    #[test]
    fn the_choice_follows_the_order_of_the_table() {
        use Resync::{Due, NotDue};
        let pos = MoveChoice::Relative(RelativeMove::Pos(STEP));
        let pos_rot = MoveChoice::Relative(RelativeMove::PosRot(STEP));
        let rot = MoveChoice::Relative(RelativeMove::Rot);
        let rows = [
            (
                Due,
                true,
                true,
                true,
                MoveChoice::Sync,
                "a resync wins over each move",
            ),
            (
                Due,
                false,
                false,
                true,
                MoveChoice::Sync,
                "a resync is sent at rest too",
            ),
            (
                NotDue,
                true,
                false,
                false,
                MoveChoice::Sync,
                "a path that does not fit",
            ),
            (
                NotDue,
                true,
                true,
                false,
                MoveChoice::Sync,
                "a turn does not save a long move",
            ),
            (NotDue, true, true, true, pos_rot, "position and rotation"),
            (NotDue, true, false, true, pos, "position only"),
            (NotDue, false, true, true, rot.clone(), "rotation only"),
            (
                NotDue,
                false,
                true,
                false,
                rot,
                "no path is needed without a move",
            ),
            (
                NotDue,
                false,
                false,
                true,
                MoveChoice::Still,
                "nothing is due",
            ),
        ];
        for (resync, position, rotation, fits, expected, case) in rows {
            assert_eq!(choose(resync, position, rotation, fits), expected, "{case}");
        }
    }
}
