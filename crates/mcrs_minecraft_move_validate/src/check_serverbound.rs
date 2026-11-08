use glam::DVec3;
use mcrs_minecraft_protocol::{Decode, VarInt};

use crate::packet_id::{
    ACCEPT_TELEPORTATION, CLIENT_TICK_END, MOVE_PLAYER_POS, MOVE_PLAYER_POS_ROT, MOVE_PLAYER_ROT,
    MOVE_PLAYER_STATUS_ONLY, MOVE_VEHICLE, PLAYER_ABILITIES, PLAYER_COMMAND, PLAYER_INPUT,
    PLAYER_LOADED, PONG,
};
use crate::{
    Accept, Checked, Disconnect, Finding, Look, Move, Reports, Serverbound, VehicleMove, Verdict,
};

const HORIZONTAL_LIMIT: f64 = 3.0e7;
const VERTICAL_LIMIT: f64 = 2.0e7;
const MOVE_FLAGS: u8 = 0x03;
const INPUT_FLAGS: u8 = 0x7f;
const FLY_REQUEST: u8 = 0x02;
const LAST_PLAYER_ACTION: i32 = 6;

/// Layer 0: checks the play-state packet `id` with `body`, the bytes after its id, and no other
/// state. A packet that these checks do not read passes as [`Serverbound::Other`].
#[must_use]
pub fn check_serverbound(id: i32, body: &[u8]) -> Verdict {
    let mut fields = Fields(body);
    let mut reports = Reports::default();
    let Some(packet) = decode(id, &mut fields, &mut reports).filter(|_| fields.0.is_empty()) else {
        return Verdict::Disconnect(Disconnect::Malformed);
    };
    let pose = match packet {
        Serverbound::Move(Move { position, look, .. }) => check_pose(position, look, &mut reports),
        Serverbound::Accept(Accept { position, look, .. }) => {
            check_pose(Some(position), Some(look), &mut reports)
        }
        Serverbound::VehicleMove(VehicleMove { position, look, .. }) => {
            if position.is_finite() && look.is_finite() {
                Ok(())
            } else {
                Err(Disconnect::InvalidVehicleMovement)
            }
        }
        Serverbound::ClientTickEnd
        | Serverbound::PlayerLoaded
        | Serverbound::Pong(_)
        | Serverbound::Other => Ok(()),
    };
    match pose {
        Ok(()) => Verdict::Pass(Checked { packet, reports }),
        Err(refused) => Verdict::Disconnect(refused),
    }
}

/// A NaN or infinite value disconnects; a value that the server clamps is a finding.
fn check_pose(
    position: Option<DVec3>,
    look: Option<Look>,
    reports: &mut Reports,
) -> Result<(), Disconnect> {
    if position.is_some_and(|position| !position.is_finite())
        || look.is_some_and(|look| !look.is_finite())
    {
        return Err(Disconnect::InvalidPlayerMovement);
    }
    if position.is_some_and(|position| {
        position.x.abs() > HORIZONTAL_LIMIT
            || position.z.abs() > HORIZONTAL_LIMIT
            || position.y.abs() > VERTICAL_LIMIT
    }) {
        reports.add(Finding::PositionOutOfRange);
    }
    if look.is_some_and(|look| look.pitch.abs() > 90.0) {
        reports.add(Finding::PitchOutOfRange);
    }
    Ok(())
}

/// Reads the fields of `id`; `None` when a field is missing or holds a value it cannot hold.
fn decode(id: i32, fields: &mut Fields<'_>, reports: &mut Reports) -> Option<Serverbound> {
    let packet = match id {
        MOVE_PLAYER_POS => {
            let position = fields.position()?;
            fields.player_move(Some(position), None, reports)?
        }
        MOVE_PLAYER_POS_ROT => {
            let position = fields.position()?;
            let look = fields.look()?;
            fields.player_move(Some(position), Some(look), reports)?
        }
        MOVE_PLAYER_ROT => {
            let look = fields.look()?;
            fields.player_move(None, Some(look), reports)?
        }
        MOVE_PLAYER_STATUS_ONLY => fields.player_move(None, None, reports)?,
        ACCEPT_TELEPORTATION => Serverbound::Accept(Accept {
            teleport_id: fields.var_int()?,
            position: fields.position()?,
            look: fields.look()?,
        }),
        MOVE_VEHICLE => Serverbound::VehicleMove(VehicleMove {
            position: fields.position()?,
            look: fields.look()?,
            on_ground: fields.byte()? != 0,
        }),
        PLAYER_INPUT => {
            if fields.byte()? & !INPUT_FLAGS != 0 {
                reports.add(Finding::UnknownInputFlag);
            }
            Serverbound::Other
        }
        PLAYER_ABILITIES => {
            if fields.byte()? & !FLY_REQUEST != 0 {
                reports.add(Finding::UnknownAbilityFlags);
            }
            Serverbound::Other
        }
        PLAYER_COMMAND => {
            fields.var_int()?;
            let action = fields.var_int()?;
            fields.var_int()?;
            (0..=LAST_PLAYER_ACTION).contains(&action).then_some(())?;
            Serverbound::Other
        }
        CLIENT_TICK_END => Serverbound::ClientTickEnd,
        PLAYER_LOADED => Serverbound::PlayerLoaded,
        PONG => Serverbound::Pong(fields.i32()?),
        _ => {
            fields.0 = &[];
            Serverbound::Other
        }
    };
    Some(packet)
}

