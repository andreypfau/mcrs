use bytes::BytesMut;
use mcrs_minecraft_protocol::{Result, VarInt, write_record};
use mcrs_minecraft_protocol_game::ByteAngle;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundMoveEntityPos, ClientboundMoveEntityPosRot, ClientboundMoveEntityRot, VecDelta,
};

pub(crate) const MOVED: f64 = 7.629_394_5e-6_f32 as f64;

pub(crate) const POSITION_REFRESH: u64 = 60;

pub(crate) const UPDATES_PER_SYNC: u32 = 400;

pub(crate) const KEYFRAME_INTERVAL_TICKS: u64 = 100;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Resync {
    Due,
    NotDue,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Due {
    pub(crate) position: bool,
    pub(crate) rotation: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum MoveChoice {
    Still,
    Relative(RelativeMove),
    Sync,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) enum RelativeMove {
    Pos(VecDelta),
    PosRot(VecDelta),
    Rot,
}

impl MoveChoice {
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
