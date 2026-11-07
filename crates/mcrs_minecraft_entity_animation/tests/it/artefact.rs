use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

#[derive(Deserialize)]
pub struct ClipArtefact {
    pub clips: Vec<ClipRecord>,
}

#[derive(Deserialize)]
pub struct ClipRecord {
    pub id: String,
    pub length: f64,
    pub looping: bool,
    pub bones: BTreeMap<String, Vec<ChannelRecord>>,
}

#[derive(Deserialize)]
pub struct ChannelRecord {
    pub target: String,
    pub keyframes: Vec<KeyframeRecord>,
}

#[derive(Deserialize)]
pub struct KeyframeRecord {
    pub time: f64,
    pub pre: [f64; 3],
    pub post: [f64; 3],
    pub interpolation: String,
}

#[derive(Deserialize)]
pub struct PoseArtefact {
    pub cases: Vec<PoseCase>,
}

#[derive(Deserialize)]
pub struct PoseCase {
    pub model: String,
    pub layer: String,
    pub inputs: Inputs,
    pub parts: BTreeMap<String, PartPose>,
}

#[derive(Deserialize)]
pub struct Inputs {
    pub walk_position: f64,
    pub walk_speed: f64,
    pub head_yaw_degrees: f64,
    pub head_pitch_degrees: f64,
    pub age_ticks: f64,
    #[serde(default)]
    pub swimming: bool,
    #[serde(default)]
    pub clip_start_ticks: BTreeMap<String, Option<f64>>,
}

#[derive(Deserialize)]
pub struct PartPose {
    pub position: [f64; 3],
    pub rotation: [f64; 3],
    pub scale: [f64; 3],
}

impl ClipArtefact {
    pub fn read() -> Self {
        read_json("entity_animations.json")
    }

    pub fn clip(&self, id: &str) -> &ClipRecord {
        self.clips
            .iter()
            .find(|clip| clip.id == id)
            .unwrap_or_else(|| panic!("the clip artefact has no {id}"))
    }
}

impl PoseArtefact {
    pub fn read() -> Self {
        read_json("entity_poses.json")
    }
}

fn read_json<T: serde::de::DeserializeOwned>(name: &str) -> T {
    let text = std::fs::read_to_string(fixtures().join(name)).unwrap();
    serde_json::from_str(&text).unwrap()
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../mcrs_minecraft_model/tests/fixtures/vanilla")
}
