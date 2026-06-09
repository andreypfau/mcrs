//! Mid-transit disconnect cleanup — five scenarios covering each tick of
//! the cross-dim transfer choreography. Each scenario stages the
//! relevant state then drives the disconnect protocol directly via
//! `run_system_once` and `process_disconnect`.
//!
//! Constructing a real `ServerSideConnection` requires a `RawConnection`
//! socket that integration tests cannot reach, so the tests exercise the
//! protocol pipeline (resources + helpers + filter system) rather than
//! the `On<Remove, ServerSideConnection>` trigger itself. The trigger
//! path is the thin shim documented in `player_index_lifecycle.rs`.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::{Commands, ResMut};
use bevy_ecs::system::RunSystemOnce;
use bevy_math::{DVec3, Vec2};
use mcrs_engine::aoi::PlayerObservers;
use mcrs_engine::geometry::ColumnPos;
use mcrs_engine::world::dimension::{DimensionBundle, InDimension};
use mcrs_engine::world::storage::column::{Column, ColumnIndex, ColumnSlot};
use mcrs_minecraft::disconnect::{
    DisconnectBudget, DisconnectProtocolPlugin, DisconnectedThisTick,
    filter_inflight_for_disconnect, process_disconnect,
};
use mcrs_minecraft::world::aoi::{ChunkSubscriptionSet, TrackedBy};
use mcrs_minecraft::world::bridge::bridge_player_transfer;
use mcrs_minecraft::world::bus::{
    InboundPlayerDespawn, InboundPlayerSpawn, OutboundPlayerAttached, OutboundPlayerDisconnect,
    OutboundPlayerPacket, OutboundPlayerTransfer, PacketPayload, PacketTarget,
    PendingInboundLifecycle, PendingInboundPartition, PlayerTransferSnapshot,
};
use mcrs_minecraft::world::player_index::{PlayerIndex, PlayerLocation};
use mcrs_protocol::uuid::Uuid;
use smallvec::SmallVec;

mod harness;
use harness::{
    drain_outbound, drive_aoi_tick, make_aoi_app, run_fixed_pre_update,
    spawn_player_in_dim_with_host_anchor,
};

fn build_disconnect_app() -> App {
    let mut app = App::new();
    app.add_message::<OutboundPlayerTransfer>();
    app.add_message::<InboundPlayerSpawn>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();
    app.init_resource::<PlayerIndex>();
    app.init_resource::<PendingInboundPartition>();
    app.init_resource::<PendingInboundLifecycle>();
    app.add_plugins(DisconnectProtocolPlugin);
    app
}

fn snapshot() -> PlayerTransferSnapshot {
    PlayerTransferSnapshot {
        uuid: Uuid::nil(),
        username: "disco".into(),
        position: DVec3::new(1.0, 64.0, 2.0),
        rotation: Vec2::ZERO,
    }
}

fn insert_location(
    app: &mut App,
    host_anchor: Entity,
    current_dim: Entity,
    previous_dim: Option<Entity>,
    in_dim_entity: Option<Entity>,
) {
    app.world_mut().resource_mut::<PlayerIndex>().insert(
        host_anchor,
        PlayerLocation {
            socket: Entity::PLACEHOLDER,
            current_dim,
            previous_dim,
            in_dim_entity,
            inbound_pending: SmallVec::new(),
        },
    );
}

fn synthetic_disconnect(app: &mut App, host_anchor: Entity) {
    app.world_mut()
        .run_system_once(
            move |mut commands: Commands,
                  mut player_index: ResMut<PlayerIndex>,
                  mut lifecycle: ResMut<PendingInboundLifecycle>,
                  mut disconnected_this_tick: ResMut<DisconnectedThisTick>,
                  mut budget: ResMut<DisconnectBudget>| {
                disconnected_this_tick.host_anchors.push(host_anchor);
                let _ = budget.consume();
                process_disconnect(host_anchor, &mut player_index, &mut lifecycle, &mut commands);
            },
        )
        .expect("disconnect helper runs");
}

