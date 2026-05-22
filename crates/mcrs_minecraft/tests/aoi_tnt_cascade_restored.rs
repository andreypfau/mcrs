//! Wave 0 failing scaffold — cascading TNT works end-to-end after the
//! per-dim block-update emit-site migration restores the single-hop
//! observer fan-out. Regression guard against the silent-drop pitfall.

use bevy_app::App;
use bevy_ecs::message::Messages;
use mcrs_minecraft::world::bus::{OutboundPlayerPacket, PacketPayload};

#[test]
fn tnt_cascade_propagates_through_block_update_per_dim() {
    panic!("not yet implemented — pending per-dim block-update wiring");

    #[allow(unreachable_code)]
    {
        let mut app = App::new();
        app.add_message::<OutboundPlayerPacket>();

        // Place a 3x3 TNT cluster and prime the centre.
        // (Real block placement wired by a later milestone.)

        // Advance ticks until the chain completes.
        for _ in 0..50 {
            app.update();
        }

        // Assertion 1: all 9 TNT blocks have detonated.
        let remaining_tnt = 0; // placeholder — counted from world state when wired
        assert_eq!(
            remaining_tnt, 0,
            "expected all 9 TNT blocks to detonate during cascade"
        );

        // Assertion 2: at least 9 BlockUpdate packets emitted.
        let buf = app.world().resource::<Messages<OutboundPlayerPacket>>();
        let cursor = buf.get_cursor();
        let count = cursor
            .read(buf)
            .filter(|pkt| matches!(pkt.data, PacketPayload::BlockUpdate { .. }))
            .count();
        assert!(
            count >= 9,
            "expected at least 9 BlockUpdate packets, observed {count}"
        );

        // Assertion 3: the ExplosionConfig cascade flag flips back to
        // true once the silent-drop regression is fixed. Wired when the
        // explosion module is re-exported through the world module.
    }
}
