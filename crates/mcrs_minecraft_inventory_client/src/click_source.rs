use bevy_ecs::resource::Resource;
use mcrs_minecraft_engine_item::RequestSource;

#[derive(Resource, Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct ClickSource(pub(crate) RequestSource);
