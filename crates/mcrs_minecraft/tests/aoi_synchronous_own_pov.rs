//! Wave 0 failing scaffold — covers AOI-06 (own-POV ring expansion is
//! synchronous: a boundary-crossing tick emits the corresponding
//! ChunkLoad packet with PacketPriority::Critical in the same tick).

use bevy_app::App;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use mcrs_minecraft::world::bus::{
    OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget,
};

#[derive(Component)]
struct PlayerMarker;

#[test]
fn own_pov_chunk_load_emits_critical_priority_same_tick() {
    panic!("not yet implemented — pending AoI system wiring");

    #[allow(unreachable_code)]
    {
        let mut app = App::new();
        app.add_message::<OutboundPlayerPacket>();
        let player = app.world_mut().spawn(PlayerMarker).id();

        // Write a Transform change crossing a chunk boundary.
        // (No real Transform Component yet; populated when the AoI
        // systems and PlayerBundle land.)

        app.update();

        let buf = app.world().resource::<Messages<OutboundPlayerPacket>>();
        let cursor = buf.get_cursor();
        let mut saw_critical_chunk_load = false;
        for pkt in cursor.read(buf) {
            if !matches!(pkt.target, PacketTarget::SinglePlayer(p) if p == player) {
                continue;
            }
            if pkt.priority != PacketPriority::Critical {
                continue;
            }
            if matches!(pkt.data, PacketPayload::ChunkLoad { .. }) {
                saw_critical_chunk_load = true;
            }
        }
        assert!(
            saw_critical_chunk_load,
            "expected at least one Critical ChunkLoad packet for the player on the boundary-crossing tick"
        );
    }
}
