use bevy::core_pipeline::FullscreenShader;
use bevy::core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy::platform::collections::HashSet;
use bevy::platform::time::Instant;
use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::view::ExtractedView;
use bevy::shader::{ShaderCacheError, ShaderDefVal};

use mcrs_minecraft_mesh::block::Pass;
use mcrs_minecraft_mesh::{STREAMS, stream_pass};

use super::LightTint;
use super::binds::Bindings;
use super::gbuffer::{DeferredFrame, GBUFFER_FORMATS, gbuffer_layout};
use super::layer::{Shape, blend};
use super::shaders::Shaders;
use super::terrain::Terrain;
use super::views::Variant;

pub const TERRAIN_PIPELINES: usize = Pass::COUNT * Shape::ALL.len() * 2;

pub const fn terrain_slot(layer: Pass, shape: Shape, wireframe: bool) -> usize {
    (layer as usize * Shape::ALL.len() + shape as usize) * 2 + wireframe as usize
}

pub fn stream_slot(stream: u32, wireframe: bool) -> usize {
    terrain_slot(stream_pass(stream), Shape::of_stream(stream), wireframe)
}

pub fn common(
    label: String,
    layout: Vec<BindGroupLayoutDescriptor>,
    shader: &Handle<Shader>,
    vertex: String,
    fragment: String,
    view: &ExtractedView,
    blend: Option<BlendState>,
) -> RenderPipelineDescriptor {
    RenderPipelineDescriptor {
        label: Some(label.into()),
        layout,
        vertex: VertexState {
            shader: shader.clone(),
            entry_point: Some(vertex.into()),
            ..default()
        },
        fragment: Some(FragmentState {
            shader: shader.clone(),
            entry_point: Some(fragment.into()),
            targets: vec![Some(ColorTargetState {
                format: view.target_format,
                blend,
                write_mask: ColorWrites::ALL,
            })],
            ..default()
        }),
        multisample: MultisampleState {
            count: 1,
            ..default()
        },
        ..default()
    }
}

pub(super) struct Pipelines {
    pub shaders: Shaders,
    pub cull_sections: CachedComputePipelineId,
    pub cull_groups: CachedComputePipelineId,
    pub cull_quads: CachedComputePipelineId,
    pub cull_count: CachedComputePipelineId,
    pub cull_scan: CachedComputePipelineId,
    pub cull_scatter: CachedComputePipelineId,
    pub cull_groups_second: CachedComputePipelineId,
    pub cull_quads_second: CachedComputePipelineId,
}

impl Pipelines {
    pub fn new(
        shaders: Shaders,
        binds: &Bindings,
        section_slots: usize,
        max_workgroups: u32,
        subgroups: bool,
        pipeline_cache: &PipelineCache,
    ) -> Self {
        let layout = vec![binds.view_layout.clone(), binds.cull_layout.clone()];
        let quad_layout = vec![binds.view_layout.clone(), binds.quad_cull_layout.clone()];
        let mut shader_defs = vec![
            ShaderDefVal::UInt("CULL_THREADS".into(), super::pass::CULL_THREADS),
            ShaderDefVal::UInt("GROUP_THREADS".into(), super::pass::GROUP_THREADS),
            ShaderDefVal::UInt("GROUPS_PER_STEP".into(), super::pass::GROUPS_PER_STEP),
            ShaderDefVal::UInt("STREAMS".into(), STREAMS as u32),
            ShaderDefVal::UInt("SECTION_SLOTS".into(), section_slots as u32),
            ShaderDefVal::UInt("CAVE_WORDS".into(), section_slots.div_ceil(32) as u32),
            ShaderDefVal::UInt("MAX_WORKGROUPS".into(), max_workgroups),
            ShaderDefVal::Int("MODEL_BIAS_CONSTANT".into(), MODEL_DEPTH_BIAS.constant),
            ShaderDefVal::UInt(
                "MODEL_BIAS_SLOPE_BITS".into(),
                MODEL_DEPTH_BIAS.slope_scale.to_bits(),
            ),
        ];
        if subgroups {
            shader_defs.push("SUBGROUPS".into());
        }
        let compute = |label: &str, layout: &Vec<BindGroupLayoutDescriptor>, entry: &str| {
            pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
                label: Some(label.to_owned().into()),
                layout: layout.clone(),
                shader: shaders.cull.clone(),
                shader_defs: shader_defs.clone(),
                entry_point: Some(entry.to_owned().into()),
                ..default()
            })
        };
        Self {
            cull_sections: compute("terrain cull sections", &quad_layout, "cull_sections"),
            cull_groups: compute("terrain cull groups", &quad_layout, "cull_groups"),
            cull_quads: compute("terrain cull quads", &quad_layout, "cull_quads"),
            cull_count: compute("terrain cull count", &layout, "count_ordered"),
            cull_scan: compute("terrain cull scan", &layout, "scan_ordered"),
            cull_scatter: compute("terrain cull scatter", &layout, "scatter_ordered"),
            cull_groups_second: compute(
                "terrain cull groups second",
                &quad_layout,
                "cull_groups_second",
            ),
            cull_quads_second: compute(
                "terrain cull quads second",
                &quad_layout,
                "cull_quads_second",
            ),
            shaders,
        }
    }
}

