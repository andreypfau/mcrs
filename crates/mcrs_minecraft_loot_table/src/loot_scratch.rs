use crate::table::Candidate;

/// The buffers of an evaluation, owned by the caller, so that a roll reuses them.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct LootScratch {
    pub(crate) candidates: Vec<Candidate>,
}

impl LootScratch {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
