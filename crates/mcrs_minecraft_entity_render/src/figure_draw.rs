use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::render_world::EntityRenderState;

#[derive(SystemParam)]
pub struct FigureDraw<'w> {
    state: Res<'w, EntityRenderState>,
}

impl FigureDraw<'_> {
    pub fn encode(&self, pass: &mut wgpu::RenderPass<'_>) {
        if !self.state.has_figure() {
            return;
        }
        if let Some(gpu) = self.state.gpu() {
            gpu.encode_figure_draw(pass);
        }
    }
}
