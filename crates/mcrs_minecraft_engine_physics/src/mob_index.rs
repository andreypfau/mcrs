use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::Remove;
use bevy_ecs::observer::On;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Query;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_entity_key::EntityUuid;
use rustc_hash::FxHashMap;

use crate::mobs::Mob;

#[derive(Resource, Debug, Default)]
pub struct MobIndex(FxHashMap<EntityUuid, Entity>);

impl MobIndex {
    #[must_use]
    pub fn get(&self, uuid: EntityUuid) -> Option<Entity> {
        self.0.get(&uuid).copied()
    }

    pub(crate) fn insert(&mut self, uuid: EntityUuid, entity: Entity) {
        self.0.insert(uuid, entity);
    }

    pub(crate) fn remove(&mut self, uuid: EntityUuid, entity: Entity) {
        if self.get(uuid) == Some(entity) {
            self.0.remove(&uuid);
        }
    }
}

pub(crate) fn forget_mob(
    remove: On<Remove<Mob>>,
    mobs: Query<&Mob>,
    index: Option<ResMut<MobIndex>>,
) {
    let entity = remove.event().entity;
    if let (Some(mut index), Ok(mob)) = (index, mobs.get(entity)) {
        index.remove(EntityUuid::from_u128(mob.uuid), entity);
    }
}
