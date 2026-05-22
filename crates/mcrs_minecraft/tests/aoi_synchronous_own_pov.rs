//! Covers AOI-06 (own-POV ring expansion is synchronous: a boundary-
//! crossing tick emits the corresponding ChunkLoad packet with
//! `PacketPriority::Critical` in the same tick).

use bevy_math::DVec3;
use mcrs_engine::world::dimension::DimensionBundle;
use mcrs_minecraft::world::bus::{PacketPayload, PacketPriority, PacketTarget};

mod harness;
use harness::{drain_outbound, drive_aoi_tick, make_aoi_app, spawn_player_in_dim};

#[test]
fn own_pov_chunk_load_emits_critical_priority_same_tick() {
    let mut app = make_aoi_app();
    let dim = app.world_mut().spawn(DimensionBundle::default()).id();
    let player = spawn_player_in_dim(&mut app, dim, DVec3::new(0.0, 64.0, 0.0));

    // Drive the boundary-crossing tick: the initial spawn already
    // qualifies for Added<ChunkSubscriptionSet>, so update_own_pov
    // computes the full subscription set and emits ChunkLoad packets
    // for every column it just added.
    drive_aoi_tick(&mut app);

    let emitted = drain_outbound(&mut app);
    let saw_critical_chunk_load = emitted.iter().any(|pkt| {
        matches!(pkt.target, PacketTarget::SinglePlayer(p) if p == player)
            && pkt.priority == PacketPriority::Critical
            && matches!(pkt.data, PacketPayload::ChunkLoad { .. })
    });
    assert!(
        saw_critical_chunk_load,
        "expected at least one Critical ChunkLoad packet for the player; \
         emitted {} packets total",
        emitted.len()
    );
}
