use bevy_ecs::resource::Resource;
use mcrs_command::Limits;

/// The limits of the run of one command line, which every dimension world reads
/// when it runs the line.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct CommandLimits(pub Limits);