fn run_filter(app: &mut App) {
    app.world_mut()
        .run_system_once(filter_inflight_for_disconnect)
        .expect("filter runs");
}

fn run_bridge_transfer(app: &mut App) {
    app.world_mut()
        .run_system_once(bridge_player_transfer)
        .expect("transfer runs");
}

fn drain_lifecycle_despawns(app: &App, dim: Entity) -> usize {
    app.world()
        .resource::<PendingInboundLifecycle>()
        .per_dim
        .get(&dim)
        .map(|b| b.despawns.len())
        .unwrap_or(0)
}

#[test]
fn disconnect_at_tick_n_e1_1_source_emit_pre_extract() {
    let mut app = build_disconnect_app();

    let host_anchor = app.world_mut().spawn_empty().id();
    let source_dim = Entity::from_raw_u32(101).unwrap();
    let dest_dim = Entity::from_raw_u32(102).unwrap();
    insert_location(
        &mut app,
        host_anchor,
        source_dim,
        None,
        Some(Entity::from_raw_u32(7).unwrap()),
    );

    app.world_mut()
        .resource_mut::<Messages<OutboundPlayerTransfer>>()
        .write(OutboundPlayerTransfer {
            host_anchor,
            dest_dim,
            snapshot: snapshot(),
        });

    synthetic_disconnect(&mut app, host_anchor);
    run_filter(&mut app);

    let mut transfers = app
        .world_mut()
        .resource_mut::<Messages<OutboundPlayerTransfer>>();
    let remaining: Vec<_> = transfers.drain().collect();
    assert!(
        remaining.is_empty(),
        "transfer for disconnected anchor must be filtered, got {}",
        remaining.len()
    );

    assert_eq!(
        drain_lifecycle_despawns(&app, source_dim),
        1,
        "source dim sees one InboundPlayerDespawn"
    );

    let index = app.world().resource::<PlayerIndex>();
    assert!(!index.contains(&host_anchor), "PlayerIndex entry removed");
}

#[test]
fn disconnect_at_tick_n_e1_2_after_bridge_transfer() {
    let mut app = build_disconnect_app();

    let host_anchor = app.world_mut().spawn_empty().id();
    // dest_dim must carry DimSubAppHandle so bridge_player_transfer's
    // live-sub-app validation accepts the transfer; source_dim is only
    // ever referenced from PlayerIndex / PendingInboundLifecycle and
    // does not need the marker.
    let source_dim = app.world_mut().spawn_empty().id();
    let dest_dim = app
        .world_mut()
        .spawn(mcrs_minecraft::world::sub_app_builder::DimSubAppHandle)
        .id();
    let in_dim_entity = app.world_mut().spawn_empty().id();
    insert_location(
        &mut app,
        host_anchor,
        source_dim,
        None,
        Some(in_dim_entity),
    );

    app.world_mut()
        .resource_mut::<Messages<OutboundPlayerTransfer>>()
        .write(OutboundPlayerTransfer {
            host_anchor,
            dest_dim,
            snapshot: snapshot(),
        });

    run_bridge_transfer(&mut app);

    let loc = app
        .world()
        .resource::<PlayerIndex>()
        .get(&host_anchor)
        .expect("location after transfer");
    assert_eq!(loc.current_dim, dest_dim);
    assert_eq!(loc.previous_dim, Some(source_dim));
    assert!(loc.in_dim_entity.is_none());
    assert_eq!(
        app.world()
            .resource::<PendingInboundLifecycle>()
            .per_dim
            .get(&dest_dim)
            .map(|b| b.spawns.len())
            .unwrap_or(0),
        1,
        "dest has the pending spawn before disconnect"
    );

    synthetic_disconnect(&mut app, host_anchor);
    run_filter(&mut app);

    let dest_bundle = app
        .world()
        .resource::<PendingInboundLifecycle>()
        .per_dim
        .get(&dest_dim)
        .expect("dest bundle still present");
    assert_eq!(
        dest_bundle.spawns.len(),
        0,
        "pending spawn filtered out by filter_inflight_for_disconnect"
    );

    assert_eq!(
        dest_bundle.despawns.len(),
        1,
        "dest dim gets a despawn (current_dim emit)"
    );
    // Two despawns reach the source dim: one emitted by bridge_player_transfer
    // itself (it now despawns the entity in the dimension being left so that
    // dimension stops streaming its chunks to the relocated client), and one
    // from the disconnect cleanup's previous_dim emit. The second is a harmless
    // no-op once the per-dim consumer has despawned the entity.
    assert_eq!(
        drain_lifecycle_despawns(&app, source_dim),
        2,
        "source dim gets a despawn from the transfer and from the disconnect previous_dim emit"
    );

    assert!(
        !app.world().resource::<PlayerIndex>().contains(&host_anchor),
        "PlayerIndex entry removed"
    );
}

