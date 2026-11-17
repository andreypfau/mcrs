use bevy_ecs::resource::Resource;
use mcrs_command::Limits;

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct CommandLimits(pub Limits);
