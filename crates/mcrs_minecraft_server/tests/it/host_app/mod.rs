use mcrs_minecraft_engine::game_profile::GameProfile;
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
use mcrs_minecraft_engine::ability::PlayerOpLevel;
use mcrs_minecraft_engine::between_ticks::drain_dim_spawn_queue;
use mcrs_minecraft_engine::between_ticks::take_dim_outputs;
use mcrs_minecraft_engine::bus::{
    InboundPlayerDespawn, InboundPlayerPacket, InboundPlayerSpawn, OutboundPlayerAttached,
    OutboundPlayerPacket, PacketPayload, PacketTarget,
};
use mcrs_minecraft_engine::channel_types::ToDim;
use mcrs_minecraft_engine::coordinator::dim::DimSubAppHandle;
use mcrs_minecraft_engine::coordinator::dim_cells::{DimCells, DimCellsPlugin};
use mcrs_minecraft_engine::coordinator::ops::{DefaultOpLevel, OpList, OpListEntry};
use mcrs_minecraft_engine::muxer::dim_inputs::{DimInputs, DimInputsPlugin};
use mcrs_minecraft_engine::muxer::muxer_link::{self, Answers};
use mcrs_minecraft_engine::muxer::session::{HostAnchorRef, SessionBundle, SessionConnection};
use mcrs_minecraft_engine::persisted_registry::PersistedSnapshot;
use mcrs_minecraft_engine::to_coordinator::JoinRequested;
use mcrs_minecraft_engine::to_muxer::JoinPlaced;
use mcrs_minecraft_engine_world::entity::player::Player;
use mcrs_minecraft_engine_world::session::Owner;
use mcrs_minecraft_engine_world::session::{
    Place, PlayerSession, PlayerSessionCounter, Session, SessionPlacement,
};
use mcrs_minecraft_engine_world::world::sub_app::{
    DimDespawnQueue, DimSpawnQueue, DimSpawnRequest,
};
use mcrs_minecraft_game_mode::{GameMode, PlayerGameMode};
use mcrs_minecraft_light_engine::block_light::{BlockLightRegistry, block_light_registry};
use mcrs_minecraft_network::metrics::PacketDispatcher;
use mcrs_minecraft_network::{ConnectionState, Instant};
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_protocol::{Decode, Encode, Packet, VarInt};
use mcrs_minecraft_protocol_game::clientbound::ClientboundLogin;
use mcrs_minecraft_vanilla_light::LightingPlugin;

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
    app.add_plugins(TaskPoolPlugin {
        task_pool_options: bevy_app::TaskPoolOptions::with_num_threads(2),
    });
    add_assets(&mut app);
    app.add_plugins(TimePlugin);
    app.insert_resource(Time::<Fixed>::from_hz(20.0));
    app.add_plugins(StatesPlugin);
    app.add_plugins(mcrs_minecraft_engine::PlayerPersistencePlugin);
    app.init_state::<AppState>();

    app.add_message::<OutboundPlayerPacket>();
    app.add_message::<InboundPlayerPacket>();
    app.add_message::<OutboundPlayerAttached>();
    app.add_message::<InboundPlayerDespawn>();

    app.add_plugins(DimCellsPlugin);
    app.add_plugins(DimInputsPlugin);
    app.add_message::<JoinRequested>();
    app.add_message::<JoinPlaced>();
    app.init_resource::<DimSpawnQueue>();
    app.init_resource::<DimDespawnQueue>();
    crate::support::insert_registries(&mut app);

    app
}

pub fn enable_lighting(app: &mut App) {
    app.add_plugins(LightingPlugin);
    let blocks = app.world().resource::<Blocks>().clone();
    app.insert_resource(BlockLightRegistry(block_light_registry(&blocks)));
}

pub fn drive_to_playing(app: &mut App) {
    app.world_mut()
        .resource_mut::<NextState<AppState>>()
        .set(AppState::Playing);
    app.update();
}

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

pub fn materialise_sub_apps(app: &mut App, dimensions: &[(&str, &str)]) {
    for (id, dimension_type) in dimensions {
        enqueue_spawn(app, id, dimension_type);
    }
    drain_dim_spawn_queue(app);
}

pub fn sub_app(app: &mut App, label: Entity) -> &mut SubApp {
    app.world_mut()
        .resource_mut::<DimCells>()
        .into_inner()
        .cell_sub_apps_mut(label)
        .next()
        .expect("the dimension is running")
}

pub fn world(app: &mut App, label: Entity) -> &mut World {
    sub_app(app, label).world_mut()
}

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
            snapshot: PersistedSnapshot::joining_player(
                Uuid::new_v4(),
                "joining".into(),
                DVec3::new(0.0, 64.0, 0.0),
                Vec2::ZERO,
                Some(crate::support::client_info_with_view_distance(12)),
            ),
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

pub struct PlayServer {
    pub app: App,
    pub dim: Entity,
}

impl PlayServer {
    pub fn start(app: App, ops: &[(Uuid, u8)], default_level: u8) -> Self {
        let overworld = [("minecraft:overworld", "minecraft:overworld")];
        Self::start_in(app, &overworld, ops, default_level)
    }

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

