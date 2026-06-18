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
use bevy_math::DVec3;
use mcrs_engine::aoi::PlayerObservers;
use mcrs_engine::geometry::ColumnPos;
use mcrs_engine::world::dimension::{DimensionBundle, InDimension};
use mcrs_engine::world::storage::column::{Column, ColumnIndex, ColumnSlot};
use mcrs_engine::session::PlayerSession;
use mcrs_minecraft::disconnect::{
    DisconnectBudget, DisconnectProtocolPlugin, DisconnectedThisTick,
    filter_inflight_for_disconnect, process_disconnect,
};
use mcrs_minecraft::world::aoi::{ChunkSubscriptionSet, TrackedBy};
use mcrs_minecraft::world::bus::{
    InboundPlayerDespawn, InboundPlayerSpawn, OutboundPlayerAttached, OutboundPlayerDisconnect,
    OutboundPlayerPacket, PacketPayload, PacketTarget,
};
use mcrs_minecraft::world::channel_types::{DimChannelsResource, ToDim};
use mcrs_engine::session::{PlayerSessionCounter, SessionEntry, SessionRegistry};
use mcrs_engine::world::channels::{DimSender, FROM_DIM_CAPACITY, TO_DIM_CAPACITY, TO_DIM_CONTROL_CAPACITY};
use mcrs_minecraft::world::channel_types::FromDim;
use mcrs_minecraft::world::player_index::PlayerIndex;

mod harness;
use harness::{
    drain_outbound, drive_aoi_tick, make_aoi_app, run_fixed_pre_update,
    spawn_player_in_dim_with_host_anchor,
};

fn build_disconnect_app() -> App {
    let mut app = App::new();
    app.add_message::<InboundPlayerSpawn>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();
    app.init_resource::<PlayerIndex>();
    app.init_resource::<SessionRegistry>();
    app.init_resource::<PlayerSessionCounter>();
    app.init_resource::<DimChannelsResource>();
    app.add_plugins(DisconnectProtocolPlugin);
    app
}


/// Register a dim channel in the app's `DimChannelsResource` and return the
/// control receiver so tests can assert on `ToDim::Despawn` messages.
fn register_dim_channel(app: &mut App, dim: Entity) -> flume::Receiver<ToDim> {
    let (srv_tx, _srv_rx) = flume::bounded::<ToDim>(TO_DIM_CAPACITY);
    let (ctl_tx, ctl_rx) = flume::bounded::<ToDim>(TO_DIM_CONTROL_CAPACITY);
    let (_from_tx, from_rx) = flume::bounded::<FromDim>(FROM_DIM_CAPACITY);
    app.world_mut()
        .resource_mut::<DimChannelsResource>()
        .insert(dim, DimSender::new(srv_tx), DimSender::new(ctl_tx), from_rx);
    ctl_rx
}

fn count_despawns(rx: &flume::Receiver<ToDim>) -> usize {
    rx.try_iter()
        .filter(|m| matches!(m, ToDim::Despawn { .. }))
        .count()
}

fn insert_location(
    app: &mut App,
    host_anchor: Entity,
    current_dim: Entity,
    previous_dim: Option<Entity>,
    in_dim_entity: Option<Entity>,
) {
    let session = app
        .world_mut()
        .resource_mut::<PlayerSessionCounter>()
        .next();
    app.world_mut().resource_mut::<SessionRegistry>().insert(
        session,
        SessionEntry {
            connection_entity: Entity::PLACEHOLDER,
            host_anchor,
            dim: current_dim,
            previous_dim,
            in_dim_entity,
            epoch: 0,
        },
    );
}

fn synthetic_disconnect(app: &mut App, host_anchor: Entity) {
    app.world_mut()
        .run_system_once(
            move |mut commands: Commands,
                  mut player_index: ResMut<PlayerIndex>,
                  mut session_registry: ResMut<SessionRegistry>,
                  dim_channels: ResMut<DimChannelsResource>,
                  mut disconnected_this_tick: ResMut<DisconnectedThisTick>,
                  mut budget: ResMut<DisconnectBudget>| {
                disconnected_this_tick.host_anchors.push(host_anchor);
                let _ = budget.consume();
                process_disconnect(
                    host_anchor,
                    &mut player_index,
                    &mut session_registry,
                    &dim_channels,
                    &mut mcrs_engine::world::sub_app::DimDespawnQueue::default(),
                    &mut commands,
                );
            },
        )
        .expect("disconnect helper runs");
}

fn run_filter(app: &mut App) {
    app.world_mut()
        .run_system_once(filter_inflight_for_disconnect)
        .expect("filter runs");
}

