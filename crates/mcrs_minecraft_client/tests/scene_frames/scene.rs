use std::collections::BTreeMap;
use std::path::Path;

use serde::Deserialize;

#[derive(Debug, Deserialize)]
pub struct Scene {
    pub name: String,
    pub resolution: [u32; 2],
    pub world: World,
    pub options: BTreeMap<String, String>,
    pub script: Vec<Action>,
}

#[derive(Debug, Deserialize)]
pub struct World {
    #[serde(rename = "type")]
    pub kind: String,
    pub seed: String,
    pub generate_structures: bool,
    pub game_rules: BTreeMap<String, bool>,
}

#[derive(Debug, Deserialize)]
#[serde(untagged)]
pub enum Action {
    Command { command: String },
    Commands { commands: Vec<String> },
    Wait { wait: u32 },
    Step { step: u32 },
    Pose { pose: Pose },
    Capture { capture: String, at: CameraPose },
}

#[derive(Debug, Deserialize)]
pub struct Pose {
    pub entity: String,
    pub pose: String,
}

#[derive(Debug, Clone, Copy, Deserialize)]
pub struct CameraPose {
    /// The eye position, in blocks.
    pub position: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
}

impl Scene {
    pub fn read(path: &Path) -> Self {
        let text = std::fs::read_to_string(path)
            .unwrap_or_else(|error| panic!("cannot read {}: {error}", path.display()));
        serde_json::from_str(&text)
            .unwrap_or_else(|error| panic!("{} is not a scene: {error}", path.display()))
    }
}