/// The fields of a packet body that are not read yet.
struct Fields<'a>(&'a [u8]);

/// Reads the fixed-width fields without the wire `Decode`, which costs about three times as
/// much on this per-packet path.
impl Fields<'_> {
    fn bytes<const N: usize>(&mut self) -> Option<[u8; N]> {
        let (head, rest) = self.0.split_first_chunk::<N>()?;
        self.0 = rest;
        Some(*head)
    }

    fn byte(&mut self) -> Option<u8> {
        self.bytes::<1>().map(|[byte]| byte)
    }

    fn i32(&mut self) -> Option<i32> {
        self.bytes().map(i32::from_be_bytes)
    }

    fn var_int(&mut self) -> Option<i32> {
        VarInt::decode(&mut self.0).ok().map(|VarInt(value)| value)
    }

    fn position(&mut self) -> Option<DVec3> {
        Some(DVec3::new(
            f64::from_be_bytes(self.bytes()?),
            f64::from_be_bytes(self.bytes()?),
            f64::from_be_bytes(self.bytes()?),
        ))
    }

    fn look(&mut self) -> Option<Look> {
        Some(Look {
            yaw: f32::from_be_bytes(self.bytes()?),
            pitch: f32::from_be_bytes(self.bytes()?),
        })
    }

    fn player_move(
        &mut self,
        position: Option<DVec3>,
        look: Option<Look>,
        reports: &mut Reports,
    ) -> Option<Serverbound> {
        let flags = self.byte()?;
        if flags & !MOVE_FLAGS != 0 {
            reports.add(Finding::UnknownMoveFlags);
        }
        Some(Serverbound::Move(Move {
            position,
            look,
            on_ground: flags & 0x01 != 0,
            horizontal_collision: flags & 0x02 != 0,
        }))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn pos(x: f64, y: f64, z: f64) -> Vec<u8> {
        [x, y, z]
            .iter()
            .flat_map(|value| value.to_be_bytes())
            .collect()
    }

    fn look(yaw: f32, pitch: f32) -> Vec<u8> {
        [yaw, pitch]
            .iter()
            .flat_map(|value| value.to_be_bytes())
            .collect()
    }

    fn reports_of(id: i32, body: &[u8]) -> Reports {
        match check_serverbound(id, body) {
            Verdict::Pass(checked) => checked.reports,
            Verdict::Disconnect(disconnect) => panic!("{id}: {disconnect}"),
        }
    }

    fn disconnect_of(id: i32, body: &[u8]) -> Option<Disconnect> {
        match check_serverbound(id, body) {
            Verdict::Pass(_) => None,
            Verdict::Disconnect(disconnect) => Some(disconnect),
        }
    }

    /// MVC-CHK-023, MVC-VEH-010: the conditions and responses of the value table.
    #[test]
    fn each_row_of_the_value_table_gets_its_response() {
        let values = [
            (f64::NAN, false),
            (f64::INFINITY, false),
            (f64::NEG_INFINITY, false),
            (-0.0, true),
            (1.0e38, true),
        ];
        for (value, finite) in values {
            let player = (!finite).then_some(Disconnect::InvalidPlayerMovement);
            let vehicle = (!finite).then_some(Disconnect::InvalidVehicleMovement);
            for axis in 0..5 {
                let mut fields = [0.0; 5];
                fields[axis] = value;
                let position = pos(fields[0], fields[1], fields[2]);
                let rotation = look(fields[3] as f32, fields[4] as f32);
                let packets = [
                    (
                        MOVE_PLAYER_POS_ROT,
                        [&position[..], &rotation, &[0]].concat(),
                    ),
                    (
                        ACCEPT_TELEPORTATION,
                        [&[1], &position[..], &rotation].concat(),
                    ),
                ];
                for (id, body) in packets {
                    assert_eq!(disconnect_of(id, &body), player, "{id}, {fields:?}");
                }
                let vehicle_move = [&position[..], &rotation, &[0]].concat();
                assert_eq!(disconnect_of(MOVE_VEHICLE, &vehicle_move), vehicle);
                let only = if axis < 3 {
                    (MOVE_PLAYER_POS, [&position[..], &[0]].concat())
                } else {
                    (MOVE_PLAYER_ROT, [&rotation[..], &[0]].concat())
                };
                assert_eq!(disconnect_of(only.0, &only.1), player, "{fields:?}");
            }
        }
        let finite = pos(1.0, 2.0, 3.0);
        let accept = |position: &[u8], rotation: &[u8]| [&[1], position, rotation].concat();

        let pitch =
            |value: f32| reports_of(MOVE_PLAYER_ROT, &[&look(0.0, value)[..], &[0]].concat());
        assert!(pitch(-90.0).is_empty());
        assert!(pitch(90.5).contains(Finding::PitchOutOfRange));
        assert!(
            reports_of(ACCEPT_TELEPORTATION, &accept(&finite, &look(0.0, -120.0)))
                .contains(Finding::PitchOutOfRange)
        );

        let out_of_range = |x: f64, y: f64, z: f64| {
            reports_of(MOVE_PLAYER_POS, &[&pos(x, y, z)[..], &[0]].concat())
                .contains(Finding::PositionOutOfRange)
        };
        assert!(!out_of_range(-3.0e7, 2.0e7, 3.0e7));
        assert!(out_of_range(3.000_000_1e7, 0.0, 0.0));
        assert!(out_of_range(0.0, -2.000_000_1e7, 0.0));
        assert!(out_of_range(0.0, 0.0, -3.000_000_1e7));

        assert!(reports_of(MOVE_PLAYER_STATUS_ONLY, &[0x03]).is_empty());
        assert!(reports_of(MOVE_PLAYER_STATUS_ONLY, &[0xff]).contains(Finding::UnknownMoveFlags));
        assert!(reports_of(PLAYER_INPUT, &[0x7f]).is_empty());
        assert!(reports_of(PLAYER_INPUT, &[0x80]).contains(Finding::UnknownInputFlag));
        assert!(reports_of(PLAYER_ABILITIES, &[0x02]).is_empty());
        assert!(reports_of(PLAYER_ABILITIES, &[0x06]).contains(Finding::UnknownAbilityFlags));

        assert!(reports_of(PLAYER_COMMAND, &[1, 6, 0]).is_empty());
        assert_eq!(
            disconnect_of(PLAYER_COMMAND, &[1, 7, 0]),
            Some(Disconnect::Malformed)
        );
    }

    /// MVC-CHK-023: the sizes after the packet id; any other size disconnects.
    #[test]
    fn a_packet_of_another_size_disconnects() {
        let fixed = [
            (MOVE_PLAYER_POS, 25),
            (MOVE_PLAYER_POS_ROT, 33),
            (MOVE_PLAYER_ROT, 9),
            (MOVE_PLAYER_STATUS_ONLY, 1),
            (PLAYER_INPUT, 1),
            (PLAYER_ABILITIES, 1),
            (CLIENT_TICK_END, 0),
            (PLAYER_LOADED, 0),
            (PONG, 4),
            (MOVE_VEHICLE, 33),
        ];
        for (id, size) in fixed {
            for len in 0..=40 {
                let malformed = disconnect_of(id, &vec![0; len]) == Some(Disconnect::Malformed);
                assert_eq!(malformed, len != size, "packet {id}, {len} bytes");
            }
        }

        let rest = [pos(0.5, 64.0, -0.5), look(0.0, 0.0)].concat();
        let ids: [&[u8]; 3] = [&[0x01], &[0x80, 0x01], &[0xff, 0xff, 0xff, 0xff, 0x07]];
        for id in ids {
            let accept = [id, &rest].concat();
            assert_eq!(
                disconnect_of(ACCEPT_TELEPORTATION, &accept),
                None,
                "{}",
                accept.len()
            );
            let longer = [&accept[..], &[0]].concat();
            assert_eq!(
                disconnect_of(ACCEPT_TELEPORTATION, &longer),
                Some(Disconnect::Malformed)
            );
        }

        let longest = [
            0xff, 0xff, 0xff, 0xff, 0x0f, 6, 0xff, 0xff, 0xff, 0xff, 0x0f,
        ];
        assert_eq!(disconnect_of(PLAYER_COMMAND, &longest), None);
        let too_long_var_int = [0xff, 0xff, 0xff, 0xff, 0xff, 0x0f, 0, 0];
        assert_eq!(
            disconnect_of(PLAYER_COMMAND, &too_long_var_int),
            Some(Disconnect::Malformed)
        );
        assert_eq!(
            disconnect_of(PLAYER_COMMAND, &[0, 0]),
            Some(Disconnect::Malformed)
        );
        assert_eq!(
            disconnect_of(PLAYER_COMMAND, &[0, 0, 0, 0]),
            Some(Disconnect::Malformed)
        );
    }
}
