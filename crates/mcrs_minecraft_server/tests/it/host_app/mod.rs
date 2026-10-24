use std::sync::Arc;

use bevy_app::{App, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_math::{DVec3, Vec2};
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::NextState;
use bevy_time::{Fixed, Time, TimePlugin};
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_engine::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerAttached,
    OutboundPlayerDisconnect, OutboundPlayerPacket, PacketPayload, PlayerTransferSnapshot,
};
use mcrs_minecraft_engine::channel_types::{DimChannelsResource, ToDim};
use mcrs_minecraft_engine::session::SessionBundle;
use mcrs_minecraft_level::session::{Place, PlayerSession, PlayerSessionCounter, SessionPlacement};
use mcrs_minecraft_level::world::sub_app::{DimDespawnQueue, DimSpawnQueue, DimSpawnRequest};
use mcrs_minecraft_light::block_light::{BlockLightRegistry, block_light_registry};
use mcrs_minecraft_protocol::packets::game::clientbound::ClientboundLogin;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_server::dim::pump_channels;
use mcrs_minecraft_server::world::sub_app_builder::{DimSubAppHandle, drain_dim_spawn_queue};

/// Build a host `App` wired for the production per-dim sub-app builder path.
///
/// Registers the host-side bus messages, channel resource, spawn/despawn
/// queues and registries that the sub-app extract closure and `pump_channels`
/// read — without them the first `app.update()` after a spawn drain panics.
/// The message set is the union required across the sub-app tests, so callers
/// that need only a subset still get a valid host app.
/// The asset plugin over the repository's assets and the game's data pack, with the pack
/// sources a dimension's sub-app reads.
pub fn add_assets(app: &mut App) {
    let roots = mcrs_minecraft_assets::packs::PackRoots {
        assets: concat!(env!("CARGO_MANIFEST_DIR"), "/../../assets").to_owned(),
        vanilla: mcrs_minecraft_fixtures::vanilla_dir().to_owned(),
    };
    mcrs_minecraft_assets::packs::register_pack_sources(
        app,
        &roots,
        mcrs_minecraft_worldgen_builtin::asset,
    );
    app.add_plugins(AssetPlugin {
        watch_for_changes_override: Some(false),
        file_path: roots.assets,
        ..Default::default()
    });
}

pub fn make_host_app() -> App {
    let mut app = App::new();
    // ChunkPlugin's worldgen startup uses AssetServer::load, which spawns
    // onto IoTaskPool; TaskPoolPlugin must run first or sub-app Startup panics.
    app.add_plugins(TaskPoolPlugin {
        task_pool_options: bevy_app::TaskPoolOptions::with_num_threads(2),
    });
    add_assets(&mut app);
    app.add_plugins(TimePlugin);
    app.insert_resource(Time::<Fixed>::from_hz(20.0));
    app.add_plugins(StatesPlugin);
    app.init_state::<AppState>();

    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<InboundPlayerPacket>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<OutboundPlayerDisconnect>();
    app.add_message::<InboundPlayerDespawn>();

    app.init_resource::<DimChannelsResource>();
    app.init_resource::<DimSpawnQueue>();
    app.init_resource::<DimDespawnQueue>();
    crate::support::insert_registries(&mut app);

    app
}

/// Give the dimensions spawned from this host a lighting engine. Production
/// inserts the same resource at `Startup`.
pub fn enable_lighting(app: &mut App) {
    let blocks = app.world().resource::<Blocks>().clone();
    app.insert_resource(BlockLightRegistry(block_light_registry(&blocks)));
}

/// Transition the host app into `AppState::Playing` and run one update so the
/// state change is applied.
pub fn drive_to_playing(app: &mut App) {
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();
}

/// Push a single dimension spawn request onto the host spawn queue.
pub fn enqueue_spawn(app: &mut App, id: &str, dimension_type: &str) {
    app.world_mut()
        .resource_mut::<DimSpawnQueue>()
        .0
        .push(DimSpawnRequest {
            dimension: ResourceKey::from_location(
                ResourceLocation::read(id).expect("a test dimension id is a valid identifier"),
            ),
            dimension_type: crate::support::dimension_type(dimension_type),
        });
}

/// Enqueue every `(dimension, dimension type)` pair and drain the queue through the
/// production builder, materialising one sub-app per request.
pub fn materialise_sub_apps(app: &mut App, dimensions: &[(&str, &str)]) {
    for (id, dimension_type) in dimensions {
        enqueue_spawn(app, id, dimension_type);
    }
    drain_dim_spawn_queue(app);
}

pub fn label_of(app: &App, dimension: &str) -> Entity {
    app.world()
        .try_query::<(Entity, &DimSubAppHandle, &ResourceKey<Dimension>)>()
        .expect("a dimension was spawned")
        .iter(app.world())
        .find(|(_, _, key)| key.as_str() == dimension)
        .map(|(entity, _, _)| entity)
        .unwrap_or_else(|| panic!("{dimension} was spawned"))
}

/// Places a new session joining `label`, sends its spawn over the dimension's control channel,
/// pumps until the dimension answers and returns the login it sent.
pub fn join(
    app: &mut App,
    label: Entity,
    dimensions: Arc<[ResourceKey<Dimension>]>,
) -> ClientboundLogin {
    let host_anchor = app.world_mut().spawn_empty().id();
    let session = app
        .world_mut()
        .resource_mut::<PlayerSessionCounter>()
        .next();
    app.world_mut()
        .entity_mut(host_anchor)
        .insert(SessionBundle::placed(
            session,
            SessionPlacement::new(Place::Joining(label), 0),
        ));
    app.world()
        .resource::<DimChannelsResource>()
        .get(label)
        .expect("a channel is registered for the dimension")
        .control_sender
        .try_send(ToDim::Spawn(InboundPlayerSpawn {
            host_anchor,
            session: PlayerSession(session.0),
            snapshot: PlayerTransferSnapshot {
                uuid: Uuid::new_v4(),
                username: "joining".into(),
                position: DVec3::new(0.0, 64.0, 0.0),
                rotation: Vec2::ZERO,
                view_distance: 12,
            },
            dimensions,
        }))
        .expect("the control channel is not full");

    for _ in 0..2 {
        app.update();
        pump_channels(app);
    }
    app.world_mut()
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .drain()
        .find_map(|packet| match packet.data {
            PacketPayload::PlayerLogin(login) => Some(login),
            _ => None,
        })
        .expect("a login is sent")
}
