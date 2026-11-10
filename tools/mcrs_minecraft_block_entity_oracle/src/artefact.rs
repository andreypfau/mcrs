use std::collections::BTreeMap;

use serde::Deserialize;

#[derive(Deserialize)]
pub struct ModelArtefact {
    pub layers: Vec<LayerRecord>,
}

#[derive(Deserialize)]
pub struct LayerRecord {
    pub model: String,
    pub layer: String,
    pub texture_width: f64,
    pub texture_height: f64,
    pub root: Part,
}

#[derive(Deserialize)]
pub struct Part {
    pub name: String,
    pub pivot: [f64; 3],
    pub rotation: [f64; 3],
    pub scale: [f64; 3],
    pub cubes: Vec<Cube>,
    pub children: Vec<Part>,
}

#[derive(Deserialize)]
pub struct Cube {
    pub corner: [f64; 3],
    pub size: [f64; 3],
    pub inflate: [f64; 3],
    pub uv: [f64; 2],
    pub uv_scale: [f64; 2],
    pub mirror: bool,
    pub visible_faces: Vec<String>,
    pub faces: Vec<Face>,
}

#[derive(Deserialize)]
pub struct Face {
    pub internal_direction: String,
    pub normal: [f64; 3],
    /// `[x, y, z, u, v]`: a point in the frame of the part and its texel. The artefact writes
    /// `NaN` for a value that the reference cannot compute; it reads as `None`.
    pub vertices: Vec<[Option<f64>; 5]>,
}

#[derive(Deserialize)]
pub struct PoseArtefact {
    pub cases: Vec<PoseCase>,
}

#[derive(Deserialize)]
#[serde(tag = "kind", rename_all = "lowercase")]
pub enum PoseCase {
    Pose {
        case: String,
        model: String,
        layer: String,
        parts: BTreeMap<String, PartTransform>,
    },
    Placement {
        shape: String,
        state: String,
        matrix: [[f64; 4]; 4],
    },
}

#[derive(Clone, Copy, Deserialize)]
pub struct PartTransform {
    pub position: [f64; 3],
    pub rotation: [f64; 3],
    pub scale: [f64; 3],
}

pub struct Artefacts {
    pub models: ModelArtefact,
    pub poses: PoseArtefact,
}

impl Artefacts {
    pub fn parse(models: &str, poses: &str) -> Self {
        Self {
            models: serde_json::from_str(&models.replace("NaN", "null")).unwrap(),
            poses: serde_json::from_str(poses).unwrap(),
        }
    }

    pub fn layer_entry(&self, model: &str, layer: &str) -> &LayerRecord {
        self.models
            .layers
            .iter()
            .find(|entry| entry.model == model && entry.layer == layer)
            .unwrap_or_else(|| panic!("the model artefact has no {model} / {layer}"))
    }

    pub fn pose(
        &self,
        case_name: &str,
        model: &str,
        layer: &str,
    ) -> &BTreeMap<String, PartTransform> {
        self.poses
            .cases
            .iter()
            .find_map(|case| match case {
                PoseCase::Pose {
                    case,
                    model: of,
                    layer: on,
                    parts,
                } if case == case_name && of == model && on == layer => Some(parts),
                _ => None,
            })
            .unwrap_or_else(|| panic!("the pose artefact has no {case_name} for {model} / {layer}"))
    }

    pub fn placement(&self, shape_name: &str, state_name: &str) -> [[f64; 4]; 4] {
        self.poses
            .cases
            .iter()
            .find_map(|case| match case {
                PoseCase::Placement {
                    shape,
                    state,
                    matrix,
                } if shape == shape_name && state == state_name => Some(*matrix),
                _ => None,
            })
            .unwrap_or_else(|| {
                panic!("the pose artefact has no placement {shape_name} {state_name}")
            })
    }
}
