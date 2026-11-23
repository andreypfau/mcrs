use std::collections::VecDeque;

use bevy_ecs::message::MessageReader;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_damage::HitRecord;
use mcrs_minecraft_engine_entity_motion::MobIntent;
use mcrs_minecraft_engine_entity_motion::MotionId;
use mcrs_minecraft_engine_entity_motion::MotionKind;
use mcrs_minecraft_engine_entity_motion::MotionState;
use mcrs_minecraft_engine_world::birth_clock::BirthClock;
use mcrs_minecraft_engine_world::entity_ids::CellEntityIds;
use mcrs_minecraft_entity_key::EntityUuid;
use mcrs_minecraft_entity_key::KeyedStream;
use mcrs_minecraft_protocol_game::entity::MobLooks;
use mcrs_minecraft_registries::CurrentRegistries;

use crate::first_looks::first_looks;
use crate::mob_body::mob_body;
use crate::mob_changes::MobChanges;
use crate::mob_changes::Spawned;
use crate::mob_index::MobIndex;
use crate::mob_row::mob_row;
use crate::mob_spawn_request::MobSpawnRequest;
use crate::mob_spawn_request::SpawnSetup;
use crate::mob_vitals::mob_vitals;
use crate::mobs::Mob;

#[derive(Resource, Debug, Default)]
pub(crate) struct WaitingSpawns(VecDeque<MobSpawnRequest>);

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::ungated_system,
        reason = "retries the waiting spawns on each tick until the cell has free entity ids, also when no request arrives"
    )
)]
#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "the metadata entries move into the `Mob` component of each spawned mob"
    )
)]
pub(crate) fn spawn_mob_entities(
    mut commands: Commands,
    mut requests: MessageReader<MobSpawnRequest>,
    clock: Res<BirthClock>,
    registries: Option<Res<CurrentRegistries>>,
    mut ids: ResMut<CellEntityIds>,
    mut index: ResMut<MobIndex>,
    mut changes: ResMut<MobChanges>,
    mut waiting: ResMut<WaitingSpawns>,
) {
    let waiting = &mut waiting.0;
    let start = waiting.len();
    waiting.extend(requests.read().cloned());
    if let Some(new) = waiting.make_contiguous().get_mut(start..) {
        new.sort_by_key(|request| request.uuid);
    }
    let registries = registries.as_deref().map(|registries| &**registries);
    while let Some(front) = waiting.front() {
        let key = EntityUuid::from_u128(front.uuid);
        if index.get(key).is_some() {
            tracing::warn!(
                uuid = front.uuid,
                "a mob with this UUID exists; the spawn is refused"
            );
            waiting.pop_front();
            continue;
        }
        let Some(id) = ids.allocate() else {
            break;
        };
        let Some(request) = waiting.pop_front() else {
            break;
        };
        let row = mob_row(request.kind);
        let state = mob_body(request.kind, &row, key, request.position);
        let vitals = mob_vitals(request.kind);
        let mut looks = match request.setup {
            SpawnSetup::First => {
                first_looks(request.kind, &KeyedStream::new(&key, clock.0), registries)
            }
            SpawnSetup::None => MobLooks::default(),
        };
        looks.no_ai = request.extras.no_ai;
        let mob = Mob {
            kind: request.kind,
            uuid: request.uuid,
            looks,
            custom_name: request.extras.custom_name,
            tags: request.extras.tags,
        };
        let entity = commands
            .spawn((
                MotionState(state),
                MotionKind(row),
                MotionId(id),
                MobIntent::default(),
                vitals,
                HitRecord::default(),
                mob,
            ))
            .id();
        index.insert(key, entity);
        changes.spawned.push(Spawned {
            entity,
            id,
            start: state,
            vitals,
        });
    }
}
