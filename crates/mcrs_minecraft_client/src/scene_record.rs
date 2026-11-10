//! The render mode of a scene capture (`docs/spec/client-frame-capture.md`, CFC-CMP-002): the
//! client takes the entities of a frame from the record of the vanilla client, not from a
//! server, so a comparison tests only the rendering.

use std::collections::HashMap;
use std::path::Path;

use bevy::prelude::*;
use mcrs_minecraft_client_entity::{
    Age, ClientEntity, Equipment, Living, Motion, Placement, SyncedData, VariantId,
};
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::variant::{FrogVariant, PigVariant};
use mcrs_minecraft_entity_render::{RecordedClip, RecordedClips, pose_from_name};
use mcrs_minecraft_game_registries::variant::{FrogVariantFile, PigVariantFile};
use mcrs_minecraft_item::ProtoStack;
use mcrs_minecraft_item::component::equippable::EquipmentSlot;
use mcrs_minecraft_item::definition::Items;
use mcrs_minecraft_protocol_game::entity::Pose;
use mcrs_minecraft_registries::{CurrentRegistries, Wire, WireId};
use mcrs_minecraft_resource::{Id, ResourceLocation};
use serde::Deserialize;

/// The entities of every frame of the record of the vanilla client, by frame image name.
#[derive(Resource, Debug)]
pub struct SceneRecord {
    scene: String,
    frames: HashMap<String, Vec<RecordEntity>>,
}

impl SceneRecord {
    pub fn read(path: &Path) -> Result<Self, String> {
        let text = std::fs::read_to_string(path).map_err(|error| error.to_string())?;
        Self::parse(&text)
    }

    fn parse(text: &str) -> Result<Self, String> {
        let record: Record = serde_json::from_str(text).map_err(|error| error.to_string())?;
        let frames = record
            .frames
            .into_iter()
            .map(|frame| (frame.image, frame.entities))
            .collect();
        Ok(Self {
            scene: record.scene,
            frames,
        })
    }

    fn entities(&self, frame: &str) -> Option<&[RecordEntity]> {
        self.frames
            .get(&format!("{}.{frame}.png", self.scene))
            .map(Vec::as_slice)
    }
}

/// Asks to replace the recorded entities with those of the named frame.
#[derive(Message)]
pub struct PlaceFrame(pub String);

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

#[derive(Debug, Deserialize)]
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

#[derive(Debug, Deserialize)]
struct ClipState {
    started: bool,
    elapsed_ticks: f32,
}

/// Replaces the recorded entities with the entities of the record frame `<scene>.<frame>.png`.
/// The world must be frozen, so that the entities keep the state of the record.
pub fn place_recorded_entities(
    mut requests: MessageReader<PlaceFrame>,
    record: Res<SceneRecord>,
    mut commands: Commands,
    placed: Query<Entity, With<RecordedEntity>>,
    registries: Option<Res<CurrentRegistries>>,
    items: Option<Res<Items>>,
) {
    let Some(PlaceFrame(frame)) = requests.read().last() else {
        return;
    };
    for entity in &placed {
        commands.entity(entity).despawn();
    }
    let Some(entities) = record.entities(frame) else {
        warn!(frame, "the scene record has no such frame");
        return;
    };
    let registries = registries.as_deref();
    let items = items.as_deref();
    for (index, row) in entities.iter().enumerate() {
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
            pose: pose_from_name(&row.pose).unwrap_or_else(|| {
                warn!(pose = row.pose, "the scene record names an unknown pose");
                Pose::Standing
            }),
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

#[cfg(test)]
mod tests {
    use super::*;

    const RECORD: &str = r#"{
        "scene": "pig_walk",
        "frames": [{
            "image": "pig_walk.walk.png",
            "entities": [{
                "id": "00000000-0000-0000-0000-000000000001",
                "type": "minecraft:pig",
                "position": [1.5, 64.0, -2.5],
                "age": 7,
                "pose": "sleeping",
                "head_pitch": 12.0,
                "body_yaw": 90.0,
                "head_yaw": 95.0,
                "walk_position": 3.0,
                "walk_speed": 0.5,
                "hurt_time": 4,
                "death_time": 2,
                "baby": true
            }]
        }]
    }"#;

    fn place(frame: &str) -> App {
        let mut app = App::new();
        app.add_message::<PlaceFrame>()
            .insert_resource(SceneRecord::parse(RECORD).unwrap())
            .add_systems(Update, place_recorded_entities);
        app.world_mut().write_message(PlaceFrame(frame.to_owned()));
        app.update();
        app
    }

    #[test]
    fn a_record_row_becomes_a_client_entity_with_its_recorded_state() {
        let mut app = place("walk");
        let mut rows = app
            .world_mut()
            .query::<(&ClientEntity, &Age, &Living, &SyncedData)>();
        let (entity, age, living, data) = rows.single(app.world()).unwrap();
        assert_eq!(entity.kind, EntityType::Pig);
        assert_eq!(age.0, 7);
        assert_eq!(living.body_yaw, 90.0);
        assert_eq!(living.walk.position, 3.0);
        assert_eq!(living.walk.speed, 0.5);
        assert_eq!((living.hurt_time, living.death_time), (4, 2));
        assert_eq!(data.pose, Pose::Sleeping);
        assert!(data.baby);
        assert_eq!(data.health, 0.0);
    }

    #[test]
    fn a_frame_the_record_lacks_places_nothing_and_clears_the_last_frame() {
        let mut app = place("walk");
        app.world_mut()
            .write_message(PlaceFrame("missing".to_owned()));
        app.update();
        let mut rows = app.world_mut().query::<&RecordedEntity>();
        assert_eq!(rows.iter(app.world()).count(), 0);
    }
}
