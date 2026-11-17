use bevy_ecs::entity::Entity;
use bevy_ecs::resource::Resource;

#[derive(Resource, Default)]
pub(crate) struct Diggers(pub(crate) Vec<Entity>);
