use crate::registrar::Handler;
use bevy_ecs::resource::Resource;
use mcrs_command::Chain;

/// The command line that runs in this world, and its chain. The host lends it to the world a
/// stage runs in.
#[derive(Resource, Default)]
pub(crate) struct RunningCommand {
    pub(crate) line: String,
    pub(crate) chain: Chain<Handler>,
}
