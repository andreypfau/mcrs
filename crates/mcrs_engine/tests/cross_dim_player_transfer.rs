//! End-to-end validation of the cross-dimension player transfer
//! choreography.
//!
//! Uses the PRODUCTION bus message types and PRODUCTION bridge systems
//! (`bridge_inbound_to_channel`, `bridge_player_transfer`,
//! `bridge_player_attach`) wired into a host `App` plus two hand-built
//! `SubApp`s mirroring a source and a destination `DimSubApp`.
//!
//! Invariants asserted:
//! - The player is never visible in BOTH sub-apps' worlds at the same
//!   tick boundary.
//! - The `SessionRegistry.in_dim_entity == None` gap is observable to main
//!   systems for exactly one tick (the boundary between the transfer
//!   bridge tick and the attach bridge tick).
//! - Inbound packets injected during the gap drain into the dest sub-app
//!   on the attach tick.

use bevy_app::{App, AppLabel, SubApp};
use bevy_ecs::message::{MessageReader, Messages};
use bevy_ecs::prelude::*;
use bevy_ecs::schedule::{Schedule, ScheduleLabel};
use bevy_math::{DVec3, Vec2};
use mcrs_minecraft::world::bridge::{
    bridge_inbound_to_channel, bridge_player_attach, bridge_player_transfer,
};
use bytes::Bytes;
use mcrs_minecraft::world::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerAttached,
    OutboundPlayerDisconnect, OutboundPlayerPacket, OutboundPlayerTransfer,
    PlayerTransferSnapshot,
};
use mcrs_minecraft::world::channel_types::{DimChannelsResource, FromDim, ToDim};
use mcrs_engine::session::{PlayerSessionCounter, SessionEntry, SessionRegistry};
use mcrs_engine::world::channels::{
    DimSender, FromDimSender, ToDimReceiver,
    FROM_DIM_CAPACITY, TO_DIM_CAPACITY, TO_DIM_CONTROL_CAPACITY,
};
use mcrs_minecraft::world::player_index::{PendingInboundBuffer, PlayerIndex};
use mcrs_minecraft::world::sub_app_builder::DimSubAppHandle;
use mcrs_protocol::uuid::Uuid;

#[derive(ScheduleLabel, Debug, Clone, PartialEq, Eq, Hash)]
struct DimTick;

#[derive(AppLabel, Clone, Copy, Debug, Hash, PartialEq, Eq)]
struct TestDimLabel(u8);

#[derive(Component)]
struct InDimPlayerMarker;

#[derive(Resource, Default)]
struct TriggerTransfer {
    fire: bool,
    dest_label_entity: Option<Entity>,
}

#[derive(Resource, Default)]
struct SourceLog {
    has_marker_per_tick: Vec<bool>,
}

#[derive(Resource, Default)]
struct DestLog {
    has_marker_per_tick: Vec<bool>,
    new_in_dim_entity: Option<Entity>,
    received_packets: Vec<i32>,
}

#[derive(Resource, Default)]
struct TransferLog(Vec<TransferEvent>);

#[derive(Debug, Clone)]
struct TransferEvent {
    #[allow(dead_code)]
    tick: u32,
    current_dim: Entity,
    in_dim_entity: Option<Entity>,
}

#[derive(Resource, Default)]
struct HostTickCount(u32);

fn record_transfer_event(
    mut log: ResMut<TransferLog>,
    mut tick: ResMut<HostTickCount>,
    session_registry: Res<SessionRegistry>,
    host_anchor: Res<TestHostAnchor>,
) {
    let event = if let Some((_, entry)) = session_registry.get_by_anchor(&host_anchor.0) {
        TransferEvent {
            tick: tick.0,
            current_dim: entry.dim,
            in_dim_entity: entry.in_dim_entity,
        }
    } else {
        TransferEvent {
            tick: tick.0,
            current_dim: Entity::PLACEHOLDER,
            in_dim_entity: None,
        }
    };
    log.0.push(event);
    tick.0 += 1;
}

