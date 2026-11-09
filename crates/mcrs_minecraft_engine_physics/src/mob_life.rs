use std::collections::VecDeque;

use bevy_ecs::message::MessageReader;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_engine::coordinator::birth_clock::BirthClock;
use mcrs_minecraft_engine::entity_ids::CellEntityIds;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::pig_sound_variant_keys;
use mcrs_minecraft_entity::registry_keys::PIG_SOUND_VARIANT;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_entity_key::KeyedStream;
use mcrs_minecraft_entity_rows::Placed;
use mcrs_minecraft_entity_rows::RowStore;
use mcrs_minecraft_entity_rows::VisibleState;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::Category;
use mcrs_minecraft_physics::Dimensions;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_protocol_game::entity::MetaDataValue;
use mcrs_minecraft_protocol_game::entity::MetadataEntry;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_replication::MobLooks;
use mcrs_minecraft_replication::MobSignal;
use mcrs_minecraft_replication::MobState;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_resource::ResourceLocation;

use crate::mob_spawn_request::MobSpawnRequest;
use crate::mob_spawn_request::SpawnSetup;
use crate::mobs::Mob;
use crate::mobs::MobUuids;
use crate::mobs::Mobs;

/// The chance of a left hand in the first spawn setup (ANIM-SPAWN-001).
const LEFT_HANDED: f64 = 0.05;

/// The spawn requests that wait for an entity id, in the order of the life cycle. The cell gets
/// [`ENTITY_ID_RANGE_LEN`](mcrs_minecraft_engine::entity_ids::ENTITY_ID_RANGE_LEN) ids in each
/// tick, so a longer run of requests spawns over several ticks instead of losing its mobs.
#[derive(Resource, Debug, Default)]
pub struct PendingSpawns(pub VecDeque<MobSpawnRequest>);

/// The mobs that came into the world and left it in this tick, for the tile events.
#[derive(Resource, Debug, Default)]
pub struct MobChanges {
    pub(crate) spawned: Vec<Spawned>,
    /// The rows that a command changed outside the pass.
    pub(crate) touched: Vec<Placed>,
    pub(crate) signals: Vec<Signaled>,
}

/// What a mob shows its viewers in the tick, besides its state.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Signaled {
    pub(crate) at: Placed,
    pub(crate) signal: MobSignal,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Spawned {
    pub(crate) id: EntityId,
    pub(crate) state: MobState,
}

/// The motion row of a mob of `kind`: a walker with the adult box of the type.
#[must_use]
pub fn mob_row(kind: &EntityTypeData) -> MotionRow {
    MotionRow::living(
        Category::Walker,
        Dimensions::scalable(kind.width, kind.height).with_eye_height(kind.eye_height),
    )
}

/// The state of a new mob of `kind` at `position`, with no velocity and no rotation
/// (TRK-LIFE-003) and the base attributes of the type.
#[must_use]
pub fn mob_body(
    kind: &EntityTypeData,
    row: &MotionRow,
    key: EntityKey,
    position: DVec3,
) -> EntityState {
    let mut living = Living::of(row);
    if let Some(base) = kind.base_attributes {
        living.attributes.movement_speed = base.movement_speed;
        living.health = base.max_health;
    }
    let body = Body {
        position,
        ..Body::default()
    };
    let mut state = EntityState::new(key, body, Some(living));
    state.phase = key.phase_offset();
    state
}

/// The life cycle of the mobs: each spawn request of the tick becomes a row, in the order of the
/// UUIDs (TRK-LIFE-002). A request with the UUID of a mob that exists, or of an earlier
/// request, spawns nothing.
pub(crate) fn spawn_mobs<M: Send + Sync + 'static>(
    mut requests: MessageReader<MobSpawnRequest>,
    clock: Res<BirthClock>,
    registries: Option<Res<CurrentRegistries>>,
    mut ids: ResMut<CellEntityIds>,
    mut rows: ResMut<RowStore<M>>,
    mut mobs: ResMut<Mobs>,
    mut changes: ResMut<MobChanges>,
    mut pending: ResMut<PendingSpawns>,
    mut uuids: ResMut<MobUuids>,
) {
    let start = pending.0.len();
    pending.0.extend(requests.read().cloned());
    if let Some(new) = pending.0.make_contiguous().get_mut(start..) {
        new.sort_by_key(|request| request.uuid);
    }
    let registries = registries.as_deref().map(|registries| &**registries);
    while let Some(front) = pending.0.front() {
        if uuids.0.contains_key(&front.uuid) {
            tracing::warn!(
                uuid = front.uuid,
                "a mob with this UUID exists; the spawn is refused"
            );
            pending.0.pop_front();
            continue;
        }
        let Some(id) = ids.allocate() else {
            break;
        };
        let Some(request) = pending.0.pop_front() else {
            break;
        };
        let row = mob_row(request.kind);
        let key = EntityKey(request.uuid);
        let state = mob_body(request.kind, &row, key, request.position);
        let mut looks = match request.setup {
            SpawnSetup::First => {
                first_looks(request.kind, &KeyedStream::new(&key, clock.0), registries)
            }
            SpawnSetup::None => MobLooks::default(),
        };
        looks.no_ai = request.extras.no_ai;
        changes.spawned.push(Spawned {
            id,
            state: mob_state(&VisibleState::from(&state)),
        });
        rows.spawn(id, state, row);
        uuids.0.insert(request.uuid, id);
        mobs.0.insert(
            id,
            Mob {
                kind: request.kind,
                uuid: request.uuid,
                looks,
                custom_name: request.extras.custom_name,
                tags: request.extras.tags,
            },
        );
    }
}

/// The state of a living row that its viewers see, as the replication sends it.
pub(crate) fn mob_state(state: &VisibleState) -> MobState {
    MobState {
        position: state.position,
        velocity: state.velocity,
        yaw: state.yaw,
        pitch: state.pitch,
        head_yaw: state.head_yaw,
        on_ground: state.on_ground,
        health: state.health,
        dying: state.dying,
    }
}

/// The values of the first spawn setup of an animal that its viewers see (ANIM-SPAWN-001): a
/// sound variant of equal chances and a left hand with the chance 0.05, from the keyed stream of
/// the new mob. The variant keeps its default.
fn first_looks(
    kind: &EntityTypeData,
    stream: &KeyedStream,
    registries: Option<&Registries>,
) -> MobLooks {
    if !kind.is_ageable() {
        return MobLooks::default();
    }
    let mut type_entries = Vec::new();
    if kind.entity_type == EntityType::Pig {
        let sound = pick(
            registries,
            PIG_SOUND_VARIANT.location(),
            pig_sound_variant_keys::CLASSIC.location(),
            stream.unit(0),
        );
        if let (Some(index), Some(sound)) = (kind.variant_indices.sound_variant, sound) {
            type_entries.push(MetadataEntry {
                index,
                value: MetaDataValue::PigSoundVariant(sound),
            });
        }
    }
    MobLooks {
        no_ai: false,
        left_handed: stream.unit(1) < LEFT_HANDED,
        type_entries,
    }
}

/// An entry of `registry` of equal chances for the uniform value `unit`, or `None` when it is the
/// entry `default`.
fn pick<R>(
    registries: Option<&Registries>,
    registry: ResourceLocation<'static>,
    default: ResourceLocation<'static>,
    unit: f64,
) -> Option<Id<R>> {
    let names = registries?.names_of(registry)?;
    let index = ((unit * names.len() as f64) as usize).min(names.len().saturating_sub(1));
    let name = names.get(index)?;
    if *name == default {
        return None;
    }
    Id::checked(u16::try_from(index).ok()?, names.len())
}
