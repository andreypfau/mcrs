//! Replication of the entities that combat spawns outside the mob rows: projectiles and
//! experience orbs.

use std::collections::BTreeMap;

use bevy_ecs::prelude::*;
use glam::DVec3;
use mcrs_minecraft_core::Look;
use mcrs_minecraft_engine::bus::{OutboundPlayerPacket, PacketPayload, PacketTarget, to_target};
use mcrs_minecraft_engine::minecraft_entity::MinecraftEntity;
use mcrs_minecraft_engine_world::entity::EntityUuid;
use mcrs_minecraft_engine_world::entity_ids::GlobalEntityId;
use mcrs_minecraft_engine_world::world::dimension::{Dimension, InDimension};
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_protocol::VarInt;
use mcrs_minecraft_protocol_game::ByteAngle;
use mcrs_minecraft_protocol_game::LpVec3;
use mcrs_minecraft_protocol_game::clientbound::{
    ClientboundAddEntity, ClientboundEntityPositionSync, ClientboundRemoveEntities, PositionPath,
};

use crate::experience::ExperienceOrb;
use crate::explosions::PrimedTnt;
use crate::position::CombatPosition;
use crate::projectiles::{Projectile, ProjectileKind};

/// A combat entity that is sent to the clients of its dimension through the generic entity
/// packets.
#[derive(Component, Clone, Copy, Debug, PartialEq)]
pub struct WireEntity {
    pub kind: EntityType,
    pub network_id: Option<i32>,
    last: Option<DVec3>,
}

impl WireEntity {
    #[must_use]
    pub const fn new(kind: EntityType) -> Self {
        Self {
            kind,
            network_id: None,
            last: None,
        }
    }
}

/// The wire id and the dimension of every announced [`WireEntity`], so a despawn can remove it
/// from the clients that received it.
#[derive(Resource, Default)]
pub struct WireRegistry {
    entities: BTreeMap<Entity, (i32, Entity)>,
}

fn projectile_kind(kind: ProjectileKind) -> EntityType {
    match kind {
        ProjectileKind::Arrow => EntityType::Arrow,
        ProjectileKind::SpectralArrow => EntityType::SpectralArrow,
        ProjectileKind::Trident => EntityType::Trident,
        ProjectileKind::Snowball => EntityType::Snowball,
        ProjectileKind::Egg => EntityType::Egg,
        ProjectileKind::Fireball => EntityType::Fireball,
        ProjectileKind::SmallFireball => EntityType::SmallFireball,
        ProjectileKind::WitherSkull => EntityType::WitherSkull,
        ProjectileKind::EnderPearl => EntityType::EnderPearl,
        ProjectileKind::ExperienceBottle => EntityType::ExperienceBottle,
        ProjectileKind::Potion => EntityType::SplashPotion,
    }
}

