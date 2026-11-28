use crate::registrar::Handler;
use bevy_ecs::resource::Resource;
use mcrs_command::Chain;

#[derive(Resource, Default)]
pub(crate) struct RunningCommand {
    pub(crate) line: String,
    pub(crate) chain: Chain<Handler>,
}
