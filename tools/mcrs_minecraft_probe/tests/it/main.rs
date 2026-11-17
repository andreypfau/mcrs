use mcrs_minecraft_probe::Scenario;
use std::collections::BTreeSet;
use std::fs;
use std::path::Path;

#[test]
fn every_scenario_loads_with_unique_step_ids() {
    let directory = Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios");
    for entry in fs::read_dir(&directory).unwrap() {
        let path = entry.unwrap().path();
        let scenario: Scenario = serde_json::from_str(&fs::read_to_string(&path).unwrap())
            .unwrap_or_else(|error| panic!("{path:?} is not a scenario: {error}"));
        let mut ids = BTreeSet::new();
        for step in &scenario.steps {
            assert!(
                ids.insert(&step.id),
                "{path:?} repeats the step id {}",
                step.id
            );
        }
    }
}
