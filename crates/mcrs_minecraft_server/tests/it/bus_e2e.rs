use bevy_app::{App, Update};
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_state::prelude::NextState;
use bytes::Bytes;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_engine::between_ticks::drain_dim_spawn_queue;
use mcrs_minecraft_engine::between_ticks::take_dim_outputs;
use mcrs_minecraft_engine::bus::{
    InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerPacket, PacketTarget,
};
use mcrs_minecraft_engine::channel_types::ToDim;
use mcrs_minecraft_engine::coordinator::dim::DimSubAppHandle;
use mcrs_minecraft_engine::muxer::bridge::bridge_inbound_to_channel;
use mcrs_minecraft_engine::muxer::dim_inputs::DimInputs;
use mcrs_minecraft_engine::muxer::session::SessionBundle;
use mcrs_minecraft_engine::persisted_registry::PersistedSnapshot;
use mcrs_minecraft_engine_world::session::{
    Place, PlayerSession, PlayerSessionCounter, SessionPlacement,
};
use mcrs_minecraft_engine_world::world::sub_app::{DimSpawnQueue, DimSpawnRequest};

use crate::host_app;
use crate::mock_connection::{marker, marker_seq};

#[derive(Resource, Default)]
struct OutboundLog(Vec<u32>);

#[derive(Resource, Default)]
struct InboundLog(Vec<i32>);

fn build_app() -> App {
    let mut app = host_app::make_host_app();
    app.init_resource::<PlayerSessionCounter>();
    app.init_resource::<mcrs_minecraft_engine_world::session::SessionAnchors>();
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
            dimension: mcrs_minecraft_dimension::dimension_keys::OVERWORLD.into(),
            dimension_type: crate::support::dimension_type(
                mcrs_minecraft_dimension::dimension_type_keys::OVERWORLD,
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
        log.0.extend(marker_seq(&msg.data));
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

    host_app::world(&mut app, label_entity)
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .write(OutboundPlayerPacket {
            target: PacketTarget::SinglePlayer(PlayerSession(0)),
            data: marker(0xDEAD),
            session: PlayerSession(0),
            epoch: 0,
        });

    // Tick 1: the dimension hands the packet over in its output, which the host takes after
    // the update. record_host_outbound runs in Update, before the dimension ticks.
    app.update();
    take_dim_outputs(app.world_mut());
    let log = app.world().resource::<OutboundLog>().0.clone();
    assert!(
        log.is_empty(),
        "outbound should NOT yet be visible to host after tick 1; log = {log:?}"
    );

    // Tick 2: record_host_outbound drains the packet that the host took after tick 1.
    app.update();
    take_dim_outputs(app.world_mut());
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
        let sub = host_app::sub_app(&mut app, label_entity);
        sub.world_mut().init_resource::<InboundLog>();
        sub.add_systems(Update, record_sub_inbound);
    }

    let host_anchor = app.world_mut().spawn_empty().id();
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
            session,
            id: 0xCAFE,
            data: Bytes::new(),
            trigger: None,
        });

    // bridge_inbound_to_channel (Update) queues it for the dimension's tick, which runs after
    // the host; drain_dim_inbox (FixedPreUpdate) writes it into sub Messages;
    // record_sub_inbound (Update) drains it — all within one app.update().
    app.update();

    let log = host_app::world(&mut app, label_entity)
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

    // Enqueue and spawn the dim. spawn_dim_subapp registers its input before the world first
    // ticks.
    app.world_mut()
        .resource_mut::<DimSpawnQueue>()
        .0
        .push(DimSpawnRequest {
            dimension: mcrs_minecraft_dimension::dimension_keys::THE_END.into(),
            dimension_type: crate::support::dimension_type(
                mcrs_minecraft_dimension::dimension_type_keys::THE_END,
            ),
        });
    drain_dim_spawn_queue(&mut app);

    // Resolve the label entity for the newly spawned dim.
    let label_entity = first_label_entity(&mut app);

    // Install the recorder in the sub-app BEFORE its first tick.
    {
        let sub = host_app::sub_app(&mut app, label_entity);
        sub.world_mut().init_resource::<InboundLog>();
        sub.add_systems(Update, record_sub_inbound);
    }

    let session = PlayerSession(1);

    // Queue messages BEFORE app.update() first ticks the dimension.
    let sent_ok: Vec<bool> = {
        let mut inputs = app.world_mut().resource_mut::<DimInputs>();
        let spawn_result = inputs.send_control(
            label_entity,
            ToDim::Spawn(InboundPlayerSpawn {
                session,
                snapshot: PersistedSnapshot::joining_player(
                    mcrs_minecraft_protocol::uuid::Uuid::nil(),
                    "readiness_player".into(),
                    bevy_math::DVec3::ZERO,
                    bevy_math::Vec2::ZERO,
                    Some(crate::support::client_info_with_view_distance(12)),
                ),
                dimensions: Vec::new().into(),
            }),
        );

        let serverbound_results: Vec<_> = (200i32..203)
            .map(|id| {
                inputs
                    .try_send_serverbound(
                        label_entity,
                        InboundPlayerPacket {
                            session,
                            id,
                            data: Bytes::new(),
                            trigger: None,
                        },
                    )
                    .is_ok()
            })
            .collect();

        std::iter::once(spawn_result)
            .chain(serverbound_results)
            .collect()
    };

    for (i, sent) in sent_ok.iter().enumerate() {
        assert!(*sent, "send {i} is taken before the dim's first tick");
    }

    // First app.update() — sub-app runs for the first time:
    // FixedPreUpdate drain_dim_inbox writes the input in FIFO order,
    // control (Spawn) before serverbound; Update record_sub_inbound logs
    // the serverbound packets (Spawn routes to InboundPlayerSpawn, not
    // InboundPlayerPacket, so it does not appear in InboundLog).
    app.update();

    let log = host_app::world(&mut app, label_entity)
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
