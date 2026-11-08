use std::collections::VecDeque;

use bevy_ecs::message::MessageReader;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use glam::DVec3;
use mcrs_engine_muxer::EntityId;
use mcrs_engine_muxer::TileKey;
use mcrs_minecraft_engine::coordinator::birth_clock::BirthClock;
use mcrs_minecraft_engine::entity_ids::CellEntityIds;
use mcrs_minecraft_entity::pig_sound_variant_keys;
use mcrs_minecraft_entity::registry_keys::PIG_SOUND_VARIANT;
use mcrs_minecraft_mob::MobType;
use mcrs_minecraft_physics::Birth;
use mcrs_minecraft_physics::Body;
use mcrs_minecraft_physics::Category;
use mcrs_minecraft_physics::Dimensions;
use mcrs_minecraft_physics::EntityState;
use mcrs_minecraft_physics::KeyedStream;
use mcrs_minecraft_physics::Living;
use mcrs_minecraft_physics::MotionRow;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_replication::MobLooks;
use mcrs_minecraft_replication::MobSignal;
use mcrs_minecraft_replication::MobState;
use mcrs_minecraft_replication::MobVariant;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_resource::ResourceLocation;

use crate::entity_pass::mob_state;
use crate::entity_rows::EntityRows;
use crate::mob_spawn_request::MobSpawnRequest;
use crate::mob_spawn_request::SpawnSetup;
use crate::mobs::Mob;
use crate::mobs::MobUuids;
use crate::mobs::Mobs;
use crate::tile_at::tile_at;

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

/// A row and its tile.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Placed {
    pub tile: TileKey,
    pub id: EntityId,
}

/// The motion row of a mob of `kind`: a walker with the adult box of the type.
#[must_use]
pub fn mob_row(kind: MobType) -> MotionRow {
    let [width, height] = kind.size();
    MotionRow::living(
        Category::Walker,
        Dimensions::scalable(width, height).with_eye_height(kind.eye_height()),
    )
}

/// The state of a new mob of `kind` at `position`, with no velocity and no rotation
/// (TRK-LIFE-003) and the base attributes of the type.
#[must_use]
pub fn mob_body(kind: MobType, row: &MotionRow, key: Birth, position: DVec3) -> EntityState {
    let mut living = Living::of(row);
    living.attributes.movement_speed = kind.movement_speed();
    living.health = kind.max_health();
    let body = Body {
        position,
        ..Body::default()
    };
    let mut state = EntityState::new(key.key, body, Some(living));
    state.phase = key.phase;
    state
}

/// The life cycle of the mobs: each spawn request of the tick becomes a row, in the order of the
/// birth keys (TRK-LIFE-002).
pub(crate) fn spawn_mobs(
    mut requests: MessageReader<MobSpawnRequest>,
    clock: Res<BirthClock>,
    registries: Option<Res<CurrentRegistries>>,
    mut ids: ResMut<CellEntityIds>,
    mut rows: ResMut<EntityRows>,
    mut mobs: ResMut<Mobs>,
    mut changes: ResMut<MobChanges>,
    mut pending: ResMut<PendingSpawns>,
    mut uuids: ResMut<MobUuids>,
) {
    let start = pending.0.len();
    pending.0.extend(requests.read().cloned());
    if let Some(new) = pending.0.make_contiguous().get_mut(start..) {
        new.sort_by_key(|request| request.key);
    }
    let registries = registries.as_deref().map(|registries| &**registries);
    while !pending.0.is_empty() {
        let Some(id) = ids.allocate() else {
            break;
        };
        let Some(request) = pending.0.pop_front() else {
            break;
        };
        let row = mob_row(request.kind);
        let state = mob_body(
            request.kind,
            &row,
            Birth::from(request.key),
            request.position,
        );
        let mut looks = match request.setup {
            SpawnSetup::First => first_looks(
                request.kind,
                &KeyedStream::new(&request.key, clock.0),
                registries,
            ),
            SpawnSetup::None => MobLooks::default(),
        };
        looks.no_ai = request.extras.no_ai;
        changes.spawned.push(Spawned {
            id,
            state: mob_state(&state),
        });
        rows.spawn(id, state, row);
        uuids.0.insert(request.uuid, id);
        mobs.0.insert(
            id,
            Mob {
                kind: request.kind,
                uuid: request.uuid,
                looks,
                tile: tile_at(request.position),
                custom_name: request.extras.custom_name,
                tags: request.extras.tags,
                last_hit: None,
            },
        );
    }
}

/// The values of the first spawn setup of an animal that its viewers see (ANIM-SPAWN-001): a
/// sound variant of equal chances and a left hand with the chance 0.05, from the keyed stream of
/// the new mob. The variant keeps its default.
fn first_looks(kind: MobType, stream: &KeyedStream, registries: Option<&Registries>) -> MobLooks {
    if !kind.is_animal() {
        return MobLooks::default();
    }
    let variant = match kind {
        MobType::Pig => MobVariant::Pig {
            variant: None,
            sound: pick(
                registries,
                PIG_SOUND_VARIANT.location(),
                pig_sound_variant_keys::CLASSIC.location(),
                stream.unit(0),
            ),
        },
        _ => MobVariant::Plain,
    };
    MobLooks {
        no_ai: false,
        left_handed: stream.unit(1) < LEFT_HANDED,
        variant,
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
