//! Per-tick disconnect budget — four scenarios bounding orphan-state
//! accumulation under mass-disconnect pressure.

use bevy_app::App;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, ResMut};
use bevy_ecs::system::RunSystemOnce;
use mcrs_minecraft::disconnect::{
    DisconnectBudget, DisconnectProtocolPlugin, DisconnectedThisTick, OverflowCounter,
    PendingDisconnectQueue, QUEUE_HARD_CAP, drain_pending_disconnects,
    filter_inflight_for_disconnect, process_disconnect,
};
use mcrs_minecraft::world::bus::{
    InboundPlayerDespawn, InboundPlayerSpawn, OutboundPlayerAttached, OutboundPlayerDisconnect,
    OutboundPlayerTransfer, PendingInboundLifecycle, PendingInboundPartition,
};
use mcrs_engine::session::{PlayerSessionCounter, SessionEntry, SessionRegistry};
use mcrs_minecraft::world::player_index::PlayerIndex;

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
    app.init_resource::<PendingInboundPartition>();
    app.init_resource::<PendingInboundLifecycle>();
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

fn insert_player_mid_transit(
    app: &mut App,
    host_anchor: Entity,
    current_dim: Entity,
    previous_dim: Option<Entity>,
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
            in_dim_entity: None,
            epoch: 0,
        },
    );
}

fn spawn_anchors(app: &mut App, count: usize, dim: Entity) -> Vec<Entity> {
    let mut anchors = Vec::with_capacity(count);
    for _ in 0..count {
        let e = app.world_mut().spawn_empty().id();
        insert_player(app, e, dim);
        anchors.push(e);
    }
    anchors
}

/// Drives the observer body once per anchor in the same tick. Mirrors
/// the production `on_player_disconnect` body shape — only the trigger
/// source differs (we can't construct a real `ServerSideConnection` in
/// integration tests).
fn fire_disconnect(app: &mut App, anchors: &[Entity]) {
    let anchors_vec = anchors.to_vec();
    app.world_mut()
        .run_system_once(
            move |mut commands: Commands,
                  mut player_index: ResMut<PlayerIndex>,
                  mut session_registry: ResMut<SessionRegistry>,
                  mut lifecycle: ResMut<PendingInboundLifecycle>,
                  mut budget: ResMut<DisconnectBudget>,
                  mut pending_queue: ResMut<PendingDisconnectQueue>,
                  mut disconnected_this_tick: ResMut<DisconnectedThisTick>,
                  mut overflow_counter: ResMut<OverflowCounter>| {
                for host_anchor in anchors_vec.iter().copied() {
                    disconnected_this_tick.host_anchors.push(host_anchor);
                    if budget.consume() {
                        process_disconnect(
                            host_anchor,
                            &mut player_index,
                            &mut session_registry,
                            &mut lifecycle,
                            &mut commands,
                        );
                    } else if !pending_queue.push_back(host_anchor) {
                        overflow_counter.0 = overflow_counter.0.saturating_add(1);
                    }
                }
            },
        )
        .expect("disconnect batch runs");
}

fn tick_first_schedule(app: &mut App) {
    app.world_mut()
        .run_system_once(drain_pending_disconnects)
        .expect("First-schedule drain");
}

fn tick_update_schedule(app: &mut App) {
    app.world_mut()
        .run_system_once(filter_inflight_for_disconnect)
        .expect("Update-schedule filter");
}

fn dim_despawn_count(app: &App, dim: Entity) -> usize {
    app.world()
        .resource::<PendingInboundLifecycle>()
        .per_dim
        .get(&dim)
        .map(|b| b.despawns.len())
        .unwrap_or(0)
}

#[test]
fn e4_1_100_simultaneous_disconnects_process_32_per_tick() {
    let mut app = build_app();
    let dim = Entity::from_raw_u32(900).unwrap();
    let anchors = spawn_anchors(&mut app, 100, dim);

    fire_disconnect(&mut app, &anchors);
    tick_update_schedule(&mut app);

    assert_eq!(
        dim_despawn_count(&app, dim),
        32,
        "exactly budget-count (32) processed in the first tick",
    );
    assert_eq!(
        app.world().resource::<PendingDisconnectQueue>().entries.len(),
        68,
        "remaining 68 anchors queued",
    );

    // Tick 2: drain refills budget and processes 32 more.
    tick_first_schedule(&mut app);
    tick_update_schedule(&mut app);
    assert_eq!(dim_despawn_count(&app, dim), 64, "32 + 32 processed");
    assert_eq!(
        app.world().resource::<PendingDisconnectQueue>().entries.len(),
        36,
    );

    // Tick 3: another 32.
    tick_first_schedule(&mut app);
    tick_update_schedule(&mut app);
    assert_eq!(dim_despawn_count(&app, dim), 96);
    assert_eq!(
        app.world().resource::<PendingDisconnectQueue>().entries.len(),
        4,
    );

    // Tick 4: remaining 4 drained; queue empty.
    tick_first_schedule(&mut app);
    tick_update_schedule(&mut app);
    assert_eq!(dim_despawn_count(&app, dim), 100);
    assert!(
        app.world()
            .resource::<PendingDisconnectQueue>()
            .entries
            .is_empty(),
        "queue drained after 4 ticks",
    );

    assert!(
        app.world().resource::<SessionRegistry>().iter().count() == 0,
        "every anchor cleared from SessionRegistry",
    );
}

