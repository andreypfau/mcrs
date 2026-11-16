use mcrs_minecraft_probe::{Action, Scenario};
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

#[test]
fn give_scenario_observes_sound_and_command_trees() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("scenarios/give.json");
    let scenario: Scenario = serde_json::from_str(&fs::read_to_string(path).unwrap()).unwrap();
    assert!(scenario.observe.sound);
    assert_eq!(scenario.observe.commands, ["give"]);
    assert!(
        scenario
            .steps
            .iter()
            .any(|step| matches!(&step.action, Action::Console { lines } if !lines.is_empty()))
    );
}
