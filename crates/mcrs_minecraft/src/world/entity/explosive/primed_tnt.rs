use crate::world::bus::{OutboundPlayerPacket, PacketPayload, PacketPriority, PacketTarget};
use crate::world::entity::explosive::ExplosiveBundle;
use crate::world::entity::player::HostAnchor;
use crate::world::entity::{EntityUuid, MinecraftEntity, MinecraftEntityType};
use crate::world::explosion::{Explosion, ExplosionRadius};
use bevy_app::{App, FixedUpdate, Plugin};
use bevy_ecs::bundle::Bundle;
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::prelude::{Commands, ContainsEntity, MessageWriter, On, Query};
use bevy_ecs::query::{With, Without};
use bevy_ecs::query::QueryData;
use derive_more::{Deref, DerefMut};
use mcrs_engine::entity::EntityNetworkAddEvent;
use mcrs_engine::entity::physics::Transform;
use mcrs_engine::entity::player::Player;
use mcrs_engine::entity::player::reposition::Reposition;
use mcrs_engine::session::PlayerSession;
use mcrs_engine::world::dimension::InDimension;
use mcrs_protocol::uuid::Uuid;

pub struct PrimedTntPlugin;

impl Plugin for PrimedTntPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(FixedUpdate, update_fuse_durations);
        app.add_observer(network_add);
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

    pub fn with_fuse(mut self, fuse: u16) -> Self {
        self.fuse = Fuse(fuse);
        self
    }
}

#[derive(Component, Debug, Default)]
#[component(storage = "SparseSet")]
pub struct PrimedTnt;

/// The detonator entity
#[derive(Component, Debug, Deref, DerefMut)]
pub struct Detonator(pub Entity);

impl ContainsEntity for Detonator {
    fn entity(&self) -> Entity {
        self.0
    }
}

#[derive(Component, Debug, Deref, DerefMut)]
pub struct Fuse(pub u16);

impl Default for Fuse {
    fn default() -> Self {
        Self(DEFAULT_FUSE_DURATION)
    }
}

#[derive(QueryData)]
struct PrimedTntQuery {
    entity: Entity,
    transform: &'static Transform,
    dimension: &'static InDimension,
    uuid: &'static EntityUuid,
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
        }
    })
}

fn network_add(
    event: On<EntityNetworkAddEvent>,
    tnt: Query<(Entity, &EntityUuid, &Transform), With<PrimedTnt>>,
    player: Query<(&HostAnchor, &Reposition), With<Player>>,
    mut packet_writer: MessageWriter<OutboundPlayerPacket>,
) {
    let Ok((entity, uuid, transform)) = tnt.get(event.entity) else {
        return;
    };
    let Ok((anchor, reposition)) = player.get(event.player) else {
        return;
    };

    packet_writer.write(OutboundPlayerPacket {
        target: PacketTarget::SinglePlayer(anchor.0),
        priority: PacketPriority::Normal,
        data: PacketPayload::PlayerEnteredView {
            entity_id: entity.index_u32() as i32,
            uuid: uuid.0,
            kind: MinecraftEntityType::PrimedTnt as i32,
            position: reposition.convert_dvec3(transform.translation),
            yaw: transform.rotation.y,
            pitch: transform.rotation.x,
        },
        session: PlayerSession(0),
        epoch: 0,
    });
}
