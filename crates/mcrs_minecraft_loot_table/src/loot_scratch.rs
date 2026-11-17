use mcrs_minecraft_predicate::PredicateScratch;

use crate::table::Candidate;

#[derive(Debug, Clone, Default)]
pub struct LootScratch {
    pub(crate) candidates: Vec<Candidate>,
    pub(crate) predicates: PredicateScratch,
}

impl LootScratch {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }
}