#[test]
fn disconnect_at_tick_n_e1_3_after_dest_spawn_pre_attach_emit() {
    let mut app = build_disconnect_app();

    let host_anchor = app.world_mut().spawn_empty().id();
    let source_dim = Entity::from_raw_u32(301).unwrap();
    let dest_dim = Entity::from_raw_u32(302).unwrap();
    insert_location(&mut app, host_anchor, dest_dim, Some(source_dim), None);

    synthetic_disconnect(&mut app, host_anchor);
    run_filter(&mut app);

    assert_eq!(
        drain_lifecycle_despawns(&app, dest_dim),
        1,
        "dest dim gets a despawn (current_dim)"
    );
    assert_eq!(
        drain_lifecycle_despawns(&app, source_dim),
        1,
        "source dim gets a despawn (previous_dim)"
    );

    assert!(
        !app.world().resource::<PlayerIndex>().contains(&host_anchor),
        "PlayerIndex entry removed"
    );
}

#[test]
fn disconnect_at_tick_n_e1_4_attached_pending_filter() {
    let mut app = build_disconnect_app();

    let host_anchor = app.world_mut().spawn_empty().id();
    let source_dim = Entity::from_raw_u32(401).unwrap();
    let dest_dim = Entity::from_raw_u32(402).unwrap();
    let new_in_dim = Entity::from_raw_u32(403).unwrap();
    insert_location(&mut app, host_anchor, dest_dim, Some(source_dim), None);

    app.world_mut()
        .resource_mut::<Messages<OutboundPlayerAttached>>()
        .write(OutboundPlayerAttached {
            host_anchor,
            new_in_dim_entity: new_in_dim,
        });

    synthetic_disconnect(&mut app, host_anchor);
    run_filter(&mut app);

    let mut attached_msgs = app
        .world_mut()
        .resource_mut::<Messages<OutboundPlayerAttached>>();
    let remaining: Vec<_> = attached_msgs.drain().collect();
    assert!(
        remaining.is_empty(),
        "OutboundPlayerAttached filtered, got {}",
        remaining.len()
    );

    assert_eq!(drain_lifecycle_despawns(&app, dest_dim), 1);
    assert_eq!(drain_lifecycle_despawns(&app, source_dim), 1);
}

#[test]
fn disconnect_at_tick_n_e1_5_steady_in_dim() {
    let mut app = build_disconnect_app();

    let host_anchor = app.world_mut().spawn_empty().id();
    let dest_dim = Entity::from_raw_u32(501).unwrap();
    let new_in_dim = Entity::from_raw_u32(502).unwrap();
    insert_location(&mut app, host_anchor, dest_dim, None, Some(new_in_dim));

    synthetic_disconnect(&mut app, host_anchor);
    run_filter(&mut app);

    assert_eq!(
        drain_lifecycle_despawns(&app, dest_dim),
        1,
        "single despawn (current_dim only) since previous_dim is None"
    );

    assert!(!app.world().resource::<PlayerIndex>().contains(&host_anchor));
}

