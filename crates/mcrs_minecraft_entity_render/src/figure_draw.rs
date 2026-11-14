use bevy::ecs::system::SystemParam;
use bevy::prelude::*;

use crate::render_world::EntityRenderState;

/// The draw of the figure view, for a pass of the host in the render world that runs after
/// [`crate::FigureViewSystems`]. A host with Bevy's tracked render pass gives its
/// `wgpu_pass()`.
#[derive(SystemParam)]
pub struct FigureDraw<'w> {
    state: Res<'w, EntityRenderState>,
}

impl FigureDraw<'_> {
    /// Draws the entities of the figure view into `pass`, lit by the lightmap of the terrain,
    /// inside the scissor rectangle and the viewport depth range that the caller has set. It
    /// sets its own pipeline and bind groups, so the caller sets its own again after the call.
    /// It draws nothing when no figure frame is in this frame or when the figure view is not
    /// ready.
    pub fn encode(&self, pass: &mut wgpu::RenderPass<'_>) {
        if !self.state.has_figure() {
            return;
        }
        if let Some(gpu) = self.state.gpu() {
            gpu.encode_figure_draw(pass);
        }
    }
}
