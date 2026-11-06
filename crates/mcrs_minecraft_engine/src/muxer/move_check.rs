//! The stateless check of a serverbound move: it reads one decoded frame and no other state.

use mcrs_minecraft_core::Look;
use mcrs_minecraft_protocol::{Decode, Packet};
use mcrs_minecraft_protocol_game::Position;
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundAcceptTeleportation, ServerboundMovePlayerPos, ServerboundMovePlayerPosRot,
    ServerboundMovePlayerRot,
};
use mcrs_minecraft_text::Text;

/// What the check makes of a frame.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MoveCheck {
    /// The frame goes on to its handlers.
    Pass,
    /// A move or an accept names a coordinate or an angle that is NaN or infinite. No clamp
    /// gives such a value a meaning, so the connection ends.
    NotFinite,
}

impl MoveCheck {
    /// The reason of the disconnect.
    pub fn reason() -> Text {
        Text::translate("multiplayer.disconnect.invalid_player_movement", [])
    }
}

/// Checks the frame `id` with `body`, which the dispatcher already decoded once. A frame that
/// does not decode here passes: the dispatcher answers for its shape.
#[must_use]
pub fn check_move(id: i32, body: &[u8]) -> MoveCheck {
    let finite = match id {
        ServerboundMovePlayerPos::ID => {
            decode::<ServerboundMovePlayerPos>(body).map(|move_| position_is_finite(move_.position))
        }
        ServerboundMovePlayerPosRot::ID => decode::<ServerboundMovePlayerPosRot>(body)
            .map(|move_| position_is_finite(move_.position) && look_is_finite(move_.look)),
        ServerboundMovePlayerRot::ID => {
            decode::<ServerboundMovePlayerRot>(body).map(|move_| look_is_finite(move_.look))
        }
        ServerboundAcceptTeleportation::ID => decode::<ServerboundAcceptTeleportation>(body)
            .map(|accept| position_is_finite(accept.position) && look_is_finite(accept.look)),
        _ => None,
    };
    if finite == Some(false) {
        MoveCheck::NotFinite
    } else {
        MoveCheck::Pass
    }
}

fn decode<'a, P: Decode<'a>>(mut body: &'a [u8]) -> Option<P> {
    P::decode(&mut body).ok()
}

fn position_is_finite(position: Position) -> bool {
    bevy_math::DVec3::from(position).is_finite()
}

fn look_is_finite(look: Look) -> bool {
    look.yaw.is_finite() && look.pitch.is_finite()
}

#[cfg(test)]
mod tests {
    use super::*;

    use mcrs_minecraft_protocol::{Encode, VarInt};
    use mcrs_minecraft_protocol_game::MoveFlags;

    /// xorshift64*, enough to spread the cases.
    struct Cases(u64);

    impl Cases {
        fn next(&mut self) -> u64 {
            self.0 ^= self.0 >> 12;
            self.0 ^= self.0 << 25;
            self.0 ^= self.0 >> 27;
            self.0.wrapping_mul(0x2545_f491_4f6c_dd1d)
        }

        fn value(&mut self) -> f64 {
            let bits = self.next();
            if bits & 1 == 0 {
                SPECIAL[(bits >> 1) as usize % SPECIAL.len()]
            } else {
                ((bits >> 11) as f64 / (1u64 << 53) as f64 - 0.5) * 8.0e7
            }
        }
    }

    fn body(packet: &impl Encode) -> Vec<u8> {
        let mut bytes = Vec::new();
        packet.encode(&mut bytes).unwrap();
        bytes
    }

    const SPECIAL: [f64; 6] = [
        f64::NAN,
        f64::INFINITY,
        f64::NEG_INFINITY,
        0.0,
        3.0e7,
        -1.0e300,
    ];

    #[test]
    fn a_move_or_an_accept_disconnects_exactly_when_a_value_is_not_finite() {
        let mut cases = Cases(0x1023);
        for _ in 0..20_000 {
            let [x, y, z, yaw, pitch] = [(); 5].map(|()| cases.value());
            let position = Position::new(x, y, z);
            let look = Look {
                yaw: yaw as f32,
                pitch: pitch as f32,
            };
            let flags = MoveFlags::new();
            let finite_position = [x, y, z].iter().all(|value| value.is_finite());
            let finite_look = look.yaw.is_finite() && look.pitch.is_finite();
            let packets = [
                (
                    ServerboundMovePlayerPos::ID,
                    body(&ServerboundMovePlayerPos { position, flags }),
                    finite_position,
                ),
                (
                    ServerboundMovePlayerPosRot::ID,
                    body(&ServerboundMovePlayerPosRot {
                        position,
                        look,
                        flags,
                    }),
                    finite_position && finite_look,
                ),
                (
                    ServerboundMovePlayerRot::ID,
                    body(&ServerboundMovePlayerRot { look, flags }),
                    finite_look,
                ),
                (
                    ServerboundAcceptTeleportation::ID,
                    body(&ServerboundAcceptTeleportation {
                        teleport_id: VarInt(1),
                        position,
                        look,
                    }),
                    finite_position && finite_look,
                ),
            ];
            for (id, bytes, finite) in packets {
                let expected = if finite {
                    MoveCheck::Pass
                } else {
                    MoveCheck::NotFinite
                };
                assert_eq!(
                    check_move(id, &bytes),
                    expected,
                    "MVC-CHK-023: packet {id}, ({x}, {y}, {z}), {look:?}"
                );
            }
        }
    }
}