/// Gives every projectile and experience orb the components that the replication needs. The
/// dimension sub-app has exactly one dimension, so an entity without `InDimension` joins it.
pub fn attach_wire_entities(
    mut commands: Commands,
    projectiles: Query<
        (
            Entity,
            &Projectile,
            Option<&InDimension>,
            Option<&EntityUuid>,
            Option<&MinecraftEntity>,
        ),
        (Without<WireEntity>,),
    >,
    orbs: Query<
        (
            Entity,
            Option<&InDimension>,
            Option<&EntityUuid>,
            Option<&MinecraftEntity>,
        ),
        (With<ExperienceOrb>, Without<WireEntity>),
    >,
    tnt: Query<
        (
            Entity,
            Option<&InDimension>,
            Option<&EntityUuid>,
            Option<&MinecraftEntity>,
        ),
        (With<PrimedTnt>, Without<WireEntity>),
    >,
    dimensions: Query<Entity, With<Dimension>>,
) {
    let Ok(dimension) = dimensions.single() else {
        return;
    };
    for (entity, projectile, seen, uuid, marker) in &projectiles {
        commands
            .entity(entity)
            .insert(WireEntity::new(projectile_kind(projectile.kind)));
        if seen.is_none() {
            commands.entity(entity).insert(InDimension(dimension));
        }
        if uuid.is_none() {
            commands.entity(entity).insert(EntityUuid::default());
        }
        if marker.is_none() {
            commands.entity(entity).insert(MinecraftEntity);
        }
    }
    for (entity, seen, uuid, marker) in &orbs {
        commands
            .entity(entity)
            .insert(WireEntity::new(EntityType::ExperienceOrb));
        if seen.is_none() {
            commands.entity(entity).insert(InDimension(dimension));
        }
        if uuid.is_none() {
            commands.entity(entity).insert(EntityUuid::default());
        }
        if marker.is_none() {
            commands.entity(entity).insert(MinecraftEntity);
        }
    }
    for (entity, seen, uuid, marker) in &tnt {
        commands
            .entity(entity)
            .insert(WireEntity::new(EntityType::Tnt));
        if seen.is_none() {
            commands.entity(entity).insert(InDimension(dimension));
        }
        if uuid.is_none() {
            commands.entity(entity).insert(EntityUuid::default());
        }
        if marker.is_none() {
            commands.entity(entity).insert(MinecraftEntity);
        }
    }
}

/// Announces every wire entity that has a global id, then sends the position changes.
pub fn sync_wire_entities(
    mut entities: Query<(
        Entity,
        &mut WireEntity,
        &InDimension,
        &CombatPosition,
        Option<&GlobalEntityId>,
        Option<&EntityUuid>,
    )>,
    mut registry: ResMut<WireRegistry>,
    mut packets: Option<MessageWriter<OutboundPlayerPacket>>,
) {
    let Some(packets) = packets.as_mut() else {
        return;
    };
    for (entity, mut wire, dimension, position, global, uuid) in &mut entities {
        if wire.network_id.is_none() {
            let (Some(global), Some(uuid)) = (global, uuid) else {
                continue;
            };
            let Ok(network_id) = i32::try_from(global.0.0) else {
                continue;
            };
            packets.write(to_target(
                PacketTarget::AllInDim(dimension.0),
                PacketPayload::AddEntity(ClientboundAddEntity {
                    id: VarInt(network_id),
                    uuid: uuid.0,
                    kind: wire.kind,
                    pos: position.0,
                    movement: LpVec3(DVec3::ZERO),
                    pitch: ByteAngle(0),
                    yaw: ByteAngle(0),
                    head_yaw: ByteAngle(0),
                    data: VarInt(0),
                }),
            ));
            wire.network_id = Some(network_id);
            wire.last = Some(position.0);
            registry.entities.insert(entity, (network_id, dimension.0));
            continue;
        }
        let Some(network_id) = wire.network_id else {
            continue;
        };
        if wire
            .last
            .is_some_and(|last| last.distance_squared(position.0) < 1.0e-12)
        {
            continue;
        }
        packets.write(to_target(
            PacketTarget::AllInDim(dimension.0),
            PacketPayload::EntityPosSync(ClientboundEntityPositionSync {
                entity_id: VarInt(network_id),
                position: PositionPath::Linear(position.0),
                look: Look::default(),
                on_ground: false,
            }),
        ));
        wire.last = Some(position.0);
    }
}

/// Removes the wire entities that left the world from the clients that received them.
pub fn forget_wire_entities(
    mut removed: RemovedComponents<WireEntity>,
    mut registry: ResMut<WireRegistry>,
    mut packets: Option<MessageWriter<OutboundPlayerPacket>>,
) {
    let Some(packets) = packets.as_mut() else {
        return;
    };
    for entity in removed.read() {
        let Some((network_id, dimension)) = registry.entities.remove(&entity) else {
            continue;
        };
        packets.write(to_target(
            PacketTarget::AllInDim(dimension),
            PacketPayload::RemoveEntities(ClientboundRemoveEntities {
                entity_ids: vec![VarInt(network_id)],
            }),
        ));
    }
}
