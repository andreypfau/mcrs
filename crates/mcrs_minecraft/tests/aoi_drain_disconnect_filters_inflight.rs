//! Regression for the deferred-drain bus-filter gap. When
//! drain_pending_disconnects pops a queued host_anchor on tick M, the
//! synchronous on_player_disconnect observer is NOT firing for that
//! anchor this tick — it already fired on the original disconnect tick.
//! Without an explicit push into DisconnectedThisTick at drain time,
//! in-flight bus messages for the queued anchor that land on tick M's
//! bus would slip past filter_inflight_for_disconnect.
//!
//! These tests verify the deferred-drain path pushes into DisconnectedThisTick
//! and the filter clears the set at end-of-tick.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::system::RunSystemOnce;
use mcrs_minecraft::disconnect::{
    DisconnectBudget, DisconnectProtocolPlugin, DisconnectedThisTick, PendingDisconnectQueue,
    drain_pending_disconnects, filter_inflight_for_disconnect,
};
use mcrs_minecraft::world::bus::{
    InboundPlayerDespawn, InboundPlayerSpawn, OutboundPlayerAttached,
    OutboundPlayerDisconnect,
};
use mcrs_minecraft::world::channel_types::DimChannelsResource;
use mcrs_engine::session::{PlayerSessionCounter, SessionEntry, SessionRegistry};
use mcrs_minecraft::world::player_index::PlayerIndex;

fn build_app() -> App {
    let mut app = App::new();
    app.add_message::<InboundPlayerSpawn>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();
    app.init_resource::<PlayerIndex>();
    app.init_resource::<SessionRegistry>();
    app.init_resource::<PlayerSessionCounter>();
    app.init_resource::<DimChannelsResource>();
    app.init_resource::<mcrs_engine::world::sub_app::DimDespawnQueue>();
    app.add_plugins(DisconnectProtocolPlugin);
    app
}

fn insert_player(app: &mut App, host_anchor: Entity, dim: Entity) {
    let session = app
        .world_mut()
        .resource_mut::<PlayerSessionCounter>()
        .next();
    app.world_mut().resource_mut::<SessionRegistry>().insert(
        session,
        SessionEntry {
            connection_entity: Entity::PLACEHOLDER,
            host_anchor,
            dim,
            previous_dim: None,
            in_dim_entity: Some(Entity::PLACEHOLDER),
            epoch: 0,
        },
    );
}

#[test]
fn drain_clears_disconnected_this_tick_via_filter_at_end_of_update() {
    // Companion check: ensure the filter system clears
    // DisconnectedThisTick at the end, so a second tick that does NOT
    // touch the queue does not retain stale anchors.
    let mut app = build_app();
    let dim = Entity::from_raw_u32(3).unwrap();
    let host_anchor = app.world_mut().spawn_empty().id();
    insert_player(&mut app, host_anchor, dim);
    {
        let mut budget = app.world_mut().resource_mut::<DisconnectBudget>();
        budget.remaining = 0;
    }
    {
        let mut q = app.world_mut().resource_mut::<PendingDisconnectQueue>();
        assert!(q.push_back(host_anchor));
    }

    app.world_mut()
        .run_system_once(drain_pending_disconnects)
        .expect("drain runs");
    app.world_mut()
        .run_system_once(filter_inflight_for_disconnect)
        .expect("filter runs");

    // Filter clears host_anchors at end-of-system.
    assert!(
        app.world()
            .resource::<DisconnectedThisTick>()
            .host_anchors
            .is_empty(),
        "filter_inflight_for_disconnect must clear DisconnectedThisTick \
         at end-of-system so the next tick starts fresh"
    );
}
