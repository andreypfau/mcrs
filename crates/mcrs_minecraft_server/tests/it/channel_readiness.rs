use bevy_app::{App, TaskPoolPlugin, Update};
use bevy_asset::AssetPlugin;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::*;
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::NextState;
use bevy_time::{Fixed, Time, TimePlugin};
use bytes::Bytes;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_level::session::PlayerSession;
use mcrs_minecraft_level::world::sub_app::{
    DimAppLabel, DimDespawnQueue, DimSpawnQueue, DimSpawnRequest,
};
use mcrs_minecraft_server::world::bus::InboundPlayerSpawn;
use mcrs_minecraft_server::world::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, OutboundPlayerAttached, OutboundPlayerDisconnect,
    OutboundPlayerPacket,
};
use mcrs_minecraft_server::world::channel_types::{DimChannelsResource, ToDim};
use mcrs_minecraft_server::world::sub_app_builder::{DimSubAppHandle, drain_dim_spawn_queue};

use crate::support;

#[derive(Resource, Default)]
struct InboundLog(Vec<i32>);

fn build_app() -> App {
    let mut app = App::new();
    app.add_plugins(TaskPoolPlugin {
        task_pool_options: bevy_app::TaskPoolOptions::with_num_threads(2),
    });
    app.add_plugins(AssetPlugin {
        watch_for_changes_override: Some(false),
        ..Default::default()
    });
    app.add_plugins(TimePlugin);
    app.insert_resource(Time::<Fixed>::from_hz(20.0));
    app.add_plugins(StatesPlugin);
    app.init_state::<AppState>();
    app.init_resource::<DimSpawnQueue>();
    app.init_resource::<DimDespawnQueue>();
    support::insert_registries(&mut app);

    app.init_resource::<mcrs_minecraft_level::session::PlayerSessionCounter>();
    app.init_resource::<DimChannelsResource>();
    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<InboundPlayerPacket>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();

    app
}

fn record_sub_inbound(
    mut msgs: ResMut<Messages<InboundPlayerPacket>>,
    mut log: ResMut<InboundLog>,
) {
    for msg in msgs.drain() {
        log.0.push(msg.id);
    }
}

#[test]
fn messages_buffered_before_dim_boots() {
    let mut app = build_app();

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
    let label_entity = {
        let mut q = app.world_mut().query::<(Entity, &DimSubAppHandle)>();
        let handles: Vec<Entity> = q.iter(app.world()).map(|(e, _)| e).collect();
        assert_eq!(
            handles.len(),
            1,
            "expected exactly one DimSubAppHandle entity"
        );
        handles[0]
    };

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
                snapshot: mcrs_minecraft_server::world::bus::PlayerTransferSnapshot {
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
