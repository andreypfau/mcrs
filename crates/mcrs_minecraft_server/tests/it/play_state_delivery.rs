//! Production-topology test verifying that the in-dim emitter routes the
//! play-login packet through the bus to the host-resident connection without
//! querying ServerSideConnection.

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_math::DVec3;
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::NextState;
use bevy_time::{Fixed, Time, TimePlugin};
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_level::session::{Place, PlayerSession, PlayerSessionCounter, SessionPlacement};
use mcrs_minecraft_level::world::sub_app::{DimDespawnQueue, DimSpawnQueue, DimSpawnRequest};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_server::dim::pump_channels;
use mcrs_minecraft_server::world::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerAttached,
    OutboundPlayerDisconnect, OutboundPlayerPacket, PacketPayload, PacketTarget,
    PlayerTransferSnapshot,
};
use mcrs_minecraft_server::world::session::SessionBundle;
use mcrs_minecraft_server::world::sub_app_builder::{DimSubAppHandle, drain_dim_spawn_queue};

use crate::support;

// ---------------------------------------------------------------------------
// Helpers
// ---------------------------------------------------------------------------

fn build_host_app() -> App {
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

    app.init_resource::<PlayerSessionCounter>();
    app.init_resource::<mcrs_minecraft_server::world::channel_types::DimChannelsResource>();
    app.init_resource::<mcrs_minecraft_level::world::in_flight::InFlightMoves>();
    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<InboundPlayerPacket>();
    app.add_message::<InboundPlayerSpawn>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();

    app
}

fn spawn_subapp(app: &mut App) -> Entity {
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();
    app.world_mut()
        .resource_mut::<DimSpawnQueue>()
        .0
        .push(DimSpawnRequest {
            dimension: mcrs_minecraft_dimension::keys::dimension::OVERWORLD.into(),
            dimension_type: crate::support::dimension_type(
                mcrs_minecraft_dimension::keys::dimension_type::OVERWORLD.as_str(),
            ),
        });
    drain_dim_spawn_queue(app);
    let mut q = app.world_mut().query::<(Entity, &DimSubAppHandle)>();
    let handles: Vec<Entity> = q.iter(app.world()).map(|(e, _)| e).collect();
    assert_eq!(handles.len(), 1, "expected exactly one DimSubAppHandle");
    handles[0]
}

// ---------------------------------------------------------------------------
// play_login_targets_host_anchor
// ---------------------------------------------------------------------------

/// The PlayerLogin packet emitted by the in-dim system must target
/// `PacketTarget::SinglePlayer(host_anchor)`, not the in-dim entity.
#[test]
fn play_login_targets_host_anchor() {
    let mut app = build_host_app();
    let dim_label = spawn_subapp(&mut app);

    let host_anchor = app.world_mut().spawn_empty().id();
    {
        let session = app
            .world_mut()
            .resource_mut::<PlayerSessionCounter>()
            .next();
        app.world_mut()
            .entity_mut(host_anchor)
            .insert(SessionBundle::placed(
                session,
                SessionPlacement::new(Place::Joining(dim_label), 0),
            ));
    }

    {
        use mcrs_minecraft_server::world::channel_types::ToDim;
        app.world()
            .resource::<mcrs_minecraft_server::world::channel_types::DimChannelsResource>()
            .get(dim_label)
            .expect("channel registered for dim_label")
            .control_sender
            .try_send(ToDim::Spawn(InboundPlayerSpawn {
                host_anchor,
                session: PlayerSession(0),
                snapshot: PlayerTransferSnapshot {
                    uuid: Uuid::new_v4(),
                    username: "target_test".into(),
                    position: DVec3::new(0.0, 64.0, 0.0),
                    rotation: bevy_math::Vec2::ZERO,
                    view_distance: 12,
                },
                dimensions: Vec::new().into(),
            }))
            .expect("control channel not full");
    }

    // Tick 1: sub-app extract runs (nothing yet), then drain_to_dim_inbox routes
    // Spawn; FixedLast flush runs (outbox empty); Update spawn consumer writes
    // OutboundPlayerPacket. pump_channels: channel empty.
    app.update();
    pump_channels(&mut app);

    // Tick 2: sub-app FixedLast flush sends play-login to channel. pump_channels
    // drains it into host Messages<OutboundPlayerPacket>.
    app.update();
    pump_channels(&mut app);

    let packets: Vec<OutboundPlayerPacket> = app
        .world_mut()
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .drain()
        .collect();

    let login_pkt = packets
        .iter()
        .find(|p| matches!(&p.data, PacketPayload::PlayerLogin(_)))
        .expect("PlayerLogin packet must be present");

    match &login_pkt.target {
        PacketTarget::SinglePlayer(e) => {
            assert_eq!(
                *e, host_anchor,
                "PlayerLogin target must be the host-anchor entity, not the in-dim entity"
            );
        }
        other => panic!(
            "PlayerLogin target must be SinglePlayer(host_anchor), got {:?}",
            other
        ),
    }
}

#[test]
fn the_login_lists_the_baked_dimensions_in_order() {
    let list = support::dimension_list_with_extra();
    let mut app = build_host_app();
    let dim_label = spawn_subapp(&mut app);

    let host_anchor = app.world_mut().spawn_empty().id();
    let session = app
        .world_mut()
        .resource_mut::<PlayerSessionCounter>()
        .next();
    app.world_mut()
        .entity_mut(host_anchor)
        .insert(SessionBundle::placed(
            session,
            SessionPlacement::new(Place::Joining(dim_label), 0),
        ));
    app.world()
        .resource::<mcrs_minecraft_server::world::channel_types::DimChannelsResource>()
        .get(dim_label)
        .expect("channel registered for dim_label")
        .control_sender
        .try_send(mcrs_minecraft_server::world::channel_types::ToDim::Spawn(
            InboundPlayerSpawn {
                host_anchor,
                session: PlayerSession(0),
                snapshot: PlayerTransferSnapshot {
                    uuid: Uuid::new_v4(),
                    username: "list_test".into(),
                    position: DVec3::new(0.0, 64.0, 0.0),
                    rotation: bevy_math::Vec2::ZERO,
                    view_distance: 12,
                },
                dimensions: list.keys().clone(),
            },
        ))
        .expect("control channel not full");

    for _ in 0..2 {
        app.update();
        pump_channels(&mut app);
    }

    let login = app
        .world_mut()
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .drain()
        .find_map(|packet| match packet.data {
            PacketPayload::PlayerLogin(login) => Some(login),
            _ => None,
        })
        .expect("PlayerLogin packet must be present");
    let listed: Vec<&str> = login.dimensions.iter().map(|key| key.as_str()).collect();
    assert_eq!(
        listed,
        [
            "minecraft:overworld",
            "minecraft:the_nether",
            "minecraft:the_end",
            "test:extra"
        ]
    );
    assert_eq!(
        login.player_spawn_info.dimension.as_str(),
        "minecraft:overworld"
    );
}
