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
use mcrs_minecraft_replication::MobState;
use mcrs_minecraft_replication::MobVariant;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_resource::ResourceLocation;

use crate::entity_pass::mob_state;
use crate::entity_rows::EntityRows;
use crate::mob_spawn_request::MobSpawnRequest;
use crate::mob_spawn_request::SpawnSetup;
use crate::mobs::Mob;
use crate::mobs::Mobs;

/// The chance of a left hand in the first spawn setup (ANIM-SPAWN-001).
const LEFT_HANDED: f64 = 0.05;

/// The mobs that came into the world and left it in this tick, for the tile events.
#[derive(Resource, Debug, Default)]
pub struct MobChanges {
    pub(crate) spawned: Vec<Spawned>,
    pub(crate) despawned: Vec<Despawned>,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Spawned {
    pub(crate) id: EntityId,
    pub(crate) state: MobState,
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct Despawned {
    pub(crate) id: EntityId,
    pub(crate) tile: TileKey,
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
) {
    let mut batch: Vec<MobSpawnRequest> = requests.read().copied().collect();
    if batch.is_empty() {
        return;
    }
    batch.sort_by_key(|request| request.key);
    let registries = registries.as_deref().map(|registries| &**registries);
    for request in batch {
        let Some(id) = ids.allocate() else {
            tracing::warn!(kind = ?request.kind, "the cell has no entity id left; the mob is lost");
            continue;
        };
        let row = mob_row(request.kind);
        let state = mob_body(
            request.kind,
            &row,
            Birth::from(request.key),
            request.position,
        );
        let looks = match request.setup {
            SpawnSetup::First => first_looks(
                request.kind,
                &KeyedStream::new(&request.key, clock.0),
                registries,
            ),
            SpawnSetup::None => MobLooks::default(),
        };
        changes.spawned.push(Spawned {
            id,
            state: mob_state(&state),
        });
        rows.spawn(id, state, row);
        mobs.0.insert(
            id,
            Mob {
                kind: request.kind,
                uuid: request.uuid,
                looks,
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
