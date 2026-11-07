//! The render mode of a scene capture (`docs/spec/client-frame-capture.md`, CFC-CMP-002): the
//! client takes the entities of a frame from the record of the vanilla client, not from a
//! server, so a comparison tests only the rendering.

use std::path::PathBuf;

use bevy::prelude::*;
use mcrs_minecraft_client_entity::{
    Age, ClientEntity, Equipment, Living, Motion, Placement, SyncedData, VariantId,
};
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::variant::{FrogVariant, PigVariant};
use mcrs_minecraft_entity_animation::pose_name;
use mcrs_minecraft_entity_render::{RecordedClip, RecordedClips};
use mcrs_minecraft_game_registries::variant::{FrogVariantFile, PigVariantFile};
use mcrs_minecraft_item::ProtoStack;
use mcrs_minecraft_item::component::equippable::EquipmentSlot;
use mcrs_minecraft_item::definition::Items;
use mcrs_minecraft_protocol_game::entity::Pose;
use mcrs_minecraft_registries::{CurrentRegistries, Wire, WireId};
use mcrs_minecraft_resource::{Id, ResourceLocation};
use serde::Deserialize;

/// The poses that a record can name.
const POSES: [Pose; 18] = [
    Pose::Standing,
    Pose::FallFlying,
    Pose::Sleeping,
    Pose::Swimming,
    Pose::SpinAttack,
    Pose::Crouching,
    Pose::LongJumping,
    Pose::Dying,
    Pose::Croaking,
    Pose::UsingTongue,
    Pose::Sitting,
    Pose::Roaring,
    Pose::Sniffing,
    Pose::Emerging,
    Pose::Digging,
    Pose::Sliding,
    Pose::Shooting,
    Pose::Inhaling,
];

/// The record of the vanilla client that the frames take their entities from.
#[derive(Resource, Clone, Debug)]
pub struct SceneRecord {
    pub path: PathBuf,
}

/// An entity that the client placed from a record.
#[derive(Component)]
pub struct RecordedEntity;

#[derive(Deserialize)]
struct Record {
    scene: String,
    frames: Vec<RecordFrame>,
}

#[derive(Deserialize)]
struct RecordFrame {
    image: String,
    entities: Vec<RecordEntity>,
}

#[derive(Deserialize)]
struct RecordEntity {
    id: uuid::Uuid,
    #[serde(rename = "type")]
    kind: String,
    position: [f64; 3],
    age: u32,
    pose: String,
    #[serde(default)]
    head_pitch: f32,
    #[serde(default)]
    body_yaw: f32,
    #[serde(default)]
    head_yaw: f32,
    #[serde(default)]
    walk_position: f32,
    #[serde(default)]
    walk_speed: f32,
    #[serde(default)]
    hurt_time: u32,
    #[serde(default)]
    death_time: u32,
    #[serde(default)]
    baby: bool,
    #[serde(default)]
    saddled: bool,
    variant: Option<String>,
    croak: Option<ClipState>,
    tongue: Option<ClipState>,
}

#[derive(Deserialize)]
struct ClipState {
    started: bool,
    elapsed_ticks: f32,
}

/// Replaces the recorded entities with the entities of the record frame `<scene>.<frame>.png`.
/// The world must be frozen, so that the entities keep the state of the record.
pub fn place_recorded_entities(
    frame: &str,
    record: &SceneRecord,
    commands: &mut Commands,
    placed: &Query<Entity, With<RecordedEntity>>,
    registries: Option<&CurrentRegistries>,
    items: Option<&Items>,
) {
    for entity in placed {
        commands.entity(entity).despawn();
    }
    let record: Record = match std::fs::read_to_string(&record.path)
        .map_err(|error| error.to_string())
        .and_then(|text| serde_json::from_str(&text).map_err(|error| error.to_string()))
    {
        Ok(record) => record,
        Err(error) => {
            error!(path = %record.path.display(), %error, "cannot read the scene record");
            return;
        }
    };
    let image = format!("{}.{frame}.png", record.scene);
    let Some(frame) = record
        .frames
        .iter()
        .find(|recorded| recorded.image == image)
    else {
        warn!(image, "the scene record has no such frame");
        return;
    };
    for (index, row) in frame.entities.iter().enumerate() {
        let Some(kind) = EntityType::read(&row.kind) else {
            continue;
        };
        let position = bevy::math::DVec3::from_array(row.position);
        let placement = Placement::new(position, row.body_yaw, row.head_pitch);
        let mut living = Living::new(row.head_yaw);
        living.body_yaw = row.body_yaw;
        living.previous_body_yaw = row.body_yaw;
        living.walk.position = row.walk_position;
        living.walk.speed = row.walk_speed;
        living.walk.previous_speed = row.walk_speed;
        living.hurt_time = row.hurt_time;
        living.death_time = row.death_time;
        let mut data = SyncedData {
            baby: row.baby,
            pose: POSES
                .into_iter()
                .find(|pose| pose_name(*pose) == row.pose)
                .unwrap_or(Pose::Standing),
            variant: row
                .variant
                .as_deref()
                .and_then(|name| variant(kind, name, registries)),
            ..SyncedData::default()
        };
        if row.death_time > 0 {
            data.health = 0.0;
        }
        let mut equipment = Equipment::default();
        if row.saddled
            && let Some(saddle) = items.and_then(|items| items.id_of("minecraft:saddle"))
        {
            equipment.set(
                EquipmentSlot::Saddle,
                ProtoStack::<Wire> {
                    id: WireId::from(saddle),
                    count: 1,
                    ..ProtoStack::default()
                },
            );
        }
        let clips: Vec<RecordedClip> = [("croak", &row.croak), ("tongue", &row.tongue)]
            .into_iter()
            .filter_map(|(entry, state)| {
                let state = state.as_ref().filter(|state| state.started)?;
                Some(RecordedClip {
                    entry: entry.to_owned(),
                    elapsed_ticks: state.elapsed_ticks,
                })
            })
            .collect();
        commands.spawn((
            RecordedEntity,
            ClientEntity {
                network_id: -1 - index as i32,
                uuid: row.id,
                kind,
            },
            Motion::new(placement, position),
            Age(row.age),
            living,
            data,
            equipment,
            RecordedClips(clips),
        ));
    }
}

fn variant(
    kind: EntityType,
    name: &str,
    registries: Option<&CurrentRegistries>,
) -> Option<VariantId> {
    let registries = registries?;
    let location = ResourceLocation::read(name).ok()?;
    match kind {
        EntityType::Pig => {
            let registry = registries.registry::<PigVariantFile>()?;
            let id = registry.by_name(location)?;
            Some(VariantId::Pig(Id::<PigVariant>::checked(
                id.number(),
                registry.len(),
            )?))
        }
        EntityType::Frog => {
            let registry = registries.registry::<FrogVariantFile>()?;
            let id = registry.by_name(location)?;
            Some(VariantId::Frog(Id::<FrogVariant>::checked(
                id.number(),
                registry.len(),
            )?))
        }
        _ => None,
    }
}
