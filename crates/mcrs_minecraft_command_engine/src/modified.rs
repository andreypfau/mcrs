use crate::command_failure::CommandFailure;
use crate::source_state::SourceState;

/// What a modifier gives for one source.
#[derive(Clone, Debug, PartialEq)]
pub enum Modified {
    None,
    One(SourceState),
    /// The sources the modifier gave with [`CommandOutput::fork`](crate::CommandOutput::fork),
    /// in their order.
    Forked,
}

pub type ModifyResult = Result<Modified, CommandFailure>;
