use std::path::{Path, PathBuf};

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

impl ModelArtefact {
    pub fn read() -> Self {
        let path = fixtures().join("entity_models.json");
        let text = std::fs::read_to_string(path)
            .unwrap()
            .replace("NaN", "null");
        serde_json::from_str(&text).unwrap()
    }

    pub fn layer_entry(&self, model: &str, layer: &str) -> &LayerRecord {
        self.layers
            .iter()
            .find(|entry| entry.model == model && entry.layer == layer)
            .unwrap_or_else(|| panic!("the model artefact has no {model} / {layer}"))
    }
}

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../mcrs_minecraft_model/tests/fixtures/vanilla")
}
