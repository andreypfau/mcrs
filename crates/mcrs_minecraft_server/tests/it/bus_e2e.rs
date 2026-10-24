use bevy_app::{App, Update};
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_state::prelude::NextState;
use bytes::Bytes;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_engine::bridge::bridge_inbound_to_channel;
use mcrs_minecraft_engine::bus::{
    InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerPacket, PacketPayload, PacketPriority,
    PacketTarget, PlayerTransferSnapshot, TestPayload,
};
use mcrs_minecraft_engine::channel_types::{DimChannelsResource, ToDim};
use mcrs_minecraft_engine::dim::DimSubAppHandle;
use mcrs_minecraft_engine::dim::pump_channels;
use mcrs_minecraft_engine::session::SessionBundle;
use mcrs_minecraft_level::session::{Place, PlayerSession, PlayerSessionCounter, SessionPlacement};
use mcrs_minecraft_level::world::sub_app::{DimAppLabel, DimSpawnQueue, DimSpawnRequest};
use mcrs_minecraft_server::world::sub_app_builder::drain_dim_spawn_queue;

use crate::host_app;

#[derive(Resource, Default)]
struct OutboundLog(Vec<u32>);

#[derive(Resource, Default)]
struct InboundLog(Vec<i32>);

fn build_app() -> App {
    let mut app = host_app::make_host_app();
    app.init_resource::<PlayerSessionCounter>();
    app.init_resource::<mcrs_minecraft_level::world::in_flight::InFlightMoves>();
    app.add_systems(Update, bridge_inbound_to_channel);
    app
}

fn drive_to_playing_and_spawn_subapps(app: &mut App) {
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();
    drain_dim_spawn_queue(app);
}

fn enqueue_overworld(app: &mut App) {
    app.world_mut()
        .resource_mut::<DimSpawnQueue>()
        .0
        .push(DimSpawnRequest {
            dimension: mcrs_minecraft_dimension::keys::dimension::OVERWORLD.into(),
            dimension_type: crate::support::dimension_type(
                mcrs_minecraft_dimension::keys::dimension_type::OVERWORLD.as_str(),
            ),
        });
}

fn first_label_entity(app: &mut App) -> Entity {
    let mut q = app.world_mut().query::<(Entity, &DimSubAppHandle)>();
    let handles: Vec<Entity> = q.iter(app.world()).map(|(e, _)| e).collect();
    assert_eq!(
        handles.len(),
        1,
        "expected exactly one DimSubAppHandle entity"
    );
    handles[0]
}

fn record_host_outbound(
    mut msgs: ResMut<Messages<OutboundPlayerPacket>>,
    mut log: ResMut<OutboundLog>,
) {
    for msg in msgs.drain() {
        let seq = match msg.data {
            PacketPayload::Test(TestPayload { seq }) => seq,
            _ => continue,
        };
        log.0.push(seq);
    }
}

fn record_sub_inbound(
    mut msgs: ResMut<Messages<InboundPlayerPacket>>,
    mut log: ResMut<InboundLog>,
) {
    for msg in msgs.drain() {
        log.0.push(msg.id);
    }
}

fn outbound_latency_is_one_host_tick() {
    let mut app = build_app();

    app.init_resource::<OutboundLog>();
    app.add_systems(Update, record_host_outbound);

    enqueue_overworld(&mut app);
    drive_to_playing_and_spawn_subapps(&mut app);

    let label_entity = first_label_entity(&mut app);

    app.sub_app_mut(DimAppLabel(label_entity))
        .world_mut()
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .write(OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(Entity::PLACEHOLDER),
            priority: PacketPriority::Normal,
            data: PacketPayload::Test(TestPayload { seq: 0xDEAD }),
            session: PlayerSession(0),
            epoch: 0,
        });

    // Tick 1: flush_from_dim_outbox sends to channel; pump_channels drains it.
    // record_host_outbound runs in Update which precedes the pump, so the packet
    // is not yet in the host Messages buffer when Update executes.
    app.update();
    pump_channels(&mut app);
    let log = app.world().resource::<OutboundLog>().0.clone();
    assert!(
        log.is_empty(),
        "outbound should NOT yet be visible to host after tick 1; log = {log:?}"
    );

    // Tick 2: host Messages buffers swap; record_host_outbound drains the packet
    // that pump_channels wrote during tick 1.
    app.update();
    pump_channels(&mut app);
    let log = app.world().resource::<OutboundLog>().0.clone();
    assert_eq!(
        log,
        vec![0xDEAD],
        "outbound visible to host after tick 2 (1 host-tick latency)"
    );
}

