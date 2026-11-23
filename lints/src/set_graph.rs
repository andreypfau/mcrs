use std::collections::HashMap;
use std::collections::HashSet;

use crate::config_node::Placement;
use crate::facts::SetFact;

#[derive(Debug, Default)]
pub struct SetGraph {
    gated: HashSet<String>,
    on_event: HashSet<String>,
    parents: HashMap<String, Vec<String>>,
}

impl SetGraph {
    pub fn add(&mut self, fact: &SetFact) {
        if fact.run_if {
            self.gated.insert(fact.set.clone());
        }
        if fact.on_event {
            self.on_event.insert(fact.set.clone());
        }
        self.parents
            .entry(fact.set.clone())
            .or_default()
            .extend(fact.parents.iter().cloned());
    }

    #[must_use]
    pub fn gates(&self, placement: &Placement) -> bool {
        placement.run_if
            || placement
                .sets
                .iter()
                .any(|set| self.reaches(set, &self.gated))
    }

    #[must_use]
    pub fn runs_on_event(&self, placement: &Placement) -> bool {
        placement.on_event
            || placement
                .sets
                .iter()
                .any(|set| self.reaches(set, &self.on_event))
    }

    fn reaches(&self, set: &str, marked: &HashSet<String>) -> bool {
        let mut seen = HashSet::new();
        let mut stack = vec![set];
        while let Some(current) = stack.pop() {
            if !seen.insert(current) {
                continue;
            }
            if marked.contains(current) {
                return true;
            }
            if let Some(parents) = self.parents.get(current) {
                stack.extend(parents.iter().map(String::as_str));
            }
        }
        false
    }
}
