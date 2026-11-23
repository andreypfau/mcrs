use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use mcrs_engine_muxer::EntityId;
use mcrs_minecraft_damage::Vitals;
use mcrs_minecraft_physics::EntityState;

#[derive(Resource, Debug, Default)]
pub struct MobChanges {
    pub(crate) spawned: Vec<Spawned>,
}

impl MobChanges {
    #[must_use]
    pub fn spawned(&self) -> &[Spawned] {
        &self.spawned
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
#[non_exhaustive]
pub struct Spawned {
    pub entity: Entity,
    pub id: EntityId,
    pub start: EntityState,
    pub vitals: Vitals,
}

pub(crate) fn has_spawns(changes: Res<MobChanges>) -> bool {
    !changes.spawned.is_empty()
}

pub(crate) fn clear_mob_changes(mut changes: ResMut<MobChanges>) {
    changes.spawned.clear();
}