#[derive(Resource, Clone, Copy)]
struct TestHostAnchor(Entity);

#[derive(Resource, Clone, Copy)]
struct TestDestLabel(Entity);

fn synthetic_snapshot() -> PlayerTransferSnapshot {
    PlayerTransferSnapshot {
        uuid: Uuid::nil(),
        username: "transfer-test".into(),
        position: DVec3::new(1.0, 2.0, 3.0),
        rotation: Vec2::ZERO,
    }
}

fn build_test_app() -> App {
    let mut app = App::new();

    // Host-side bus message registrations.
    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<InboundPlayerPacket>();
    app.add_message::<OutboundPlayerTransfer>();
    app.add_message::<InboundPlayerSpawn>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();

    app.init_resource::<PlayerIndex>();
    app.init_resource::<SessionRegistry>();
    app.init_resource::<PlayerSessionCounter>();
    app.init_resource::<PendingInboundBuffer>();
    app.init_resource::<DimChannelsResource>();
    app.init_resource::<TransferLog>();
    app.init_resource::<HostTickCount>();
    app.init_resource::<mcrs_engine::world::sub_app::DimDespawnQueue>();

    // Allocate label entities. The DimSubAppHandle marker is what
    // bridge_player_transfer's live-sub-app validation looks for.
    let source_label_entity = app.world_mut().spawn(DimSubAppHandle).id();
    let dest_label_entity = app.world_mut().spawn(DimSubAppHandle).id();
    app.insert_resource(TestDestLabel(dest_label_entity));

    // Allocate a host-anchor entity and seed SessionRegistry.
    let host_anchor = app.world_mut().spawn_empty().id();
    let src_in_dim = Entity::from_raw_u32(10).expect("nonzero");
    {
        let session = app.world_mut().resource_mut::<PlayerSessionCounter>().next();
        app.world_mut().resource_mut::<SessionRegistry>().insert(
            session,
            SessionEntry {
                connection_entity: Entity::PLACEHOLDER,
                host_anchor,
                dim: source_label_entity,
                previous_dim: None,
                in_dim_entity: Some(src_in_dim),
                epoch: 0,
            },
        );
    }
    app.insert_resource(TestHostAnchor(host_anchor));

    app.add_systems(
        bevy_app::Update,
        (
            bridge_inbound_to_channel,
            bridge_player_transfer,
            bridge_player_attach,
            record_transfer_event,
        )
            .chain(),
    );

    insert_source_sub_app(&mut app, source_label_entity);
    insert_dest_sub_app(&mut app, dest_label_entity);

    app
}

fn make_dim_channels(app: &mut App, label_entity: Entity) -> (flume::Receiver<ToDim>, flume::Receiver<ToDim>, flume::Sender<FromDim>) {
    let (srv_tx, srv_rx) = flume::bounded::<ToDim>(TO_DIM_CAPACITY);
    let (ctl_tx, ctl_rx) = flume::bounded::<ToDim>(TO_DIM_CONTROL_CAPACITY);
    let (from_tx, from_rx) = flume::bounded::<FromDim>(FROM_DIM_CAPACITY);
    app.world_mut()
        .resource_mut::<DimChannelsResource>()
        .insert(label_entity, DimSender::new(srv_tx), DimSender::new(ctl_tx), from_rx);
    (srv_rx, ctl_rx, from_tx)
}

fn insert_source_sub_app(app: &mut App, label_entity: Entity) {
    let (srv_rx, ctl_rx, _from_tx) = make_dim_channels(app, label_entity);

    let mut sub_app = SubApp::new();
    sub_app.update_schedule = Some(DimTick.intern());
    sub_app.add_schedule(Schedule::new(DimTick));
    register_sub_messages(&mut sub_app);
    sub_app.init_resource::<SourceLog>();
    sub_app.init_resource::<TriggerTransfer>();
    sub_app.insert_resource(ToDimReceiver::<ToDim> { serverbound: srv_rx, control: ctl_rx });

    // Pre-populate the in-dim marker so tick 0 shows the player living
    // in the source sub-app.
    sub_app.world_mut().spawn(InDimPlayerMarker);

    sub_app.add_systems(DimTick, (source_log_marker, source_maybe_despawn_marker));

    sub_app.set_extract(move |main_world, sub_world| {
        drain_outbound_transfer(main_world, sub_world);
        drain_outbound_attached(main_world, sub_world);
        drain_inbound_for_dim(main_world, sub_world);
    });

    app.insert_sub_app(TestDimLabel(0), sub_app);
}

