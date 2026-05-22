//! Wave 0 failing scaffold — block-update emission resolves recipients
//! through the per-dim PlayerObservers Component, eliminating the
//! two-frame buffer rotation that caused the TNT silent-drop
//! regression.

use bevy_app::App;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use mcrs_engine::aoi::PlayerObservers;
use mcrs_minecraft::world::bus::{
    OutboundPlayerPacket, PacketPayload, PacketTarget,
};

#[derive(Component)]
struct PlayerMarker;

#[test]
fn block_update_resolves_observers_per_dim_emit_site() {
    panic!("not yet implemented — pending per-dim block-update wiring");

    #[allow(unreachable_code)]
    {
        let mut app = App::new();
        app.add_message::<OutboundPlayerPacket>();

        let player = app.world_mut().spawn(PlayerMarker).id();
        let mut observers = PlayerObservers::default();
        observers.0.push(player);
        let _chunk = app.world_mut().spawn(observers).id();

        // Trigger a block change and run one tick.
        app.update();

        let buf = app.world().resource::<Messages<OutboundPlayerPacket>>();
        let cursor = buf.get_cursor();
        let mut block_update_count = 0;
        for pkt in cursor.read(buf) {
            if !matches!(pkt.data, PacketPayload::BlockUpdate { .. }) {
                continue;
            }
            match &pkt.target {
                PacketTarget::PlayerSet(set) => {
                    assert!(
                        set.contains(&player),
                        "BlockUpdate PlayerSet target missing the chunk observer"
                    );
                    block_update_count += 1;
                }
                _ => panic!(
                    "expected PacketTarget::PlayerSet for BlockUpdate, got {:?}",
                    pkt.target
                ),
            }
        }
        assert_eq!(
            block_update_count, 1,
            "expected exactly one BlockUpdate packet per block change"
        );
    }
}
