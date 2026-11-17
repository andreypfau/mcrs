use crate::command_failure::CommandFailure;
use crate::source_state::SourceState;

#[derive(Clone, Debug, PartialEq)]
pub enum Modified {
    None,
    One(SourceState),
    Forked,
}

pub type ModifyResult = Result<Modified, CommandFailure>;
