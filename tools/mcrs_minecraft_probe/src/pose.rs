use mcrs_minecraft_core::Look;
use mcrs_minecraft_protocol_game::clientbound::ClientboundPlayerPosition;
use mcrs_minecraft_protocol_game::serverbound::ServerboundAcceptTeleportation;
use mcrs_minecraft_protocol_game::{Position, PositionFlag};

/// Where the server last put the client. The client never moves by itself.
#[derive(Default)]
pub(crate) struct Pose {
    x: f64,
    y: f64,
    z: f64,
    look: Look,
}

impl Pose {
    /// Takes the pose of a position sync and returns its answer. A flagged field of the packet is
    /// an offset from this pose; the answer carries the absolute result.
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
        ServerboundAcceptTeleportation {
            teleport_id: packet.teleport_id,
            position: Position::from(position),
            look,
        }
    }
}