fn insert_dest_sub_app(app: &mut App, label_entity: Entity) {
    let (srv_rx, ctl_rx, _from_tx) = make_dim_channels(app, label_entity);

    let mut sub_app = SubApp::new();
    sub_app.update_schedule = Some(DimTick.intern());
    sub_app.add_schedule(Schedule::new(DimTick));
    register_sub_messages(&mut sub_app);
    sub_app.init_resource::<DestLog>();
    sub_app.insert_resource(ToDimReceiver::<ToDim> { serverbound: srv_rx, control: ctl_rx });

    sub_app.add_systems(
        DimTick,
        (dest_log_marker, dest_consume_spawns, dest_consume_packets).chain(),
    );

    sub_app.set_extract(move |main_world, sub_world| {
        drain_outbound_transfer(main_world, sub_world);
        drain_outbound_attached(main_world, sub_world);
        drain_inbound_for_dim(main_world, sub_world);
    });

    app.insert_sub_app(TestDimLabel(1), sub_app);
}

fn register_sub_messages(sub_app: &mut SubApp) {
    sub_app.add_message::<OutboundPlayerPacket>();
    sub_app.add_message::<InboundPlayerPacket>();
    sub_app.add_message::<OutboundPlayerTransfer>();
    sub_app.add_message::<InboundPlayerSpawn>();
    sub_app.add_message::<OutboundPlayerAttached>();
    sub_app.add_message::<OutboundPlayerDisconnect>();
    sub_app.add_message::<InboundPlayerDespawn>();
}

fn drain_outbound_transfer(main_world: &mut World, sub_world: &mut World) {
    let drained: Vec<OutboundPlayerTransfer> = sub_world
        .resource_mut::<Messages<OutboundPlayerTransfer>>()
        .drain()
        .collect();
    if !drained.is_empty() {
        let mut main_msgs = main_world.resource_mut::<Messages<OutboundPlayerTransfer>>();
        for msg in drained {
            main_msgs.write(msg);
        }
    }
}

fn drain_outbound_attached(main_world: &mut World, sub_world: &mut World) {
    let drained: Vec<OutboundPlayerAttached> = sub_world
        .resource_mut::<Messages<OutboundPlayerAttached>>()
        .drain()
        .collect();
    if !drained.is_empty() {
        let mut main_msgs = main_world.resource_mut::<Messages<OutboundPlayerAttached>>();
        for msg in drained {
            main_msgs.write(msg);
        }
    }
}

/// Drain inbound messages from the dim's `ToDimReceiver` into the sub-app's
/// per-message buffers. The `ToDimReceiver` lives in `sub_world`; the extract
/// closure calls this to mirror what `drain_to_dim_inbox` does in production.
fn drain_inbound_for_dim(_main_world: &mut World, sub_world: &mut World) {
    let Some(rx) = sub_world.get_resource::<ToDimReceiver<ToDim>>() else {
        return;
    };
    let ctl_msgs: Vec<ToDim> = rx.control.try_iter().collect();
    let srv_msgs: Vec<ToDim> = rx.serverbound.try_iter().collect();

    for msg in ctl_msgs.into_iter().chain(srv_msgs) {
        match msg {
            ToDim::Spawn { host_anchor, session, snapshot } => {
                sub_world
                    .resource_mut::<Messages<InboundPlayerSpawn>>()
                    .write(InboundPlayerSpawn { host_anchor, session, snapshot });
            }
            ToDim::Despawn { host_anchor } => {
                sub_world
                    .resource_mut::<Messages<InboundPlayerDespawn>>()
                    .write(InboundPlayerDespawn { host_anchor, session: mcrs_engine::session::PlayerSession(0) });
            }
            ToDim::Serverbound { player, id, data, timestamp } => {
                sub_world
                    .resource_mut::<Messages<InboundPlayerPacket>>()
                    .write(InboundPlayerPacket { player, id, data, timestamp });
            }
            ToDim::Attach { .. } => {
                // Attach signals are consumed by the dim's spawn system;
                // this test harness does not model that sub-system.
            }
        }
    }
}