fn inbound_latency_is_zero_host_ticks() {
    let mut app = build_app();

    enqueue_overworld(&mut app);
    drive_to_playing_and_spawn_subapps(&mut app);

    let label_entity = first_label_entity(&mut app);

    {
        let sub = app.sub_app_mut(DimAppLabel(label_entity));
        sub.world_mut().init_resource::<InboundLog>();
        sub.add_systems(Update, record_sub_inbound);
    }

    let host_anchor = app.world_mut().spawn_empty().id();
    let player = host_anchor;
    let session = app
        .world_mut()
        .resource_mut::<PlayerSessionCounter>()
        .next();
    app.world_mut()
        .entity_mut(host_anchor)
        .insert(SessionBundle::placed(
            session,
            SessionPlacement::new(Place::InDim(label_entity), 0),
        ));

    app.world_mut()
        .resource_mut::<Messages<InboundPlayerPacket>>()
        .write(InboundPlayerPacket {
            player,
            id: 0xCAFE,
            data: Bytes::new(),
            timestamp: std::time::Instant::now(),
        });

    // bridge_inbound_to_channel (Update) routes to the dim's serverbound channel;
    // drain_to_dim_inbox (FixedPreUpdate) reads it into sub Messages;
    // record_sub_inbound (Update) drains it — all within one app.update().
    app.update();

    let log = app
        .sub_app(DimAppLabel(label_entity))
        .world()
        .resource::<InboundLog>()
        .0
        .clone();
    assert_eq!(
        log,
        vec![0xCAFE],
        "inbound visible inside sub-app after tick 1 (0 host-tick latency)"
    );
}

fn messages_sent_before_a_dimension_boots_arrive_in_order_on_its_first_tick() {
    let mut app = host_app::make_host_app();

    // Transition to Playing so sub-apps can be spawned.
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();

    // Enqueue and spawn the dim. spawn_dim_subapp creates the channel pair
    // before the sub-app's schedule first runs.
    app.world_mut()
        .resource_mut::<DimSpawnQueue>()
        .0
        .push(DimSpawnRequest {
            dimension: mcrs_minecraft_dimension::keys::dimension::THE_END.into(),
            dimension_type: crate::support::dimension_type(
                mcrs_minecraft_dimension::keys::dimension_type::THE_END.as_str(),
            ),
        });
    drain_dim_spawn_queue(&mut app);

    // Resolve the label entity for the newly spawned dim.
    let label_entity = first_label_entity(&mut app);

    // Install the recorder in the sub-app BEFORE its first tick.
    {
        let sub = app.sub_app_mut(DimAppLabel(label_entity));
        sub.world_mut().init_resource::<InboundLog>();
        sub.add_systems(Update, record_sub_inbound);
    }

    let anchor = Entity::from_raw_u32(5).expect("nonzero");

    // Send messages into the channel BEFORE app.update() runs the sub-app.
    // All sends must return Ok without blocking — the bounded channel buffer
    // is the pending queue (structural readiness).
    let sent_ok: Vec<bool> = {
        let channels = app.world().resource::<DimChannelsResource>();
        let entry = channels
            .get(label_entity)
            .expect("channel entry exists before first tick");

        let spawn_result = entry
            .control_sender
            .try_send(ToDim::Spawn(InboundPlayerSpawn {
                host_anchor: anchor,
                session: PlayerSession(1),
                snapshot: PlayerTransferSnapshot {
                    uuid: mcrs_minecraft_protocol::uuid::Uuid::nil(),
                    username: "readiness_player".into(),
                    position: bevy_math::DVec3::ZERO,
                    rotation: bevy_math::Vec2::ZERO,
                    view_distance: 12,
                },
                dimensions: Vec::new().into(),
            }));

        let serverbound_results: Vec<_> = (200i32..203)
            .map(|id| {
                entry
                    .serverbound_sender
                    .try_send(ToDim::Serverbound(InboundPlayerPacket {
                        player: anchor,
                        id,
                        data: Bytes::new(),
                        timestamp: std::time::Instant::now(),
                    }))
                    .is_ok()
            })
            .collect();

        std::iter::once(spawn_result.is_ok())
            .chain(serverbound_results)
            .collect()
    };

    for (i, sent) in sent_ok.iter().enumerate() {
        assert!(
            *sent,
            "send {i} must return Ok without blocking before the dim's first tick"
        );
    }

    // First app.update() — sub-app runs for the first time:
    // FixedPreUpdate drain_to_dim_inbox drains the channel in FIFO order,
    // control (Spawn) before serverbound; Update record_sub_inbound logs
    // the serverbound packets (Spawn routes to InboundPlayerSpawn, not
    // InboundPlayerPacket, so it does not appear in InboundLog).
    app.update();

    let log = app
        .sub_app(DimAppLabel(label_entity))
        .world()
        .resource::<InboundLog>()
        .0
        .clone();

    // The three serverbound ids must arrive in send order.
    assert_eq!(
        log,
        vec![200, 201, 202],
        "buffered serverbound messages must be delivered FIFO on the first dim tick"
    );
}

#[test]
fn the_bus_buffers_until_a_dimension_boots_and_costs_one_host_tick_out_and_none_in() {
    messages_sent_before_a_dimension_boots_arrive_in_order_on_its_first_tick();
    outbound_latency_is_one_host_tick();
    inbound_latency_is_zero_host_ticks();
}
