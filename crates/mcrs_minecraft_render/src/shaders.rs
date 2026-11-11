use bevy::prelude::*;
use bevy::shader::Shader;

use super::layer::Shape;

pub(super) struct Shaders {
    pub greedy: Handle<Shader>,
    pub model: Handle<Shader>,
    pub cull: Handle<Shader>,
}

impl Shaders {
    pub fn load(asset_server: &AssetServer) -> Self {
        Self {
            greedy: load(asset_server, "core/greedy.wesl"),
            model: load(asset_server, "core/model.wesl"),
            cull: load(asset_server, "core/cull.wesl"),
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
    asset_server.load(format!("embedded://mcrs_minecraft_render/shaders/{name}"))
}