pub(crate) fn terrain_descriptor(
    binds: &Bindings,
    shaders: &Shaders,
    layer: Pass,
    shape: Shape,
    wireframe: bool,
    view: &ExtractedView,
) -> RenderPipelineDescriptor {
    let mut descriptor = RenderPipelineDescriptor {
        primitive: PrimitiveState {
            topology: PrimitiveTopology::TriangleList,
            front_face: FrontFace::Ccw,
            cull_mode: layer.writes_depth().then_some(Face::Back),
            ..default()
        },
        depth_stencil: Some(DepthStencilState {
            format: CORE_3D_DEPTH_FORMAT,
            depth_write_enabled: Some(layer.writes_depth()),
            depth_compare: Some(super::DEPTH_COMPARE),
            stencil: default(),
            bias: model_depth_bias(layer, shape),
        }),
        ..common(
            format!(
                "terrain {} {}{}",
                layer.label(),
                shape.label(),
                if wireframe { " wireframe" } else { "" }
            ),
            vec![binds.view_layout.clone(), binds.draw_layout.clone()],
            shaders.shape(shape),
            format!("vertex_{}", shape.label()),
            format!("fragment_{}_{}", shape.label(), layer.label()),
            view,
            blend(layer),
        )
    };
    let fragment = descriptor
        .fragment
        .as_mut()
        .expect("common sets a fragment");
    if wireframe {
        fragment.shader_defs.push("WIREFRAME".into());
    }
    if layer != Pass::Translucent {
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

// Model quads sit flush against the greedy faces behind them, so without a nudge the two
// fight for the same depth. The cull allows for the nudge when it tests a model quad.
const MODEL_DEPTH_BIAS: DepthBiasState = DepthBiasState {
    constant: 2,
    slope_scale: 1.0,
    clamp: 0.0,
};

fn model_depth_bias(layer: Pass, shape: Shape) -> DepthBiasState {
    if shape == Shape::Model && layer.writes_depth() {
        MODEL_DEPTH_BIAS
    } else {
        default()
    }
}

/// Queued once, on the first frame the terrain and a 3D view exist, and never dropped: the
/// pipeline cache neither deduplicates nor evicts, so queuing again would leak a set.
#[derive(Resource, Default)]
pub(crate) struct DeferredPipelines {
    pub lighting: Option<CachedRenderPipelineId>,
    pub terrain: [Option<CachedRenderPipelineId>; TERRAIN_PIPELINES],
    variants: [Option<CachedRenderPipelineId>; Variant::ALL.len()],
    pub tinted: bool,
}

impl DeferredPipelines {
    fn tint_ready(&self, tint: Option<&LightTint>) -> bool {
        !self.tinted || tint.is_some_and(|tint| tint.bind_group.is_some())
    }

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

pub(crate) fn frame_ready(
    frame: Option<Res<DeferredFrame>>,
    tint: Option<Res<LightTint>>,
    pipelines: Res<DeferredPipelines>,
    cache: Res<PipelineCache>,
) -> bool {
    frame.is_some() && pipelines.tint_ready(tint.as_deref()) && pipelines.ready(&cache)
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy render system: each resource and query is its own parameter so the scheduler sees the access"
)]
pub(crate) fn prepare_deferred_pipelines(
    mut pipelines: ResMut<DeferredPipelines>,
    terrain: Option<Res<Terrain>>,
    views: Query<&ExtractedView, With<Camera3d>>,
    fullscreen: Res<FullscreenShader>,
    asset_server: Res<AssetServer>,
    cache: Res<PipelineCache>,
    frame: Option<Res<DeferredFrame>>,
    tint: Option<Res<LightTint>>,
    mut failed: Local<HashSet<CachedRenderPipelineId>>,
    mut waited: Local<Option<(Instant, u32)>>,
    mut announced: Local<bool>,
) {
    if pipelines.lighting.is_none()
        && let (Some(terrain), Some(view)) = (terrain, views.iter().next())
    {
        queue_pipelines(
            &mut pipelines,
            tint.as_deref().map(|tint| &tint.layout),
            &terrain,
            view,
            &fullscreen,
            &asset_server,
            &cache,
        );
    }
    for id in pipelines.ids() {
        if let CachedPipelineState::Err(error) = cache.get_render_pipeline_state(id)
            && !matches!(
                error,
                ShaderCacheError::ShaderNotLoaded(_)
                    | ShaderCacheError::ShaderImportNotYetAvailable
            )
            && failed.insert(id)
        {
            let label = cache.get_render_pipeline_descriptor(id).label.as_deref();
            error!(?label, "a deferred pipeline cannot be built: {error}");
        }
    }
    if *announced {
        return;
    }
    let (since, frames) = waited.get_or_insert((Instant::now(), 0));
    if frame.is_some() && pipelines.tint_ready(tint.as_deref()) && pipelines.ready(&cache) {
        let ms = since.elapsed().as_millis();
        info!(frames = *frames, ms, "the deferred path is ready");
        *announced = true;
    } else {
        *frames += 1;
    }
}

fn queue_pipelines(
    pipelines: &mut DeferredPipelines,
    tint_layout: Option<&BindGroupLayoutDescriptor>,
    terrain: &Terrain,
    view: &ExtractedView,
    fullscreen: &FullscreenShader,
    asset_server: &AssetServer,
    cache: &PipelineCache,
) {
    pipelines.tinted = tint_layout.is_some();
    if !pipelines.tinted {
        let stand_in: Handle<Shader> =
            asset_server.load("embedded://mcrs_minecraft_render/shaders/include/untinted.wgsl");
        // Never dropped, as `load_shader_library!` keeps a library.
        core::mem::forget(stand_in);
    }
    let lighting =
        asset_server.load("embedded://mcrs_minecraft_render/shaders/core/lighting_pass.wgsl");
    let show =
        asset_server.load("embedded://mcrs_minecraft_render/shaders/core/gbuffer_views.wgsl");
    let layout: Vec<_> = [
        terrain.view_layout().clone(),
        terrain.draw_layout().clone(),
        gbuffer_layout(),
    ]
    .into_iter()
    .chain(tint_layout.cloned())
    .collect();
    let fullscreen_pipeline = |label: &str, shader: &Handle<Shader>, entry: &str, def: &str| {
        let mut descriptor = RenderPipelineDescriptor {
            vertex: fullscreen.to_vertex_state(),
            ..common(
                label.into(),
                layout.clone(),
                shader,
                String::new(),
                entry.into(),
                view,
                None,
            )
        };
        let fragment = descriptor.fragment.as_mut().expect("a fullscreen fragment");
        if !def.is_empty() {
            fragment.shader_defs.push(def.into());
        }
        if tint_layout.is_some() {
            fragment.shader_defs.push("LIGHT_TINT".into());
        }
        cache.queue_render_pipeline(descriptor)
    };
    pipelines.lighting = Some(fullscreen_pipeline(
        "deferred lighting",
        &lighting,
        "lighting",
        "",
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
        let descriptor = terrain_descriptor(
            &terrain.binds,
            &terrain.pipelines.shaders,
            layer,
            shape,
            wireframe,
            view,
        );
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

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_deferred_table_fills_every_terrain_slot_exactly_once() {
        let mut slots: Vec<usize> = gbuffer_slots()
            .chain(forward_slots())
            .map(|(layer, shape, wireframe)| terrain_slot(layer, shape, wireframe))
            .collect();
        slots.sort_unstable();
        assert_eq!(slots, (0..TERRAIN_PIPELINES).collect::<Vec<_>>());
        assert!(gbuffer_slots().all(|(layer, _, _)| layer != Pass::Translucent));
        assert!(forward_slots().all(|(layer, _, _)| layer == Pass::Translucent));
    }

    #[test]
    fn the_table_holds_one_pipeline_per_layer_shape_and_wireframe() {
        let mut slots: Vec<usize> = Pass::ALL
            .iter()
            .flat_map(|&layer| {
                Shape::ALL.iter().flat_map(move |&shape| {
                    [false, true].map(|wireframe| terrain_slot(layer, shape, wireframe))
                })
            })
            .collect();
        slots.sort_unstable();
        assert_eq!(slots, (0..TERRAIN_PIPELINES).collect::<Vec<_>>());
    }
}