    pub fn ticks(&mut self, anchor: Entity, n: usize) -> Vec<PacketPayload> {
        let session = self.session_of(anchor);
        let packets = self.ticks_all(n);
        session.map_or_else(Vec::new, |session| sent_to(session, packets))
    }

    pub fn ticks_all(&mut self, n: usize) -> Vec<OutboundPlayerPacket> {
        let mut packets = Vec::new();
        for _ in 0..n {
            packets.extend(tick(&mut self.app));
        }
        packets
    }

    pub fn submit_to(&mut self, dim: Entity, message: ToDim) {
        submit_to(&mut self.app, dim, message);
    }

    pub fn join(&mut self, uuid: Uuid, name: &str) -> (Entity, Vec<PacketPayload>) {
        self.join_in(self.dim, uuid, name)
    }

    pub fn join_in(&mut self, dim: Entity, uuid: Uuid, name: &str) -> (Entity, Vec<PacketPayload>) {
        let anchor = self.admit(dim, uuid, name);
        let packets = self.ticks(anchor, 3);
        (anchor, packets)
    }

    pub fn admit(&mut self, dim: Entity, uuid: Uuid, name: &str) -> Entity {
        let arrival = Arrival {
            uuid,
            name,
            position: DVec3::new(8.5, 64.0, 8.5),
            view_distance: 2,
        };
        admit(&mut self.app, dim, &arrival)
    }

    pub fn queue<P: Packet + Encode>(&mut self, anchor: Entity, packet: &P) {
        self.queue_arrived(anchor, packet, Instant::now());
    }

    pub fn queue_arrived<P: Packet + Encode>(
        &mut self,
        anchor: Entity,
        packet: &P,
        arrival: Instant,
    ) {
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
            trigger.raise(world.commands(), connection, data.clone());
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
                    acknowledged_tick: 0,
                    arrival,
                },
            );
        assert!(sent.is_ok(), "the dimension takes the packet");
    }

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

    pub fn session_of(&self, anchor: Entity) -> Option<PlayerSession> {
        session_of(&self.app, anchor)
    }

    pub fn game_mode(&mut self, anchor: Entity) -> GameMode {
        let player = self.player(anchor);
        self.world().get::<PlayerGameMode>(player).unwrap().0
    }
}

pub fn tick(app: &mut App) -> Vec<OutboundPlayerPacket> {
    app.update();
    mcrs_minecraft_engine_server::runner::between_ticks(app);
    muxer_link::deliver(app.world_mut(), Answers::All);
    app.world_mut()
        .resource_mut::<Messages<OutboundPlayerPacket>>()
        .drain()
        .collect()
}

pub fn submit_to(app: &mut App, dim: Entity, message: ToDim) {
    let sent = app
        .world_mut()
        .resource_mut::<DimInputs>()
        .send_control(dim, message);
    assert!(sent, "the dimension takes the message");
}

pub fn session_of(app: &App, anchor: Entity) -> Option<PlayerSession> {
    app.world().get::<Session>(anchor).map(|session| session.0)
}

pub struct Arrival<'a> {
    pub uuid: Uuid,
    pub name: &'a str,
    pub position: DVec3,
    pub view_distance: u8,
}

pub fn admit(app: &mut App, dim: Entity, arrival: &Arrival<'_>) -> Entity {
    admit_with_file(app, dim, arrival, &PersistedSnapshot::new())
}

pub fn admit_with_file(
    app: &mut App,
    dim: Entity,
    arrival: &Arrival<'_>,
    file: &PersistedSnapshot,
) -> Entity {
    let mut snapshot = file.clone();
    for (key, tag) in joining_snapshot(arrival).iter() {
        snapshot.insert(key, tag.clone());
    }
    admit_snapshot(app, dim, arrival, snapshot)
}

pub fn admit_unplaced(app: &mut App, dim: Entity, arrival: &Arrival<'_>) -> Entity {
    let mut snapshot = joining_snapshot(arrival);
    snapshot.remove("Pos");
    admit_snapshot(app, dim, arrival, snapshot)
}

fn joining_snapshot(arrival: &Arrival<'_>) -> PersistedSnapshot {
    PersistedSnapshot::joining_player(
        arrival.uuid,
        arrival.name.into(),
        arrival.position,
        Vec2::ZERO,
        Some(crate::support::client_info_with_view_distance(
            arrival.view_distance,
        )),
    )
}

fn admit_snapshot(
    app: &mut App,
    dim: Entity,
    arrival: &Arrival<'_>,
    snapshot: PersistedSnapshot,
) -> Entity {
    let session = app
        .world_mut()
        .resource_mut::<PlayerSessionCounter>()
        .next();
    let anchor = app
        .world_mut()
        .spawn((
            GameProfile {
                id: arrival.uuid,
                username: arrival.name.into(),
                properties: Vec::new(),
            },
            SessionBundle::placed(session, SessionPlacement::new(Place::Joining(dim), 0)),
        ))
        .id();
    app.world_mut().spawn(HostAnchorRef(anchor));
    submit_to(
        app,
        dim,
        ToDim::Spawn(InboundPlayerSpawn {
            session,
            snapshot,
            dimensions: Vec::new().into(),
        }),
    );
    anchor
}

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