/// Regression for the transfer-out removal path proving that the downstream
/// eviction effects are identical to the disconnect path because both flow
/// through the same shared drain consumer.
///
/// Step 1: use `bridge_player_transfer` (the real transfer system) to generate
/// the `InboundPlayerDespawn` for the source dimension — proving the transfer
/// path actually emits one.
///
/// Step 2: inject that despawn message directly into a single-App AoI harness
/// with two players (observer O and transferring player T) and drive the drain.
/// Assert the same three eviction truths observed for disconnect in the
/// production-topology test above:
///   1. O's `TrackedBy` no longer contains T.
///   2. A `PlayerLeftView` packet targeting O carrying T's wire id was emitted.
///   3. T's `ChunkSubscriptionSet` and `TrackedBy` are cleared.
///
/// No transfer-specific eviction code is involved — the test passes using
/// only the shared drain consumer, demonstrating parity.
#[test]
fn transfer_out_eviction_matches_disconnect_via_shared_drain() {
    // --- Step 1: run bridge_player_transfer to get an InboundPlayerDespawn ---
    let mut bridge_app = build_disconnect_app();

    let host_anchor = bridge_app.world_mut().spawn_empty().id();
    let source_dim = bridge_app.world_mut().spawn_empty().id();
    let dest_dim = bridge_app
        .world_mut()
        .spawn(mcrs_minecraft::world::sub_app_builder::DimSubAppHandle)
        .id();
    let in_dim_entity = bridge_app.world_mut().spawn_empty().id();
    insert_location(
        &mut bridge_app,
        host_anchor,
        source_dim,
        None,
        Some(in_dim_entity),
    );
    bridge_app
        .world_mut()
        .resource_mut::<Messages<OutboundPlayerTransfer>>()
        .write(OutboundPlayerTransfer {
            host_anchor,
            dest_dim,
            snapshot: snapshot(),
        });
    run_bridge_transfer(&mut bridge_app);

    // Verify: bridge emitted a despawn for the source dim.
    let despawn_msg = bridge_app
        .world()
        .resource::<PendingInboundLifecycle>()
        .per_dim
        .get(&source_dim)
        .and_then(|b| b.despawns.first().cloned());
    let despawn_msg =
        despawn_msg.expect("bridge_player_transfer must push InboundPlayerDespawn for source dim");
    assert_eq!(despawn_msg.host_anchor, host_anchor);

    // --- Step 2: run the drain on a transfer-generated despawn in a single-App AoI harness ---
    //
    // Use a fresh AoI app with its own host_anchor entities. The bridge step above proves
    // that bridge_player_transfer emits InboundPlayerDespawn { host_anchor } for the source
    // dim. This step proves that the drain handles that message identically to disconnect —
    // the two halves together cover the full transfer-out eviction chain.

    let mut aoi_app = make_aoi_app();
    let dim = aoi_app.world_mut().spawn(DimensionBundle::default()).id();

    // Both host anchors are entities inside aoi_app — no cross-app entity id confusion.
    let ha_o = aoi_app.world_mut().spawn_empty().id();
    let ha_t = aoi_app.world_mut().spawn_empty().id();

    // Observer O at position (0, 64, 0).
    let player_o =
        spawn_player_in_dim_with_host_anchor(&mut aoi_app, dim, DVec3::new(0.0, 64.0, 0.0), ha_o);

    // Target T: the player that has transferred out; HostAnchorRef(ha_t) is used
    // by the drain to resolve the in-dim entity from the InboundPlayerDespawn.
    let player_t = spawn_player_in_dim_with_host_anchor(
        &mut aoi_app,
        dim,
        DVec3::new(0.0, 64.0, 0.0),
        ha_t,
    );

    // Seed a column grid with PlayerObservers so update_own_pov can subscribe
    // both players, and update_tracked_by can build TrackedBy caches.
    let radius: i32 = 20;
    {
        let mut col_map: rustc_hash::FxHashMap<ColumnPos, ColumnSlot> =
            rustc_hash::FxHashMap::default();
        for dx in -radius..=radius {
            for dz in -radius..=radius {
                let col_pos = ColumnPos::new(dx, dz);
                let column = aoi_app
                    .world_mut()
                    .spawn((Column, PlayerObservers::default(), InDimension(dim)))
                    .id();
                col_map.insert(col_pos, ColumnSlot { entity: column, section_count: 1 });
            }
        }
        aoi_app
            .world_mut()
            .get_mut::<ColumnIndex>(dim)
            .expect("DimensionBundle provides ColumnIndex")
            .0
            .extend(col_map);
    }

    // Warm-up tick: populate ChunkSubscriptionSet, PlayerObservers, TrackedBy.
    drive_aoi_tick(&mut aoi_app);

    // Non-vacuous precondition: O.TrackedBy must contain T before eviction.
    let o_tracked_t_before = aoi_app
        .world()
        .get::<TrackedBy>(player_o)
        .map(|tb| tb.0.contains(&player_t))
        .unwrap_or(false);
    assert!(
        o_tracked_t_before,
        "precondition: O.TrackedBy must contain T before transfer-out eviction"
    );

    // Non-vacuous precondition: T's ChunkSubscriptionSet must be non-empty.
    let t_sub_before = aoi_app
        .world()
        .get::<ChunkSubscriptionSet>(player_t)
        .map(|css| !css.0.is_empty())
        .unwrap_or(false);
    assert!(
        t_sub_before,
        "precondition: T's ChunkSubscriptionSet must be non-empty before transfer-out eviction"
    );

    // Clear warm-up packets.
    let _ = drain_outbound(&mut aoi_app);

    // Inject InboundPlayerDespawn for player_t — mirroring what the extract
    // closure would do after bridge_player_transfer emits it for the source dim.
    // The despawn_msg.host_anchor from Step 1 confirms the message shape; here
    // we use ha_t (the aoi_app-local anchor) so the drain's HostAnchorRef lookup
    // resolves to the correct in-dim entity without cross-app entity id aliasing.
    aoi_app
        .world_mut()
        .resource_mut::<Messages<InboundPlayerDespawn>>()
        .write(InboundPlayerDespawn { host_anchor: ha_t });

    // Drive only FixedPreUpdate: this runs drain_inbound_player_despawn without
    // also running update_own_pov/update_tracked_by (FixedPostUpdate) which
    // might reset TrackedBy from a Changed<Transform> perspective. The drain
    // is all we're testing here.
    run_fixed_pre_update(&mut aoi_app);

    // Assertion 1: O.TrackedBy no longer contains T.
    let o_tracked_t_after = aoi_app
        .world()
        .get::<TrackedBy>(player_o)
        .map(|tb| tb.0.contains(&player_t))
        .unwrap_or(true);
    assert!(
        !o_tracked_t_after,
        "O.TrackedBy still contains T after transfer-out eviction via shared drain"
    );

    // Assertion 2: a PlayerLeftView targeting O carrying T's wire id was emitted.
    let expected_wire_id = player_t.index_u32() as i32;
    let pkts: Vec<OutboundPlayerPacket> = drain_outbound(&mut aoi_app);
    let left_view_for_o = pkts.iter().any(|pkt| {
        matches!(&pkt.target, PacketTarget::SinglePlayer(e) if *e == player_o)
            && matches!(&pkt.data, PacketPayload::PlayerLeftView { entity_ids }
                if entity_ids.contains(&expected_wire_id))
    });
    assert!(
        left_view_for_o,
        "expected PlayerLeftView targeting O with T's wire id after transfer-out; \
         found {} emitted packets (none matched)",
        pkts.len()
    );

    // Assertion 3: T's ChunkSubscriptionSet and TrackedBy are empty.
    let t_css_empty = aoi_app
        .world()
        .get::<ChunkSubscriptionSet>(player_t)
        .map(|css| css.0.is_empty())
        .unwrap_or(true);
    assert!(
        t_css_empty,
        "T's ChunkSubscriptionSet is non-empty after transfer-out eviction"
    );
    let t_tracked_by_empty = aoi_app
        .world()
        .get::<TrackedBy>(player_t)
        .map(|tb| tb.0.is_empty())
        .unwrap_or(true);
    assert!(
        t_tracked_by_empty,
        "T's TrackedBy is non-empty after transfer-out eviction"
    );
}
