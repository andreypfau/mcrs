use std::collections::VecDeque;

use bevy_ecs::message::MessageReader;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_damage::HitOutcome;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::entity_ids::CellEntityIds;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::pig_sound_variant_keys;
use mcrs_minecraft_entity::registry_keys::PIG_SOUND_VARIANT;
use mcrs_minecraft_entity_key::EntityKey;
use mcrs_minecraft_entity_key::KeyedStream;
use mcrs_minecraft_entity_rows::Placed;
use mcrs_minecraft_entity_rows::RowStore;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::Category;
use mcrs_minecraft_physics::Dimensions;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_protocol_game::entity::MetaDataValue;
use mcrs_minecraft_protocol_game::entity::MetadataEntry;
use mcrs_minecraft_protocol_game::entity::MobLooks;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_registries::Registries;
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
/// [`ENTITY_ID_RANGE_LEN`](mcrs_minecraft_engine_world::entity_ids::ENTITY_ID_RANGE_LEN) ids in each
/// tick, so a longer run of requests spawns over several ticks instead of losing its mobs.
#[derive(Resource, Debug, Default)]
pub struct PendingSpawns(pub VecDeque<MobSpawnRequest>);

/// What happened to the mobs in this tick outside the rows: their spawns, the rows that a
/// command changed and the hits. The publish of the tick reads them, and the end of the tick
/// clears them.
#[derive(Resource, Debug, Default)]
pub struct MobChanges {
    pub(crate) spawned: Vec<Spawned>,
    pub(crate) touched: Vec<Placed>,
    pub(crate) strikes: Vec<Struck>,
}

impl MobChanges {
    /// The mobs that spawned in this tick, in spawn order.
    #[must_use]
    pub fn spawned(&self) -> &[Spawned] {
        &self.spawned
    }

    /// The rows that a command changed outside the pass, in the order of the changes.
    #[must_use]
    pub fn touched(&self) -> &[Placed] {
        &self.touched
    }

    /// The hits that a mob took and did not reject, in the order of the hits.
    #[must_use]
    pub fn strikes(&self) -> &[Struck] {
        &self.strikes
    }
}

/// A mob that came into the world, with its state at the spawn, where its tracking starts.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spawned {
    pub id: EntityId,
    pub start: EntityState,
}

/// A hit that the mob row `at` did not reject.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Struck {
    pub at: Placed,
    /// The network id of the damage type of the hit.
    pub damage_type: u32,
    pub outcome: HitOutcome,
    /// Whether the mob is dying after the hit.
    pub died: bool,
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
        changes.spawned.push(Spawned { id, start: state });
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

/// The end of the tick of the mobs, after their publish: the cold table forgets the mobs whose
/// rows left the world, and the changes of the tick clear.
pub(crate) fn end_mob_changes<M: Send + Sync + 'static>(
    rows: Res<RowStore<M>>,
    mut mobs: ResMut<Mobs>,
    mut uuids: ResMut<MobUuids>,
    mut changes: ResMut<MobChanges>,
) {
    for removed in rows.changes().removed() {
        if let Some(mob) = mobs.0.remove(&removed.id) {
            uuids.0.remove(&mob.uuid);
        }
    }
    changes.spawned.clear();
    changes.touched.clear();
    changes.strikes.clear();
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
