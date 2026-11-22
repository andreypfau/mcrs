use std::collections::HashMap;
use std::collections::HashSet;

use crate::config_node::Placement;
use crate::facts::SetFact;

#[derive(Debug, Default)]
pub struct SetGraph {
    gated: HashSet<String>,
    parents: HashMap<String, Vec<String>>,
}

impl SetGraph {
    pub fn add(&mut self, fact: &SetFact) {
        if fact.run_if {
            self.gated.insert(fact.set.clone());
        }
        self.parents
            .entry(fact.set.clone())
            .or_default()
            .extend(fact.parents.iter().cloned());
    }

    #[must_use]
    pub fn gates(&self, placement: &Placement) -> bool {
        placement.run_if || placement.sets.iter().any(|set| self.is_gated(set))
    }

    fn is_gated(&self, set: &str) -> bool {
        let mut seen = HashSet::new();
        let mut stack = vec![set];
        while let Some(current) = stack.pop() {
            if !seen.insert(current) {
                continue;
            }
            if self.gated.contains(current) {
                return true;
            }
            if let Some(parents) = self.parents.get(current) {
                stack.extend(parents.iter().map(String::as_str));
            }
        }
        false
    }
}