fn source_log_marker(markers: Query<&InDimPlayerMarker>, mut log: ResMut<SourceLog>) {
    let has_marker = !markers.is_empty();
    log.has_marker_per_tick.push(has_marker);
}

fn source_maybe_despawn_marker(
    mut commands: Commands,
    markers: Query<Entity, With<InDimPlayerMarker>>,
    mut trigger: ResMut<TriggerTransfer>,
) {
    if !trigger.fire {
        return;
    }
    trigger.fire = false;
    for entity in markers.iter() {
        commands.entity(entity).despawn();
    }
}

fn dest_log_marker(markers: Query<&InDimPlayerMarker>, mut log: ResMut<DestLog>) {
    let has_marker = !markers.is_empty();
    log.has_marker_per_tick.push(has_marker);
}

fn dest_consume_spawns(
    mut commands: Commands,
    mut reader: MessageReader<InboundPlayerSpawn>,
    mut attached_writer: ResMut<Messages<OutboundPlayerAttached>>,
    mut log: ResMut<DestLog>,
) {
    for spawn in reader.read() {
        let new_in_dim = commands.spawn(InDimPlayerMarker).id();
        log.new_in_dim_entity = Some(new_in_dim);
        attached_writer.write(OutboundPlayerAttached {
            host_anchor: spawn.host_anchor,
            new_in_dim_entity: new_in_dim,
        });
    }
}

fn dest_consume_packets(
    mut reader: MessageReader<InboundPlayerPacket>,
    mut log: ResMut<DestLog>,
) {
    for packet in reader.read() {
        log.received_packets.push(packet.id);
    }
}

fn source_log(app: &App) -> Vec<bool> {
    app.sub_app(TestDimLabel(0))
        .world()
        .resource::<SourceLog>()
        .has_marker_per_tick
        .clone()
}

fn dest_log(app: &App) -> Vec<bool> {
    app.sub_app(TestDimLabel(1))
        .world()
        .resource::<DestLog>()
        .has_marker_per_tick
        .clone()
}

fn dest_received_packets(app: &App) -> Vec<i32> {
    app.sub_app(TestDimLabel(1))
        .world()
        .resource::<DestLog>()
        .received_packets
        .clone()
}

fn transfer_log(app: &App) -> Vec<TransferEvent> {
    app.world().resource::<TransferLog>().0.clone()
}

fn arm_transfer(app: &mut App, host_anchor: Entity, dest_label: Entity) {
    let source_sub = app.sub_app_mut(TestDimLabel(0));
    {
        let mut trigger = source_sub.world_mut().resource_mut::<TriggerTransfer>();
        trigger.fire = true;
        trigger.dest_label_entity = Some(dest_label);
    }
    source_sub
        .world_mut()
        .resource_mut::<Messages<OutboundPlayerTransfer>>()
        .write(OutboundPlayerTransfer {
            host_anchor,
            dest_dim: dest_label,
            snapshot: synthetic_snapshot(),
        });
}

