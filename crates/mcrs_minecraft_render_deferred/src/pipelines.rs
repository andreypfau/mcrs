use bevy::core_pipeline::FullscreenShader;
use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::view::ExtractedView;
use mcrs_minecraft_mesh::block::Pass;
use mcrs_minecraft_render::{
    RenderPath, Shape, TERRAIN_PIPELINES, Terrain, pipeline_descriptor, terrain_slot,
};

use crate::ParityMask;
use crate::gbuffer::{GBUFFER_FORMATS, gbuffer_layout};
use crate::views::Variant;
use crate::volume::volume_layout;

/// Queued once, on the first frame the deferred path is asked for, and never dropped: the
/// pipeline cache neither deduplicates nor evicts, so queuing again would leak a set.
#[derive(Resource, Default)]
pub(crate) struct DeferredPipelines {
    pub lighting: Option<CachedRenderPipelineId>,
    pub terrain: [Option<CachedRenderPipelineId>; TERRAIN_PIPELINES],
    variants: [Option<CachedRenderPipelineId>; Variant::ALL.len()],
}

impl DeferredPipelines {
    fn gated(&self) -> impl Iterator<Item = CachedRenderPipelineId> + '_ {
        self.lighting
            .into_iter()
            .chain(self.terrain.iter().copied().flatten())
    }

    pub fn ids(&self) -> impl Iterator<Item = CachedRenderPipelineId> + '_ {
        self.gated().chain(self.variants.iter().copied().flatten())
    }

    pub fn variant<'cache>(
        &self,
        variant: Variant,
        cache: &'cache PipelineCache,
    ) -> Option<&'cache RenderPipeline> {
        cache.get_render_pipeline(self.variants[variant as usize]?)
    }

    pub fn terrain<'cache>(
        &self,
        slot: usize,
        cache: &'cache PipelineCache,
    ) -> Option<&'cache RenderPipeline> {
        cache.get_render_pipeline(self.terrain[slot]?)
    }

    pub fn ready(&self, cache: &PipelineCache) -> bool {
        self.lighting.is_some()
            && self
                .gated()
                .all(|id| cache.get_render_pipeline(id).is_some())
    }
}

pub(crate) fn prepare_deferred_pipelines(
    mut pipelines: ResMut<DeferredPipelines>,
    requested: Res<RenderPath>,
    mask: Res<ParityMask>,
    terrain: Option<Res<Terrain>>,
    views: Query<&ExtractedView, With<Camera3d>>,
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
    let lighting =
        asset_server.load("embedded://mcrs_minecraft_render_deferred/shaders/lighting.wgsl");
    let show = asset_server.load("embedded://mcrs_minecraft_render_deferred/shaders/show.wgsl");
    let fullscreen_pipeline = |label: &str, shader: &Handle<Shader>, entry: &str, def: &str| {
        let mut descriptor = RenderPipelineDescriptor {
            vertex: fullscreen.to_vertex_state(),
            ..pipeline_descriptor(
                label.into(),
                vec![
                    terrain.view_layout().clone(),
                    terrain.draw_layout().clone(),
                    gbuffer_layout(),
                    volume_layout(),
                ],
                shader,
                String::new(),
                entry.into(),
                view,
                None,
            )
        };
        if !def.is_empty() {
            let fragment = descriptor.fragment.as_mut().expect("a fullscreen fragment");
            fragment.shader_defs.push(def.into());
        }
        cache.queue_render_pipeline(descriptor)
    };
    pipelines.lighting = Some(fullscreen_pipeline(
        "deferred lighting",
        &lighting,
        "lighting",
        if mask.0 { "PARITY_MASK" } else { "" },
    ));
    for variant in Variant::ALL {
        let (shader, entry, def, label) = match variant {
            Variant::LightingTerm => (&lighting, "lighting", "LIGHTING_TERM", "lighting term"),
            Variant::Unlit => (&lighting, "lighting", "UNLIT", "unlit"),
            Variant::Albedo => (&show, "show", "SHOW_ALBEDO", "albedo"),
            Variant::Ao => (&show, "show", "SHOW_AO", "AO"),
            Variant::Normals => (&show, "show", "SHOW_NORMAL", "normals"),
            Variant::BlockLight => (&show, "show", "SHOW_BLOCK_LIGHT", "block light"),
            Variant::SkyLight => (&show, "show", "SHOW_SKY_LIGHT", "sky light"),
            Variant::Grid => (&show, "show", "SHOW_GRID", "block grid"),
        };
        pipelines.variants[variant as usize] = Some(fullscreen_pipeline(
            &format!("deferred {label}"),
            shader,
            entry,
            def,
        ));
    }
    for (layer, shape, wireframe) in gbuffer_slots().chain(forward_slots()) {
        let descriptor = deferred_descriptor(&terrain, layer, shape, wireframe, view, mask.0);
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

fn forward_slots() -> impl Iterator<Item = (Pass, Shape, bool)> {
    Shape::ALL
        .into_iter()
        .flat_map(|shape| [false, true].map(move |wireframe| (Pass::Translucent, shape, wireframe)))
}

fn deferred_descriptor(
    terrain: &Terrain,
    layer: Pass,
    shape: Shape,
    wireframe: bool,
    view: &ExtractedView,
    parity_mask: bool,
) -> RenderPipelineDescriptor {
    let mut descriptor = terrain.pipeline_descriptor(layer, shape, wireframe, view);
    descriptor.label = descriptor
        .label
        .map(|label| format!("{label} deferred").into());
    let writes_gbuffer = layer != Pass::Translucent;
    // A translucent greedy face writes no G-buffer, so it needs no face out of the vertex stage.
    if writes_gbuffer || shape == Shape::Model {
        descriptor.vertex.entry_point = Some(format!("vertex_{}_deferred", shape.label()).into());
    }
    let defs: &[&str] = if parity_mask {
        &["DEFERRED", "PARITY_MASK"]
    } else {
        &["DEFERRED"]
    };
    descriptor
        .vertex
        .shader_defs
        .extend(defs.iter().map(|&def| def.into()));
    let fragment = descriptor
        .fragment
        .as_mut()
        .expect("terrain pipelines have a fragment stage");
    fragment.entry_point =
        Some(format!("fragment_{}_{}_deferred", shape.label(), layer.label()).into());
    fragment
        .shader_defs
        .extend(defs.iter().map(|&def| def.into()));
    if writes_gbuffer {
        fragment.targets = GBUFFER_FORMATS
            .map(|format| {
                Some(ColorTargetState {
                    format,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })
            })
            .into();
    }
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

    #[test]
    fn the_deferred_table_fills_every_terrain_slot_exactly_once() {
        let mut slots: Vec<usize> = gbuffer_slots()
            .chain(forward_slots())
            .map(|(layer, shape, wireframe)| terrain_slot(layer, shape, wireframe))
            .collect();
        slots.sort_unstable();
        assert_eq!(slots, (0..TERRAIN_PIPELINES).collect::<Vec<_>>());
        assert!(forward_slots().all(|(layer, _, _)| layer == Pass::Translucent));
    }
}
