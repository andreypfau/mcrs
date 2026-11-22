use bevy_ecs::entity::Entity;
use bevy_ecs::lifecycle::Insert;
use bevy_ecs::observer::On;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::ResMut;
use mcrs_minecraft_damage::Vitals;

#[derive(Resource, Debug, Default)]
pub(crate) struct VitalsWork(pub(crate) Vec<Entity>);

pub(crate) fn has_vitals_work(work: Res<VitalsWork>) -> bool {
    !work.0.is_empty()
}

pub(crate) fn track_inserted_vitals(
    insert: On<Insert<Vitals>>,
    vitals: Query<&Vitals>,
    mut work: ResMut<VitalsWork>,
) {
    let entity = insert.event().entity;
    if vitals.get(entity).is_ok_and(Vitals::is_counting) && !work.0.contains(&entity) {
        work.0.push(entity);
    }
}
