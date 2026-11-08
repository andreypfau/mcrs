use crate::source_state::SourceState;
use mcrs_minecraft_engine_world::session::PlayerSession;

/// One run of a stage for one source: the session of the player who receives the output,
/// the source, and the stage whose nodes the handler reads.
#[derive(Clone, Debug)]
pub struct Invocation {
    /// The session of the player who sent the line and receives its messages.
    pub output: PlayerSession,
    /// The source this run of the stage is for.
    pub source: SourceState,
    pub(crate) stage: usize,
}

impl Invocation {
    /// The stage of the chain that this run is for.
    #[must_use]
    pub fn stage(&self) -> usize {
        self.stage
    }
}
