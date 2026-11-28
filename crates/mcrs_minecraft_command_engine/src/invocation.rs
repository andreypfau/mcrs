use crate::source_state::SourceState;
use mcrs_minecraft_engine_world::session::PlayerSession;

#[derive(Clone, Debug)]
pub struct Invocation {
    pub output: PlayerSession,
    pub source: SourceState,
    pub(crate) stage: usize,
}

impl Invocation {
    #[must_use]
    pub fn stage(&self) -> usize {
        self.stage
    }
}
