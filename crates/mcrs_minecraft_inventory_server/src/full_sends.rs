use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;

#[derive(Resource, Debug, Default)]
pub(crate) struct FullSends(pub(crate) Vec<Entity>);

pub(crate) fn any_full_send(sends: Res<FullSends>) -> bool {
    !sends.0.is_empty()
}