#[test]
fn disconnect_at_tick_n_e1_3_after_dest_spawn_pre_attach_emit() {
    let mut app = build_disconnect_app();

    let host_anchor = app.world_mut().spawn_empty().id();
    let source_dim = Entity::from_raw_u32(301).unwrap();
    let dest_dim = Entity::from_raw_u32(302).unwrap();
    let src_ctl_rx = register_dim_channel(&mut app, source_dim);
    let dst_ctl_rx = register_dim_channel(&mut app, dest_dim);
    insert_location(&mut app, host_anchor, dest_dim, Some(source_dim), None);

    synthetic_disconnect(&mut app, host_anchor);
    run_filter(&mut app);

    assert_eq!(
        count_despawns(&dst_ctl_rx),
        1,
        "dest dim gets a despawn (current_dim)"
    );
    assert_eq!(
        count_despawns(&src_ctl_rx),
        1,
        "source dim gets a despawn (previous_dim)"
    );

    assert!(
        app.world().resource::<SessionRegistry>().get_by_anchor(&host_anchor).is_none(),
        "SessionRegistry entry removed"
    );
}

#[test]
fn disconnect_at_tick_n_e1_4_attached_pending_filter() {
    let mut app = build_disconnect_app();

    let host_anchor = app.world_mut().spawn_empty().id();
    let source_dim = Entity::from_raw_u32(401).unwrap();
    let dest_dim = Entity::from_raw_u32(402).unwrap();
    let new_in_dim = Entity::from_raw_u32(403).unwrap();
    let src_ctl_rx = register_dim_channel(&mut app, source_dim);
    let dst_ctl_rx = register_dim_channel(&mut app, dest_dim);
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

    assert_eq!(count_despawns(&dst_ctl_rx), 1);
    assert_eq!(count_despawns(&src_ctl_rx), 1);
}

#[test]
fn disconnect_at_tick_n_e1_5_steady_in_dim() {
    let mut app = build_disconnect_app();

    let host_anchor = app.world_mut().spawn_empty().id();
    let dest_dim = Entity::from_raw_u32(501).unwrap();
    let new_in_dim = Entity::from_raw_u32(502).unwrap();
    let dst_ctl_rx = register_dim_channel(&mut app, dest_dim);
    insert_location(&mut app, host_anchor, dest_dim, None, Some(new_in_dim));

    synthetic_disconnect(&mut app, host_anchor);
    run_filter(&mut app);

    assert_eq!(
        count_despawns(&dst_ctl_rx),
        1,
        "single despawn (current_dim only) since previous_dim is None"
    );

    assert!(app.world().resource::<SessionRegistry>().get_by_anchor(&host_anchor).is_none());
}

/// Regression: a transfer-out eviction (via `InboundPlayerDespawn`) has the
/// same AoI cleanup effects as a direct disconnect. Both flow through the
/// shared drain consumer in the per-dim sub-app.
///
/// Asserts:
///   1. O's `TrackedBy` no longer contains T.
///   2. A `PlayerLeftView` packet targeting O carrying T's wire id was emitted.
///   3. T's `ChunkSubscriptionSet` and `TrackedBy` are cleared.
#[test]
fn transfer_out_eviction_matches_disconnect_via_shared_drain() {
    let mut aoi_app = make_aoi_app();
    let dim = aoi_app.world_mut().spawn(DimensionBundle::default()).id();

    let ha_o = aoi_app.world_mut().spawn_empty().id();
    let ha_t = aoi_app.world_mut().spawn_empty().id();

    let player_o =
        spawn_player_in_dim_with_host_anchor(&mut aoi_app, dim, DVec3::new(0.0, 64.0, 0.0), ha_o);

    let player_t = spawn_player_in_dim_with_host_anchor(
        &mut aoi_app,
        dim,
        DVec3::new(0.0, 64.0, 0.0),
        ha_t,
    );

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

    drive_aoi_tick(&mut aoi_app);

    let o_tracked_t_before = aoi_app
        .world()
        .get::<TrackedBy>(player_o)
        .map(|tb| tb.0.contains(&player_t))
        .unwrap_or(false);
    assert!(
        o_tracked_t_before,
        "precondition: O.TrackedBy must contain T before transfer-out eviction"
    );

    let t_sub_before = aoi_app
        .world()
        .get::<ChunkSubscriptionSet>(player_t)
        .map(|css| !css.0.is_empty())
        .unwrap_or(false);
    assert!(
        t_sub_before,
        "precondition: T's ChunkSubscriptionSet must be non-empty before transfer-out eviction"
    );

    let _ = drain_outbound(&mut aoi_app);

    // Inject InboundPlayerDespawn for player_t directly into the sub-app Messages.
    aoi_app
        .world_mut()
        .resource_mut::<Messages<InboundPlayerDespawn>>()
        .write(InboundPlayerDespawn { host_anchor: ha_t, session: PlayerSession(0) });

    run_fixed_pre_update(&mut aoi_app);

    let o_tracked_t_after = aoi_app
        .world()
        .get::<TrackedBy>(player_o)
        .map(|tb| tb.0.contains(&player_t))
        .unwrap_or(true);
    assert!(
        !o_tracked_t_after,
        "O.TrackedBy still contains T after transfer-out eviction via shared drain"
    );

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
