use bevy::core_pipeline::FullscreenShader;
use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::view::ExtractedView;
use mcrs_minecraft_mesh::block::Pass;
use mcrs_minecraft_render::{
    RenderPath, Shape, TERRAIN_PIPELINES, Terrain, pipeline_descriptor, terrain_slot,
};

use crate::gbuffer::{GBUFFER_FORMATS, gbuffer_layout};

/// Queued once, on the first frame the deferred path is asked for, and never dropped: the
/// pipeline cache neither deduplicates nor evicts, so queuing again would leak a set.
#[derive(Resource, Default)]
pub(crate) struct DeferredPipelines {
    pub lighting: Option<CachedRenderPipelineId>,
    pub terrain: [Option<CachedRenderPipelineId>; TERRAIN_PIPELINES],
}

impl DeferredPipelines {
    pub fn ids(&self) -> impl Iterator<Item = CachedRenderPipelineId> + '_ {
        self.lighting
            .into_iter()
            .chain(self.terrain.iter().copied().flatten())
    }

    pub fn gbuffer<'cache>(
        &self,
        slot: usize,
        cache: &'cache PipelineCache,
    ) -> Option<&'cache RenderPipeline> {
        cache.get_render_pipeline(self.terrain[slot]?)
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
    let shader =
        asset_server.load("embedded://mcrs_minecraft_render_deferred/shaders/lighting.wgsl");
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
    for (layer, shape, wireframe) in gbuffer_slots() {
        let descriptor = gbuffer_descriptor(&terrain, layer, shape, wireframe, view);
        pipelines.terrain[terrain_slot(layer, shape, wireframe)] =
            Some(cache.queue_render_pipeline(descriptor));
    }
    info!("queued the deferred pipelines");
}

fn gbuffer_slots() -> impl Iterator<Item = (Pass, Shape, bool)> {
    [Pass::Solid, Pass::Cutout].into_iter().flat_map(|layer| {
        Shape::ALL
            .into_iter()
            .flat_map(move |shape| [false, true].map(move |wireframe| (layer, shape, wireframe)))
    })
}

fn gbuffer_descriptor(
    terrain: &Terrain,
    layer: Pass,
    shape: Shape,
    wireframe: bool,
    view: &ExtractedView,
) -> RenderPipelineDescriptor {
    let mut descriptor = terrain.pipeline_descriptor(layer, shape, wireframe, view);
    descriptor.label = descriptor
        .label
        .map(|label| format!("{label} deferred").into());
    descriptor.vertex.entry_point = Some(format!("vertex_{}_deferred", shape.label()).into());
    descriptor.vertex.shader_defs.push("DEFERRED".into());
    let fragment = descriptor
        .fragment
        .as_mut()
        .expect("terrain pipelines have a fragment stage");
    fragment.entry_point =
        Some(format!("fragment_{}_{}_deferred", shape.label(), layer.label()).into());
    fragment.shader_defs.push("DEFERRED".into());
    fragment.targets = GBUFFER_FORMATS
        .map(|format| {
            Some(ColorTargetState {
                format,
                blend: None,
                write_mask: ColorWrites::ALL,
            })
        })
        .into();
    descriptor
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_deferred_table_holds_one_gbuffer_pipeline_per_opaque_layer_shape_and_wireframe() {
        let mut slots: Vec<usize> = gbuffer_slots()
            .map(|(layer, shape, wireframe)| terrain_slot(layer, shape, wireframe))
            .collect();
        slots.sort_unstable();
        slots.dedup();
        assert_eq!(slots.len(), 8);
        assert!(gbuffer_slots().all(|(layer, _, _)| layer != Pass::Translucent));
    }
}
