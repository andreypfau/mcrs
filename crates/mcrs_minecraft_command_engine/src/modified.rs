use crate::command_failure::CommandFailure;
use crate::source_state::SourceState;

/// What a modifier gives for one source.
#[derive(Clone, Debug, PartialEq)]
pub enum Modified {
    /// No source: the chain stops for this source.
    None,
    /// One source in place of the one the modifier read.
    One(SourceState),
    /// The sources the modifier gave with [`CommandOutput::fork`](crate::CommandOutput::fork),
    /// in their order.
    Forked,
}

/// The result of a modifier: the sources it gives, or the error the source receives.
pub type ModifyResult = Result<Modified, CommandFailure>;