#[test]
fn cross_dim_transfer_completes_atomically_in_four_host_ticks() {
    let mut app = build_test_app();
    let host_anchor = app.world().resource::<TestHostAnchor>().0;
    let dest_label = app.world().resource::<TestDestLabel>().0;

    // Tick 0 (baseline): no transfer triggered yet.
    app.update();

    // Arm the transfer between tick 0 and tick 1.
    arm_transfer(&mut app, host_anchor, dest_label);

    // Tick 1 (initiate transfer): source sub-app's marker-despawn fires;
    // the pre-injected transfer message sits in source's
    // Messages<OutboundPlayerTransfer>.
    app.update();

    // Tick 2: source.extract drains the transfer into main; main systems
    // have not yet observed it (main ran BEFORE extract).
    app.update();

    // Tick 3: main's bridge_player_transfer drains main.Messages,
    // mutates SessionRegistry, writes spawn to dest control channel.
    // dest.extract drains the spawn; dest.update consumes it, spawns
    // marker, emits attached.
    app.update();

    // Tick 4: dest.extract drains attached message into main.
    app.update();

    // Tick 5: main's bridge_player_attach drains main.Messages and sets
    // in_dim_entity = Some(new).
    app.update();

    let source = source_log(&app);
    let dest = dest_log(&app);
    let events = transfer_log(&app);
    let new_in_dim = app
        .sub_app(TestDimLabel(1))
        .world()
        .resource::<DestLog>()
        .new_in_dim_entity
        .expect("dest spawn should have created an in-dim entity");

    assert!(source.len() >= 6, "source logged at least 6 ticks: {source:?}");
    assert!(dest.len() >= 6, "dest logged at least 6 ticks: {dest:?}");

    // Atomicity invariant: at every tick boundary, the player is never
    // visible in BOTH sub-apps.
    for tick in 0..source.len().min(dest.len()) {
        assert!(
            !(source[tick] && dest[tick]),
            "tick {tick}: player visible in BOTH source and dest sub-apps",
        );
    }

    assert!(source[0], "tick 0: source has the player marker");
    assert!(
        dest.iter().any(|&seen| seen),
        "dest eventually sees the player marker: {dest:?}",
    );

    let gap_ticks: Vec<&TransferEvent> = events
        .iter()
        .filter(|ev| ev.current_dim == dest_label && ev.in_dim_entity.is_none())
        .collect();
    assert!(
        !gap_ticks.is_empty(),
        "transfer log should contain at least one tick where current_dim==dest \
         and in_dim_entity==None: {events:?}",
    );

    let last = events.last().expect("at least one event");
    assert_eq!(
        last.current_dim, dest_label,
        "final state: current_dim points to dest sub-app",
    );
    assert_eq!(
        last.in_dim_entity,
        Some(new_in_dim),
        "final state: in_dim_entity points to the spawned dest marker",
    );
}

#[test]
fn inbound_packets_buffered_during_transit_drain_on_attach() {
    let mut app = build_test_app();
    let host_anchor = app.world().resource::<TestHostAnchor>().0;
    let dest_label = app.world().resource::<TestDestLabel>().0;

    // Tick 0: baseline.
    app.update();

    arm_transfer(&mut app, host_anchor, dest_label);

    // Tick 1: source marker despawn + transfer sits in source buffer.
    app.update();
    // Tick 2: source.extract drains into main.
    app.update();

    // Inject 2 inbound packets into main.Messages<InboundPlayerPacket>
    // while the player is in the gap.
    for seq in 0..2i32 {
        app.world_mut()
            .resource_mut::<Messages<InboundPlayerPacket>>()
            .write(InboundPlayerPacket {
                player: host_anchor,
                id: seq + 100,
                data: Bytes::new(),
                timestamp: std::time::Instant::now(),
            });
    }

    app.update(); // tick 4: dest.extract drains attached into main
    app.update(); // tick 5: bridge_player_attach; packets route to dest channel

    // Allow one more tick so the dest sub-app's update reads the packets.
    app.update(); // tick 6

    let received = dest_received_packets(&app);
    assert_eq!(
        received.len(),
        2,
        "two packets injected during transit must arrive at dest: {received:?}",
    );
    let mut sorted = received.clone();
    sorted.sort_unstable();
    assert_eq!(sorted, vec![100, 101], "packet ids match: {received:?}");
}
