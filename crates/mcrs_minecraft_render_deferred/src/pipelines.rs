use bevy::core_pipeline::FullscreenShader;
use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::view::ExtractedView;
use mcrs_minecraft_render::{RenderPath, Terrain, pipeline_descriptor};

use crate::gbuffer::gbuffer_layout;

/// Queued once, on the first frame the deferred path is asked for, and never dropped: the
/// pipeline cache neither deduplicates nor evicts, so queuing again would leak a set.
#[derive(Resource, Default)]
pub(crate) struct DeferredPipelines {
    pub lighting: Option<CachedRenderPipelineId>,
}

impl DeferredPipelines {
    pub fn ids(&self) -> impl Iterator<Item = CachedRenderPipelineId> + '_ {
        self.lighting.into_iter()
    }

    pub fn ready(&self, cache: &PipelineCache) -> bool {
        self.lighting.is_some() && self.ids().all(|id| cache.get_render_pipeline(id).is_some())
    }
}

pub(crate) fn prepare_deferred_pipelines(
    mut pipelines: ResMut<DeferredPipelines>,
    requested: Res<RenderPath>,
    terrain: Option<Res<Terrain>>,
    views: Query<&ExtractedView>,
    fullscreen: Res<FullscreenShader>,
    asset_server: Res<AssetServer>,
    cache: Res<PipelineCache>,
) {
    if *requested != RenderPath::Deferred || pipelines.lighting.is_some() {
        return;
    }
    let (Some(terrain), Some(view)) = (terrain, views.iter().next()) else {
        return;
    };
    let shader = asset_server.load("embedded://mcrs_minecraft_render_deferred/shaders/lighting.wgsl");
    let lighting = RenderPipelineDescriptor {
        vertex: fullscreen.to_vertex_state(),
        ..pipeline_descriptor(
            "deferred lighting".into(),
            vec![
                terrain.view_layout().clone(),
                terrain.draw_layout().clone(),
                gbuffer_layout(),
            ],
            &shader,
            String::new(),
            "lighting".into(),
            view,
            None,
        )
    };
    pipelines.lighting = Some(cache.queue_render_pipeline(lighting));
    info!("queued the deferred pipelines");
}
