use mcrs_minecraft_engine::muxer::session::GameProfile;
use mcrs_minecraft_resource::{ResourceKeyBuf, ResourceLocationBuf};
use std::sync::Arc;

use bevy_app::{App, SubApp, TaskPoolPlugin};
use bevy_asset::AssetPlugin;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Messages;
use bevy_ecs::prelude::With;
use bevy_ecs::world::World;
use bevy_math::{DVec3, Vec2};
use bevy_state::app::{AppExtStates, StatesPlugin};
use bevy_state::prelude::NextState;
use bevy_time::{Fixed, Time, TimePlugin};
use bytes::Bytes;
use mcrs_minecraft_assets::AppState;
use mcrs_minecraft_block::definition::Blocks;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_engine::ability::{PlayerGameMode, PlayerOpLevel};
use mcrs_minecraft_engine::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerAttached,
    OutboundPlayerDisconnect, OutboundPlayerPacket, PacketPayload, PacketTarget,
    PlayerTransferSnapshot,
};
use mcrs_minecraft_engine::channel_types::ToDim;
use mcrs_minecraft_engine::coordinator::dim::{DimSubAppHandle, take_dim_outputs};
use mcrs_minecraft_engine::coordinator::dim_cells::{DimCells, DimCellsPlugin};
use mcrs_minecraft_engine::coordinator::ops::{DefaultOpLevel, OpList, OpListEntry};
use mcrs_minecraft_engine::coordinator::sub_app_builder::drain_dim_spawn_queue;
use mcrs_minecraft_engine::muxer::dim_inputs::DimInputs;
use mcrs_minecraft_engine::muxer::entity_replication::{self, Answers};
use mcrs_minecraft_engine::muxer::session::{HostAnchorRef, SessionBundle, SessionConnection};
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_engine_world::session::{
    Place, PlayerSession, PlayerSessionCounter, Session, SessionPlacement,
};
use mcrs_minecraft_engine_world::world::sub_app::{
    DimDespawnQueue, DimSpawnQueue, DimSpawnRequest,
};
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_light_engine::block_light::{BlockLightRegistry, block_light_registry};
use mcrs_minecraft_network::ConnectionState;
use mcrs_minecraft_network::metrics::PacketDispatcher;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Decode, Encode, Packet, VarInt};
use mcrs_minecraft_protocol_game::clientbound::ClientboundLogin;
use mcrs_minecraft_vanilla_light::LightingPlugin;

/// Build a host `App` wired for the production per-dim sub-app builder path.
///
/// Registers the host-side bus messages, the dimension cells, the spawn/despawn
/// queues and registries that the dimension tick and `take_dim_outputs`
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
        mcrs_minecraft_assets::packs::PackLayering {
            local_packs: vec!["mcrs".to_owned(), "beta".to_owned()],
            builtin: mcrs_minecraft_worldgen_builtin::asset,
        },
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

    app.add_plugins(DimCellsPlugin);
    app.init_resource::<DimSpawnQueue>();
    app.init_resource::<DimDespawnQueue>();
    crate::support::insert_registries(&mut app);

    app
}

