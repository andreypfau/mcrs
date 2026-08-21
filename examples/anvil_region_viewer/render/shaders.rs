use bevy::prelude::*;
use bevy::shader::Shader;

use super::layer::Shape;

// Held alive because a shader whose imported module was dropped fails to compile.
const IMPORTED: [&str; 9] = [
    "include/fields.wgsl",
    "include/region.wgsl",
    "include/frame.wgsl",
    "include/quad.wgsl",
    "include/lighting.wgsl",
    "include/terrain_bindings.wgsl",
    "include/surface.wgsl",
    "include/finish.wgsl",
    "include/sky_bindings.wgsl",
];

pub(super) struct Shaders {
    pub greedy: Handle<Shader>,
    pub model: Handle<Shader>,
    pub cull: Handle<Shader>,
    pub sky: Handle<Shader>,
    pub clouds: Handle<Shader>,
    #[expect(dead_code, reason = "held so the modules these import stay loaded")]
    imports: Vec<Handle<Shader>>,
}

impl Shaders {
    pub fn load(asset_server: &AssetServer) -> Self {
        Self {
            greedy: load(asset_server, "core/greedy.wgsl"),
            model: load(asset_server, "core/model.wgsl"),
            cull: load(asset_server, "core/cull.wgsl"),
            sky: load(asset_server, "core/sky.wgsl"),
            clouds: load(asset_server, "core/clouds.wgsl"),
            imports: IMPORTED
                .iter()
                .map(|name| load(asset_server, name))
                .collect(),
        }
    }

    pub fn shape(&self, shape: Shape) -> &Handle<Shader> {
        match shape {
            Shape::Greedy => &self.greedy,
            Shape::Model => &self.model,
        }
    }
}

fn load(asset_server: &AssetServer, name: &str) -> Handle<Shader> {
    asset_server.load(format!(
        "embedded://anvil_region_viewer/render/shaders/{name}"
    ))
}
