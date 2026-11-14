use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::renderer::RenderDevice;

use crate::render_world::EntityRenderState;

/// The draw of the figure view, for a pass of the host in the render world that runs after
/// [`crate::FigureViewSystems`]. A host with Bevy's tracked render pass gives its
/// `wgpu_pass()`.
#[derive(SystemParam)]
pub struct FigureDraw<'w> {
    state: ResMut<'w, EntityRenderState>,
    device: Res<'w, RenderDevice>,
}

impl FigureDraw<'_> {
    /// Draws the entities of the figure view into `pass`, lit by the lightmap `lightmap` with
    /// its sampler, inside the scissor rectangle and the viewport depth range that the caller has
    /// set. It sets its own pipeline and bind groups, so the caller sets its own again after
    /// the call. It draws nothing when no figure frame is in this frame or when the figure view
    /// is not ready.
    pub fn encode(
        &mut self,
        pass: &mut wgpu::RenderPass<'_>,
        lightmap: &wgpu::TextureView,
        sampler: &wgpu::Sampler,
    ) {
        if self.state.figure().is_none() {
            return;
        }
        let device = self.device.wgpu_device();
        if let Some(gpu) = self.state.gpu_mut() {
            gpu.encode_figure_draw(device, pass, lightmap, sampler);
        }
    }
}
