//! Regression for the deferred-drain bus-filter gap. When
//! drain_pending_disconnects pops a queued host_anchor on tick M, the
//! synchronous on_player_disconnect observer is NOT firing for that
//! anchor this tick — it already fired on the original disconnect tick.
//! Without an explicit push into DisconnectedThisTick at drain time, an
//! OutboundPlayerTransfer for the queued anchor that lands on tick M's
//! bus would slip past filter_inflight_for_disconnect and reach the dest
//! sub-app after PlayerIndex is cleared.
//!
//! This test stages a queued anchor under a saturated budget, then
//! injects an OutboundPlayerTransfer for that anchor between the
//! `First` and `Update` schedules of the drain tick, and asserts the
//! message is filtered before any consumer can observe it.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::system::RunSystemOnce;
use bevy_math::{DVec3, Vec2};
use mcrs_minecraft::disconnect::{
    DisconnectBudget, DisconnectProtocolPlugin, DisconnectedThisTick, PendingDisconnectQueue,
    drain_pending_disconnects, filter_inflight_for_disconnect,
};
use mcrs_minecraft::world::bus::{
    InboundPlayerDespawn, InboundPlayerSpawn, OutboundPlayerAttached,
    OutboundPlayerDisconnect, OutboundPlayerTransfer, PlayerTransferSnapshot,
};
use mcrs_minecraft::world::channel_types::DimChannelsResource;
use mcrs_engine::session::{PlayerSessionCounter, SessionEntry, SessionRegistry};
use mcrs_minecraft::world::player_index::PlayerIndex;
use mcrs_protocol::uuid::Uuid;

fn build_app() -> App {
    let mut app = App::new();
    app.add_message::<OutboundPlayerTransfer>();
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

fn snapshot() -> PlayerTransferSnapshot {
    PlayerTransferSnapshot {
        uuid: Uuid::nil(),
        username: "drained".into(),
        position: DVec3::new(0.0, 64.0, 0.0),
        rotation: Vec2::ZERO,
    }
}

#[test]
fn drain_tick_filters_inflight_transfer_for_queued_anchor() {
    let mut app = build_app();
    let source_dim = Entity::from_raw_u32(1).unwrap();
    let dest_dim = Entity::from_raw_u32(2).unwrap();

    // Allocate a host-anchor and record its location.
    let host_anchor = app.world_mut().spawn_empty().id();
    insert_player(&mut app, host_anchor, source_dim);

    // Stage: saturate the budget so the upcoming disconnect MUST go
    // through the queue (this is the same path E4.1 exercises).
    {
        let mut budget = app.world_mut().resource_mut::<DisconnectBudget>();
        budget.remaining = 0;
    }

    // Queue the disconnect (simulates the observer path under budget
    // pressure: PendingDisconnectQueue.push_back(host_anchor)).
    {
        let mut q = app.world_mut().resource_mut::<PendingDisconnectQueue>();
        assert!(q.push_back(host_anchor), "push under cap succeeds");
    }

    // Now we are on the DRAIN tick. The host_anchor is still in
    // PlayerIndex, the dest sub-app's emit cycle has just produced an
    // OutboundPlayerTransfer for this anchor (e.g., an in-flight
    // cross-dim transfer the player kicked off before the disconnect
    // hit). Inject the transfer message DIRECTLY into the host's
    // Messages buffer to simulate the sub-app extract closure having
    // shuttled it across in `First`-time.
    {
        let mut transfer_msgs = app
            .world_mut()
            .resource_mut::<Messages<OutboundPlayerTransfer>>();
        transfer_msgs.write(OutboundPlayerTransfer {
            host_anchor,
            dest_dim,
            snapshot: snapshot(),
        });
    }

    // Step 1: First-schedule — drain_pending_disconnects pops the
    // queued anchor, pushes it into DisconnectedThisTick, and runs
    // process_disconnect (which removes the PlayerIndex entry and routes
    // an InboundPlayerDespawn into source_dim's lifecycle).
    app.world_mut()
        .run_system_once(drain_pending_disconnects)
        .expect("drain runs");

    // Sanity-check: the drained anchor is recorded in
    // DisconnectedThisTick. THIS IS THE CR-03 INVARIANT.
    let recorded = app
        .world()
        .resource::<DisconnectedThisTick>()
        .host_anchors.contains(&host_anchor);
    assert!(
        recorded,
        "drain_pending_disconnects must push the dequeued host_anchor \
         into DisconnectedThisTick before the same-tick filter pass \
         (CR-03 invariant)"
    );

    // Step 2: Update-schedule — filter_inflight_for_disconnect drains
    // and rewrites the bus, dropping messages whose host_anchor is in
    // DisconnectedThisTick.
    app.world_mut()
        .run_system_once(filter_inflight_for_disconnect)
        .expect("filter runs");

    // The injected OutboundPlayerTransfer MUST have been filtered out.
    // Draining the buffer should produce zero surviving messages.
    let surviving: Vec<OutboundPlayerTransfer> = {
        let mut transfer_msgs = app
            .world_mut()
            .resource_mut::<Messages<OutboundPlayerTransfer>>();
        transfer_msgs.drain().collect()
    };
    assert!(
        surviving.is_empty(),
        "OutboundPlayerTransfer for a just-drained queued anchor must be \
         filtered before the bus consumer sees it; survived = {}",
        surviving.len()
    );

    // SessionRegistry entry is gone (process_disconnect removed it).
    assert!(
        app.world()
            .resource::<SessionRegistry>()
            .get_by_anchor(&host_anchor)
            .is_none(),
        "SessionRegistry entry for drained anchor must be removed by process_disconnect"
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
