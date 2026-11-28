use bevy::asset::{AssetServer, Handle};
use bevy::shader::Shader;

use mcrs_minecraft_mesh::QuadFormat;

pub(super) struct Shaders {
    pub greedy: Handle<Shader>,
    pub model: Handle<Shader>,
}

impl Shaders {
    pub fn load(asset_server: &AssetServer) -> Self {
        Self {
            greedy: load(asset_server, "core/greedy.wesl"),
            model: load(asset_server, "core/model.wesl"),
        }
    }

    pub fn format(&self, format: QuadFormat) -> &Handle<Shader> {
        match format {
            QuadFormat::Greedy => &self.greedy,
            QuadFormat::Model => &self.model,
        }
    }
}

fn load(asset_server: &AssetServer, name: &str) -> Handle<Shader> {
    asset_server.load(format!("embedded://mcrs_minecraft_render/shaders/{name}"))
}
