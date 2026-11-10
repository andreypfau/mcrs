use std::collections::BTreeMap;

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
    pub fn parse(text: &str) -> Self {
        serde_json::from_str(text).unwrap()
    }

    pub fn clip(&self, id: &str) -> &ClipRecord {
        self.clips
            .iter()
            .find(|clip| clip.id == id)
            .unwrap_or_else(|| panic!("the clip artefact has no {id}"))
    }
}

impl PoseArtefact {
    pub fn parse(text: &str) -> Self {
        serde_json::from_str(text).unwrap()
    }
}

#[test]
fn the_pose_artefact_has_each_case_of_the_pig_and_the_frog() {
    let artefact = PoseArtefact::parse(&mcrs_minecraft_fixtures::fixture("entity_poses").text());
    let frog_cases = artefact
        .cases
        .iter()
        .filter(|case| case.model == "minecraft:frog")
        .count();
    assert_eq!(
        (artefact.cases.len() - frog_cases, frog_cases),
        (432, 59),
        "the artefact has each case"
    );
}
