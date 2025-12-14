use crate::world::entity::explosive::ExplosiveBundle;
use crate::world::entity::{EntityUuid, MinecraftEntity};
use crate::world::explosion::{Explosion, ExplosionRadius};
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::bundle::Bundle;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::message::Message;
use bevy_ecs::prelude::{Commands, ContainsEntity, MessageReader, ParallelCommands, Query};
use bevy_ecs::query::{Added, QueryData, With, Without};
use bevy_math::{DVec3, Vec3};
use bevy_reflect::Reflect;
use derive_more::{Deref, DerefMut};
use mcrs_engine::entity::EntityObservers;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::player::Player;
use mcrs_engine::entity::player::chunk_view::{ChunkTrackingView, PlayerChunkObserver};
use mcrs_engine::entity::player::reposition::Reposition;
use mcrs_engine::world::chunk::{ChunkIndex, ChunkPos};
use mcrs_engine::world::dimension::{Dimension, DimensionPlayers, InDimension};
use mcrs_network::ServerSideConnection;
use mcrs_protocol::packets::game::clientbound::ClientboundAddEntity;
use mcrs_protocol::uuid::Uuid;
use mcrs_protocol::{ByteAngle, VarInt, Velocity, WritePacket};

pub struct PrimedTntPlugin;

impl Plugin for PrimedTntPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, update_new_primed_tnt);
        app.add_systems(FixedUpdate, update_fuse_durations);
    }
}

pub const DEFAULT_EXPLOSION_RADIUS: u16 = 4;
pub const DEFAULT_FUSE_DURATION: u16 = 80;

#[derive(Bundle)]
pub struct PrimedTntBundle {
    pub dimension: InDimension,
    pub transform: Transform,
    pub uuid: EntityUuid,
    pub explosive: ExplosiveBundle,
    pub fuse: Fuse,
    marker: PrimedTnt,
    mc_entity_marker: MinecraftEntity,
}

impl PrimedTntBundle {
    pub fn new(dimension: InDimension, transform: Transform) -> Self {
        Self {
            explosive: ExplosiveBundle {
                explosion_radius: ExplosionRadius(DEFAULT_EXPLOSION_RADIUS),
                ..Default::default()
            },
            fuse: Fuse::default(),
            mc_entity_marker: MinecraftEntity,
            marker: PrimedTnt,
            uuid: EntityUuid(Uuid::new_v4()),
            transform,
            dimension,
        }
    }
}

#[derive(Component, Debug, Default, Reflect)]
#[component(storage = "SparseSet")]
pub struct PrimedTnt;

/// The detonator entity
#[derive(Component, Debug, Reflect, Deref, DerefMut)]
pub struct Detonator(pub Entity);

impl ContainsEntity for Detonator {
    fn entity(&self) -> Entity {
        self.0
    }
}

#[derive(Component, Debug, Reflect, Deref, DerefMut)]
pub struct Fuse(pub u16);

impl Default for Fuse {
    fn default() -> Self {
        Self(DEFAULT_FUSE_DURATION)
    }
}

#[derive(QueryData)]
#[query_data(mutable)]
struct PlayerViewQuery {
    player: Entity,
    view: &'static PlayerChunkObserver,
    connection: &'static mut ServerSideConnection,
    reposition: &'static Reposition,
}

impl<'w, 's> PlayerViewQueryItem<'w, 's> {
    fn can_view_chunk(&self, chunk_pos: &ChunkPos) -> bool {
        self.view.can_view_chunk(chunk_pos)
    }

    fn send(&mut self, entity: &PrimedTntQueryItem) {
        let pkt = ClientboundAddEntity {
            id: VarInt(entity.entity.index() as i32),
            uuid: entity.uuid.0,
            kind: VarInt(132),
            pos: self.reposition.convert_dvec3(entity.transform.translation),
            velocity: VarInt(0),
            yaw: ByteAngle::from_degrees(entity.transform.rotation.y),
            pitch: ByteAngle::from_degrees(entity.transform.rotation.x),
            head_yaw: ByteAngle::from_degrees(entity.transform.rotation.y),
            data: VarInt(0),
        };
        self.connection.write_packet(&pkt);
        println!("try to spawn: {:?}", entity.entity);
    }
}

#[derive(QueryData)]
struct PrimedTntQuery {
    entity: Entity,
    transform: &'static Transform,
    dimension: &'static InDimension,
    uuid: &'static EntityUuid,
}

fn update_new_primed_tnt(
    entities: Query<(PrimedTntQuery), (With<PrimedTnt>, Without<EntityObservers>)>,
    dim_players: Query<(&DimensionPlayers), With<Dimension>>,
    mut players: Query<PlayerViewQuery>,
    mut commands: Commands,
) {
    entities.iter().for_each(|(tnt_entity)| {
        let Some(dim_players) = dim_players.get(tnt_entity.dimension.entity()).ok() else {
            return;
        };
        let entity_chunk = ChunkPos::from(tnt_entity.transform.translation);
        let mut iter = players.iter_many_mut(dim_players.iter());
        let mut viewers = vec![];
        while let Some((mut player_view)) = iter.fetch_next() {
            if !player_view.view.can_view_chunk(&entity_chunk) {
                continue;
            }
            player_view.send(&tnt_entity);
            viewers.push(player_view.player);
        }
        commands
            .entity(tnt_entity.entity)
            .insert(EntityObservers::new(viewers));
    })
}

fn update_fuse_durations(
    mut query: Query<(Entity, &mut Fuse), (With<PrimedTnt>, Without<Explosion>)>,
    mut commands: Commands,
) {
    query.iter_mut().for_each(|(e, mut fuse)| {
        let f = **fuse;
        if f > 0 {
            **fuse -= 1;
        } else {
            let mut cmds = commands.entity(e);
            cmds.remove::<Fuse>();
            cmds.insert(Explosion);
            println!("ticks left, BOOM!")
        }
    })
}
