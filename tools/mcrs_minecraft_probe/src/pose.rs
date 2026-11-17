use mcrs_minecraft_core::Look;
use mcrs_minecraft_protocol_game::clientbound::ClientboundPlayerPosition;
use mcrs_minecraft_protocol_game::serverbound::{
    ServerboundAcceptTeleportation, ServerboundMovePlayerPosRot,
};
use mcrs_minecraft_protocol_game::{MoveFlags, Position, PositionFlag};

#[derive(Default)]
pub(crate) struct Pose {
    x: f64,
    y: f64,
    z: f64,
    look: Look,
    accepted: bool,
}

impl Pose {
    pub(crate) fn look(&self) -> Look {
        self.look
    }

    pub(crate) fn accepted_position(&self) -> Option<Position> {
        self.accepted.then(|| Position::new(self.x, self.y, self.z))
    }

    pub(crate) fn clear_accepted(&mut self) {
        self.accepted = false;
    }

    pub(crate) fn accept(
        &mut self,
        packet: &ClientboundPlayerPosition,
    ) -> ServerboundAcceptTeleportation {
        let flags = packet.flags;
        let mut position = packet.position;
        let mut look = packet.look;
        if flags.contains(PositionFlag::X) {
            position.x += self.x;
        }
        if flags.contains(PositionFlag::Y) {
            position.y += self.y;
        }
        if flags.contains(PositionFlag::Z) {
            position.z += self.z;
        }
        if flags.contains(PositionFlag::YRot) {
            look.yaw += self.look.yaw;
        }
        if flags.contains(PositionFlag::XRot) {
            look.pitch += self.look.pitch;
        }
        self.x = position.x;
        self.y = position.y;
        self.z = position.z;
        self.look = look;
        self.accepted = true;
        ServerboundAcceptTeleportation {
            teleport_id: packet.teleport_id,
            position: Position::from(position),
            look,
        }
    }

    pub(crate) fn move_pos_rot(&self) -> Option<ServerboundMovePlayerPosRot> {
        self.accepted.then_some(ServerboundMovePlayerPosRot {
            position: Position::new(self.x, self.y, self.z),
            look: self.look,
            flags: MoveFlags::new().with_on_ground(true),
        })
    }
}