#[test]
fn e4_2_queue_hard_cap_drops_overflow_with_warn() {
    let mut app = build_app();
    let dim = Entity::from_raw_u32(910).unwrap();

    // Saturate the budget so every push from now on goes through the queue.
    {
        let mut budget = app.world_mut().resource_mut::<DisconnectBudget>();
        budget.remaining = 0;
    }

    // Pre-fill the queue to QUEUE_HARD_CAP - 1 with throwaway anchors.
    {
        let mut q = app.world_mut().resource_mut::<PendingDisconnectQueue>();
        let placeholder = Entity::PLACEHOLDER;
        for _ in 0..(QUEUE_HARD_CAP - 1) {
            assert!(q.push_back(placeholder));
        }
        assert_eq!(q.entries.len(), QUEUE_HARD_CAP - 1);
    }

    // Stage 5 disconnects. First one fills the queue to the cap; the
    // remaining four overflow.
    let anchors = spawn_anchors(&mut app, 5, dim);
    let initial = app.world().resource::<OverflowCounter>().0;
    assert_eq!(initial, 0, "counter starts at zero");

    fire_disconnect(&mut app, &anchors);

    assert_eq!(
        app.world().resource::<PendingDisconnectQueue>().entries.len(),
        QUEUE_HARD_CAP,
        "queue saturated at hard cap",
    );
    assert_eq!(
        app.world().resource::<OverflowCounter>().0,
        4,
        "four overflow drops recorded by the counter",
    );
}

#[test]
fn e4_3_reconnect_after_disconnect_no_state_overlap() {
    let mut app = build_app();
    let dim = Entity::from_raw_u32(920).unwrap();

    let host_anchor_1 = app.world_mut().spawn_empty().id();
    insert_player(&mut app, host_anchor_1, dim);

    fire_disconnect(&mut app, &[host_anchor_1]);
    tick_update_schedule(&mut app);

    // At this point host_anchor_1 must be gone before any "reconnect"
    // takes effect.
    assert!(
        app.world().resource::<SessionRegistry>().get_by_anchor(&host_anchor_1).is_none(),
        "anchor_1 evicted before reconnect insert",
    );

    // Reconnect — allocate a fresh anchor entity, mirroring the login
    // observer's behaviour of spawning a new host-anchor per session.
    let host_anchor_2 = app.world_mut().spawn_empty().id();
    insert_player(&mut app, host_anchor_2, dim);

    let registry = app.world().resource::<SessionRegistry>();
    assert!(registry.get_by_anchor(&host_anchor_2).is_some());
    assert!(registry.get_by_anchor(&host_anchor_1).is_none());
    assert_eq!(registry.iter().count(), 1, "no state overlap between sessions");
}

#[test]
fn e4_4_mass_disconnect_interleaved_with_mid_transit_player() {
    let mut app = build_app();
    let source_dim = Entity::from_raw_u32(930).unwrap();
    let dest_dim = Entity::from_raw_u32(931).unwrap();

    // Player A — mid-transit. Insert with previous_dim already set so
    // process_disconnect routes to BOTH dims when A is disconnected.
    let player_a = app.world_mut().spawn_empty().id();
    insert_player_mid_transit(&mut app, player_a, dest_dim, Some(source_dim));

    // 50 bystander anchors, all in dest_dim, all about to disconnect.
    let bystanders = spawn_anchors(&mut app, 50, dest_dim);

    // Drive the disconnect for bystanders + A, all in the same tick.
    let mut all_disconnected: Vec<Entity> = bystanders.clone();
    all_disconnected.push(player_a);
    fire_disconnect(&mut app, &all_disconnected);
    tick_update_schedule(&mut app);

    // Budget is 32 per tick. 32 of the 51 (50 bystanders + A) are
    // processed; the rest queue.
    let processed_dest = dim_despawn_count(&app, dest_dim);
    let queued = app
        .world()
        .resource::<PendingDisconnectQueue>()
        .entries
        .len();
    assert_eq!(
        processed_dest + queued,
        51,
        "every disconnect either processed or queued (no drops)",
    );

    // Drain across more ticks until everything is processed.
    for _ in 0..4 {
        tick_first_schedule(&mut app);
        tick_update_schedule(&mut app);
    }

    assert!(
        app.world().resource::<SessionRegistry>().iter().count() == 0,
        "all anchors evicted within the budget window",
    );

    // Player A's previous_dim despawn must have landed in source_dim
    // (the dual-dim sub-case-1 path); this is the invariant the mass
    // disconnect must not corrupt.
    assert_eq!(
        dim_despawn_count(&app, source_dim),
        1,
        "player A's previous_dim despawn routed regardless of budget contention",
    );
    assert_eq!(
        dim_despawn_count(&app, dest_dim),
        51,
        "all 51 disconnects emit a despawn in dest dim (50 bystanders + A current_dim)",
    );
}
