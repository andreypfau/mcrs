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
        let schedule = fact.schedule.as_deref();
        let set = scoped(schedule, &fact.set);
        if fact.placement.run_if {
            self.gated.insert(set.clone());
        }
        if fact.placement.on_event {
            self.on_event.insert(set.clone());
        }
        self.parents.entry(set).or_default().extend(
            fact.placement
                .sets
                .iter()
                .map(|parent| scoped(schedule, parent)),
        );
    }

    #[must_use]
    pub fn gates(&self, placement: &Placement, schedule: Option<&str>) -> bool {
        placement.run_if || self.any_reaches(&placement.sets, schedule, &self.gated)
    }

    #[must_use]
    pub fn runs_on_event(&self, placement: &Placement, schedule: Option<&str>) -> bool {
        placement.on_event || self.any_reaches(&placement.sets, schedule, &self.on_event)
    }

    fn any_reaches(
        &self,
        sets: &[String],
        schedule: Option<&str>,
        marked: &HashSet<String>,
    ) -> bool {
        sets.iter()
            .any(|set| self.reaches(&scoped(schedule, set), marked))
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

fn scoped(schedule: Option<&str>, set: &str) -> String {
    format!("{}/{set}", schedule.unwrap_or_default())
}