/// Give the dimensions spawned from this host a lighting engine. Production
/// builds the same table at `Startup`.
pub fn enable_lighting(app: &mut App) {
    app.add_plugins(LightingPlugin);
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
            dimension: ResourceKeyBuf::from_location(
                ResourceLocationBuf::read(id).expect("a test dimension id is a valid identifier"),
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

/// The world of the dimension `label`, lent out of its cell until the next update.
///
/// Panics when no dimension has the label.
pub fn sub_app(app: &mut App, label: Entity) -> &mut SubApp {
    app.world_mut()
        .resource_mut::<DimCells>()
        .into_inner()
        .sub_app_mut(label)
        .expect("the dimension is running")
}

/// [`sub_app`]'s world.
pub fn world(app: &mut App, label: Entity) -> &mut World {
    sub_app(app, label).world_mut()
}

/// Every dimension world, lent out as [`sub_app`] lends one, in the order they tick.
pub fn sub_apps(app: &mut App) -> Vec<&mut SubApp> {
    app.world_mut()
        .resource_mut::<DimCells>()
        .into_inner()
        .sub_apps_mut()
        .collect()
}

pub fn label_of(app: &App, dimension: &str) -> Entity {
    app.world()
        .try_query::<(Entity, &DimSubAppHandle, &ResourceKeyBuf<Dimension>)>()
        .expect("a dimension was spawned")
        .iter(app.world())
        .find(|(_, _, key)| key.as_str() == dimension)
        .map(|(entity, _, _)| entity)
        .unwrap_or_else(|| panic!("{dimension} was spawned"))
}

/// Places a new session joining `label`, sends its spawn to the dimension, ticks until the
/// dimension answers and returns the login it sent.
pub fn join(
    app: &mut App,
    label: Entity,
    dimensions: Arc<[ResourceKeyBuf<Dimension>]>,
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
    let sent = app.world_mut().resource_mut::<DimInputs>().send_control(
        label,
        ToDim::Spawn(InboundPlayerSpawn {
            session: PlayerSession(session.0),
            snapshot: PlayerTransferSnapshot {
                uuid: Uuid::new_v4(),
                username: "joining".into(),
                position: DVec3::new(0.0, 64.0, 0.0),
                rotation: Vec2::ZERO,
                client_info: Some(crate::support::client_info_with_view_distance(12)),
            },
            dimensions,
        }),
    );
    assert!(sent, "the dimension takes input");

    for _ in 0..2 {
        app.update();
        take_dim_outputs(app.world_mut());
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

/// A host app in `Playing` with one overworld dimension, whose players join through the
/// dimension's channels.
pub struct PlayServer {
    pub app: App,
    pub dim: Entity,
}

impl PlayServer {
    /// Finishes `app`, lists `ops` as `(uuid, level)` and gives every other player
    /// `default_level`.
    pub fn start(app: App, ops: &[(Uuid, u8)], default_level: u8) -> Self {
        let overworld = [("minecraft:overworld", "minecraft:overworld")];
        Self::start_in(app, &overworld, ops, default_level)
    }

    /// A server that runs `dimensions`; the first one is where `join` places a player.
    pub fn start_in(
        mut app: App,
        dimensions: &[(&str, &str)],
        ops: &[(Uuid, u8)],
        default_level: u8,
    ) -> Self {
        app.finish();
        app.init_resource::<PlayerSessionCounter>();
        app.add_message::<InboundPlayerSpawn>();
        app.insert_resource(OpList::new(ops.iter().map(|&(uuid, level)| OpListEntry {
            uuid,
            name: "op".into(),
            level,
            bypasses_player_limit: false,
        })));
        app.insert_resource(DefaultOpLevel(PlayerOpLevel(default_level)));
        drive_to_playing(&mut app);
        materialise_sub_apps(&mut app, dimensions);
        let dim = label_of(&app, dimensions[0].0);
        Self { app, dim }
    }

    pub fn world(&mut self) -> &mut World {
        self.world_in(self.dim)
    }

    pub fn world_in(&mut self, dim: Entity) -> &mut World {
        world(&mut self.app, dim)
    }

    /// Runs `n` updates and returns the packets sent to `anchor` or to every player.
    pub fn ticks(&mut self, anchor: Entity, n: usize) -> Vec<PacketPayload> {
        let session = self.session_of(anchor);
        let packets = self.ticks_all(n);
        session.map_or_else(Vec::new, |session| sent_to(session, packets))
    }

    /// Runs `n` updates and returns every packet sent.
    pub fn ticks_all(&mut self, n: usize) -> Vec<OutboundPlayerPacket> {
        let mut packets = Vec::new();
        for _ in 0..n {
            self.app.update();
            mcrs_minecraft_engine_server::runner::between_ticks(&mut self.app);
            entity_replication::deliver(self.app.world_mut(), Answers::All);
            packets.extend(
                self.app
                    .world_mut()
                    .resource_mut::<Messages<OutboundPlayerPacket>>()
                    .drain(),
            );
        }
        packets
    }

    pub fn submit_to(&mut self, dim: Entity, message: ToDim) {
        let sent = self
            .app
            .world_mut()
            .resource_mut::<DimInputs>()
            .send_control(dim, message);
        assert!(sent, "the dimension takes the message");
    }

    /// Returns the host anchor of the new player and the packets of its first ticks.
    pub fn join(&mut self, uuid: Uuid, name: &str) -> (Entity, Vec<PacketPayload>) {
        self.join_in(self.dim, uuid, name)
    }

    pub fn join_in(&mut self, dim: Entity, uuid: Uuid, name: &str) -> (Entity, Vec<PacketPayload>) {
        let anchor = self.admit(dim, uuid, name);
        let packets = self.ticks(anchor, 3);
        (anchor, packets)
    }

    /// Places a new session in `dim` and sends its spawn, without running a tick. Returns its
    /// host anchor.
    pub fn admit(&mut self, dim: Entity, uuid: Uuid, name: &str) -> Entity {
        let session = self
            .app
            .world_mut()
            .resource_mut::<PlayerSessionCounter>()
            .next();
        let anchor = self
            .app
            .world_mut()
            .spawn((
                GameProfile {
                    id: uuid,
                    username: name.into(),
                    properties: Vec::new(),
                },
                SessionBundle::placed(session, SessionPlacement::new(Place::Joining(dim), 0)),
            ))
            .id();
        self.app.world_mut().spawn(HostAnchorRef(anchor));
        self.submit_to(
            dim,
            ToDim::Spawn(InboundPlayerSpawn {
                session,
                snapshot: PlayerTransferSnapshot {
                    uuid,
                    username: name.into(),
                    position: DVec3::new(8.5, 64.0, 8.5),
                    rotation: Vec2::ZERO,
                    client_info: Some(crate::support::client_info_with_view_distance(2)),
                },
                dimensions: Vec::new().into(),
            }),
        );
        anchor
    }

    /// Raises `packet` from `anchor` on its connection in the host, then queues it in the
    /// dimension its session is placed in, as the bridge does with a packet from a socket.
    pub fn queue<P: Packet + Encode>(&mut self, anchor: Entity, packet: &P) {
        let mut data = Vec::new();
        packet.encode(&mut data).unwrap();
        let data = Bytes::from(data);
        let trigger = PacketDispatcher::serverbound()
            .with_play(mcrs_minecraft_engine::muxer::game_packets::GAME_SERVERBOUND)
            .dispatch(ConnectionState::Game, P::ID, &data)
            .unwrap();
        let world = self.app.world_mut();
        if let (Some(trigger), Some(connection)) = (
            trigger,
            world
                .get::<SessionConnection>(anchor)
                .map(SessionConnection::entity),
        ) {
            trigger.raise(&mut world.commands(), connection, data.clone());
            world.flush();
        }
        let dim = world
            .get::<SessionPlacement>(anchor)
            .and_then(|placement| placement.place().dim())
            .unwrap_or(self.dim);
        let session = self.session_of(anchor).expect("the anchor has a session");
        let sent = self
            .app
            .world_mut()
            .resource_mut::<DimInputs>()
            .try_send_serverbound(
                dim,
                InboundPlayerPacket {
                    session,
                    id: P::ID,
                    trigger,
                    data,
                },
            );
        assert!(sent.is_ok(), "the dimension takes the packet");
    }

    /// Queues `packet` from `anchor` and returns the packets of the next two ticks.
    pub fn send<P: Packet + Encode>(&mut self, anchor: Entity, packet: &P) -> Vec<PacketPayload> {
        self.queue(anchor, packet);
        self.ticks(anchor, 2)
    }

    pub fn player(&mut self, anchor: Entity) -> Entity {
        self.player_in(self.dim, anchor)
            .expect("the player is in the dimension")
    }

    pub fn player_in(&mut self, dim: Entity, anchor: Entity) -> Option<Entity> {
        let session = self.session_of(anchor)?;
        let world = self.world_in(dim);
        world
            .query_filtered::<(Entity, &Owner), With<Player>>()
            .iter(world)
            .find(|(_, owner)| owner.0 == session)
            .map(|(player, _)| player)
    }

    /// The session the host anchor `anchor` carries, while it is connected.
    pub fn session_of(&self, anchor: Entity) -> Option<PlayerSession> {
        self.app
            .world()
            .get::<Session>(anchor)
            .map(|session| session.0)
    }

    pub fn game_mode(&mut self, anchor: Entity) -> GameMode {
        let player = self.player(anchor);
        self.world().get::<PlayerGameMode>(player).unwrap().0
    }
}

/// The packets of the muxer in `packets`, in order: the id of each packet and its body.
pub fn replicated(packets: &[PacketPayload]) -> Vec<(i32, Vec<u8>)> {
    let mut out = Vec::new();
    for packet in packets {
        let PacketPayload::Replication(chunks) = packet else {
            continue;
        };
        for chunk in chunks {
            let mut rest: &[u8] = chunk;
            while let Some((length, tail)) = rest.split_first_chunk::<4>() {
                let (mut packet, tail) = tail.split_at(u32::from_be_bytes(*length) as usize);
                let id = VarInt::decode(&mut packet)
                    .expect("a record starts with its id")
                    .0;
                out.push((id, packet.to_vec()));
                rest = tail;
            }
        }
    }
    out
}

/// The packets of `packets` that reach `session`, in order.
pub fn sent_to(session: PlayerSession, packets: Vec<OutboundPlayerPacket>) -> Vec<PacketPayload> {
    packets
        .into_iter()
        .filter(|packet| match &packet.target {
            PacketTarget::SinglePlayer(target) => *target == session,
            PacketTarget::PlayerSet(targets) => targets.contains(&session),
            PacketTarget::AllPlayers => true,
            PacketTarget::AllInDim(_) => false,
        })
        .map(|packet| packet.data)
        .collect()
}
