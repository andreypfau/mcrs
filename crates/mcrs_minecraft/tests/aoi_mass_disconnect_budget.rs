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
use mcrs_minecraft::world::player_index::{PlayerIndex, PlayerLocation};
use smallvec::SmallVec;

fn build_app() -> App {
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

fn make_location(dim: Entity) -> PlayerLocation {
    PlayerLocation {
        socket: Entity::PLACEHOLDER,
        current_dim: dim,
        previous_dim: None,
        in_dim_entity: Some(Entity::PLACEHOLDER),
        inbound_pending: SmallVec::new(),
    }
}

fn spawn_anchors(app: &mut App, count: usize, dim: Entity) -> Vec<Entity> {
    let mut anchors = Vec::with_capacity(count);
    for _ in 0..count {
        let e = app.world_mut().spawn_empty().id();
        app.world_mut()
            .resource_mut::<PlayerIndex>()
            .insert(e, make_location(dim));
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
        app.world().resource::<PlayerIndex>().is_empty(),
        "every anchor cleared from PlayerIndex",
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
    app.world_mut()
        .resource_mut::<PlayerIndex>()
        .insert(host_anchor_1, make_location(dim));

    fire_disconnect(&mut app, &[host_anchor_1]);
    tick_update_schedule(&mut app);

    // At this point host_anchor_1 must be gone before any "reconnect"
    // takes effect.
    assert!(
        !app.world().resource::<PlayerIndex>().contains(&host_anchor_1),
        "anchor_1 evicted before reconnect insert",
    );

    // Reconnect — allocate a fresh anchor entity, mirroring the login
    // observer's behaviour of spawning a new host-anchor per session.
    let host_anchor_2 = app.world_mut().spawn_empty().id();
    app.world_mut()
        .resource_mut::<PlayerIndex>()
        .insert(host_anchor_2, make_location(dim));

    let index = app.world().resource::<PlayerIndex>();
    assert!(index.contains(&host_anchor_2));
    assert!(!index.contains(&host_anchor_1));
    assert_eq!(index.len(), 1, "no state overlap between sessions");
}

#[test]
fn e4_4_mass_disconnect_interleaved_with_mid_transit_player() {
    let mut app = build_app();
    let source_dim = Entity::from_raw_u32(930).unwrap();
    let dest_dim = Entity::from_raw_u32(931).unwrap();

    // Player A — mid-transit. Insert with previous_dim already set so
    // process_disconnect routes to BOTH dims when A is disconnected.
    let player_a = app.world_mut().spawn_empty().id();
    app.world_mut().resource_mut::<PlayerIndex>().insert(
        player_a,
        PlayerLocation {
            socket: Entity::PLACEHOLDER,
            current_dim: dest_dim,
            previous_dim: Some(source_dim),
            in_dim_entity: None,
            inbound_pending: SmallVec::new(),
        },
    );

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
        app.world().resource::<PlayerIndex>().is_empty(),
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
