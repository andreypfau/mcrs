use bevy_ecs::resource::Resource;
use mcrs_command::Limits;

/// The limits of the run of one command line, which the host reads
/// when it runs the line.
// chisle: the game rules `max_command_forks` and `max_command_sequence_length` have no state in
// the server yet; a game rule resource lifts it.
#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct CommandLimits(pub Limits);
