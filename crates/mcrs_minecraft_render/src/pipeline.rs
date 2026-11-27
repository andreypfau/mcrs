use bevy::core_pipeline::FullscreenShader;
use bevy::core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy::mesh::VertexBufferLayout;
use bevy::platform::collections::HashSet;
use bevy::platform::time::Instant;
use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::view::ExtractedView;
use bevy::shader::ShaderCacheError;

use mcrs_minecraft_mesh::block::Pass;
use mcrs_minecraft_mesh::{DrawKind, Group, QuadFormat};

use super::binds::Bindings;
use super::gbuffer::{DeferredFrame, GBUFFER_FORMATS, WORLD_IMAGE_FORMAT};
use super::layer::blend;
use super::render_path::RenderPath;
use super::shaders::Shaders;
use super::shared_bindings::{gbuffer, present, transparency, transparent_gbuffer};
use super::terrain::Terrain;
use super::views::Variant;
use super::{ImprovedTransparency, LightTint};

pub const TERRAIN_PIPELINES: usize = DrawKind::COUNT * 2;

pub const fn terrain_slot(kind: DrawKind, wireframe: bool) -> usize {
    kind.index() * 2 + wireframe as usize
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
    descriptor(
        label,
        layout,
        shader,
        vertex,
        fragment,
        view.target_format,
        blend,
    )
}

pub(crate) fn descriptor(
    label: String,
    layout: Vec<BindGroupLayoutDescriptor>,
    shader: &Handle<Shader>,
    vertex: String,
    fragment: String,
    format: TextureFormat,
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
                format,
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

const GROUP_INSTANCE: [VertexAttribute; 1] = [VertexAttribute {
    format: VertexFormat::Uint32x3,
    offset: 0,
    shader_location: 0,
}];

pub(crate) fn terrain_descriptor(
    binds: &Bindings,
    shaders: &Shaders,
    path: RenderPath,
    kind: DrawKind,
    wireframe: bool,
) -> RenderPipelineDescriptor {
    let mut descriptor = RenderPipelineDescriptor {
        primitive: PrimitiveState {
            topology: PrimitiveTopology::TriangleList,
            front_face: FrontFace::Ccw,
            cull_mode: kind.writes_depth().then_some(Face::Back),
            ..default()
        },
        depth_stencil: Some(DepthStencilState {
            format: CORE_3D_DEPTH_FORMAT,
            depth_write_enabled: Some(kind.writes_depth()),
            depth_compare: Some(super::DEPTH_COMPARE),
            stencil: default(),
            bias: model_depth_bias(kind),
        }),
        ..descriptor(
            format!(
                "terrain {}{}",
                kind.label(),
                if wireframe { " wireframe" } else { "" }
            ),
            vec![binds.view_layout.clone(), binds.draw_layout.clone()],
            shaders.format(kind.format),
            format!("vertex_{}", kind.format.label()),
            format!("fragment_{}_{}", kind.format.label(), kind.layer.label()),
            WORLD_IMAGE_FORMAT,
            blend(kind.layer),
        )
    };
    descriptor.vertex.shader_defs = path.shader_defs();
    if kind.writes_depth() {
        descriptor.vertex.shader_defs.push("SOLID".into());
    }
    if !path.is_full() {
        descriptor.vertex.buffers = vec![VertexBufferLayout {
            array_stride: size_of::<Group>() as u64,
            step_mode: VertexStepMode::Instance,
            attributes: GROUP_INSTANCE.to_vec(),
        }];
    }
    if let Some(fragment) = descriptor.fragment.as_mut() {
        fragment.shader_defs = path.shader_defs();
        if kind.writes_depth() {
            fragment.shader_defs.push("SOLID".into());
        }
        if wireframe {
            fragment.shader_defs.push("WIREFRAME".into());
        }
        if kind.layer != Pass::Translucent {
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
    }
    descriptor
}

pub(super) const MODEL_DEPTH_BIAS: DepthBiasState = DepthBiasState {
    constant: 2,
    slope_scale: 1.0,
    clamp: 0.0,
};

fn model_depth_bias(kind: DrawKind) -> DepthBiasState {
    if kind.format == QuadFormat::Model && kind.writes_depth() {
        MODEL_DEPTH_BIAS
    } else {
        default()
    }
}

#[derive(Resource, Default)]
pub(crate) struct DeferredPipelines {
    pub lighting: Option<CachedRenderPipelineId>,
    pub present: Option<CachedRenderPipelineId>,
    pub terrain: [Option<CachedRenderPipelineId>; TERRAIN_PIPELINES],
    variants: [Option<CachedRenderPipelineId>; Variant::ALL.len()],
    pub tinted: bool,
    pub transparency: Option<TransparencyPipelines>,
}

pub(crate) struct TransparencyPipelines {
    pub lighting: CachedRenderPipelineId,
    translucent: [CachedRenderPipelineId; QuadFormat::COUNT],
}

impl TransparencyPipelines {
    fn ids(&self) -> impl Iterator<Item = CachedRenderPipelineId> + '_ {
        std::iter::once(self.lighting).chain(self.translucent.iter().copied())
    }

    pub fn ready(&self, cache: &PipelineCache) -> bool {
        self.ids().all(|id| cache.get_render_pipeline(id).is_some())
    }

    pub fn translucent<'cache>(
        &self,
        format: QuadFormat,
        cache: &'cache PipelineCache,
    ) -> Option<&'cache RenderPipeline> {
        cache.get_render_pipeline(*self.translucent.get(format as usize)?)
    }
}

impl DeferredPipelines {
    pub fn transparency_supported(&self, terrain: &Terrain, tint: Option<&LightTint>) -> bool {
        terrain.path() != RenderPath::Minimal || !self.tinted || tint.is_none()
    }

    fn tint_ready(&self, tint: Option<&LightTint>) -> bool {
        !self.tinted || tint.is_some_and(|tint| tint.bind_group.is_some())
    }

    fn gated(&self) -> impl Iterator<Item = CachedRenderPipelineId> + '_ {
        self.lighting
            .into_iter()
            .chain(self.present)
            .chain(self.terrain.iter().copied().flatten())
    }

    pub fn ids(&self) -> impl Iterator<Item = CachedRenderPipelineId> + '_ {
        self.gated()
            .chain(self.variants.iter().copied().flatten())
            .chain(
                self.transparency
                    .iter()
                    .flat_map(TransparencyPipelines::ids),
            )
    }

    pub fn variant<'cache>(
        &self,
        variant: Variant,
        cache: &'cache PipelineCache,
    ) -> Option<&'cache RenderPipeline> {
        cache.get_render_pipeline((*self.variants.get(variant as usize)?)?)
    }

    pub fn terrain<'cache>(
        &self,
        slot: usize,
        cache: &'cache PipelineCache,
    ) -> Option<&'cache RenderPipeline> {
        cache.get_render_pipeline((*self.terrain.get(slot)?)?)
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

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::ungated_system,
        reason = "the system checks the pipeline cache every frame for failed deferred pipelines"
    )
)]
#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "the pipelines are queued only when the terrain first appears or when improved transparency switches on"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(28))]
#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::blocking_in_system,
        reason = "the wall clock measures once how long the deferred path takes to become ready, for one log line"
    )
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
    improved: Option<Res<ImprovedTransparency>>,
    mut failed: Local<HashSet<CachedRenderPipelineId>>,
    mut waited: Local<Option<(Instant, u32)>>,
    mut announced: Local<bool>,
) {
    if let (Some(terrain), Some(view)) = (terrain.as_deref(), views.iter().next()) {
        if pipelines.lighting.is_none() {
            queue_pipelines(
                &mut pipelines,
                tint.as_deref().map(|tint| &tint.layout),
                terrain,
                view,
                &fullscreen,
                &asset_server,
                &cache,
            );
        }
        let tint_layout = tint
            .as_deref()
            .map(|tint| &tint.layout)
            .filter(|_| pipelines.tinted);
        if pipelines.transparency.is_none()
            && pipelines.transparency_supported(terrain, tint.as_deref())
            && improved.is_some_and(|improved| improved.0)
        {
            pipelines.transparency = Some(queue_transparency(
                tint_layout,
                terrain,
                &fullscreen,
                &asset_server,
                &cache,
            ));
        }
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

fn queue_lighting_and_views(
    pipelines: &mut DeferredPipelines,
    tint_layout: Option<&BindGroupLayoutDescriptor>,
    terrain: &Terrain,
    fullscreen: &FullscreenShader,
    asset_server: &AssetServer,
    cache: &PipelineCache,
) {
    pipelines.tinted = tint_layout.is_some();
    let lighting =
        asset_server.load("embedded://mcrs_minecraft_render/shaders/core/lighting_pass.wesl");
    let show =
        asset_server.load("embedded://mcrs_minecraft_render/shaders/core/gbuffer_views.wesl");
    let lighting_layout: Vec<_> = [
        terrain.view_layout().clone(),
        terrain.draw_layout().clone(),
        gbuffer::layout(),
    ]
    .into_iter()
    .chain(tint_layout.cloned())
    .collect();
    let views_layout = vec![terrain.view_layout().clone(), gbuffer::layout()];
    let fullscreen_pipeline = |label: &str,
                               layout: Vec<BindGroupLayoutDescriptor>,
                               shader: &Handle<Shader>,
                               entry: &str,
                               def: &'static str,
                               tinted: bool| {
        let mut descriptor = RenderPipelineDescriptor {
            vertex: fullscreen.to_vertex_state(),
            ..descriptor(
                label.into(),
                layout,
                shader,
                String::new(),
                entry.into(),
                WORLD_IMAGE_FORMAT,
                None,
            )
        };
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader_defs = terrain.path().shader_defs();
            if !def.is_empty() {
                fragment.shader_defs.push(def.into());
            }
            if tinted {
                fragment.shader_defs.push("LIGHT_TINT".into());
            }
        }
        cache.queue_render_pipeline(descriptor)
    };
    pipelines.lighting = Some(fullscreen_pipeline(
        "deferred lighting",
        lighting_layout.clone(),
        &lighting,
        "lighting",
        "",
        tint_layout.is_some(),
    ));
    for variant in Variant::ALL {
        let (shader, entry, def, label) = match variant {
            Variant::LightingTerm => (&lighting, "lighting", "LIGHTING_TERM", "lighting term"),
            Variant::Unlit => (&show, "show", "SHOW_UNLIT", "unlit"),
            Variant::Albedo => (&show, "show", "SHOW_ALBEDO", "albedo"),
            Variant::Ao => (&show, "show", "SHOW_AO", "AO"),
            Variant::Normals => (&show, "show", "SHOW_NORMAL", "normals"),
            Variant::BlockLight => (&show, "show", "SHOW_BLOCK_LIGHT", "block light"),
            Variant::SkyLight => (&show, "show", "SHOW_SKY_LIGHT", "sky light"),
            Variant::Grid => (&show, "show", "SHOW_GRID", "block grid"),
        };
        let (layout, tinted) = if variant.is_gbuffer_view() {
            (views_layout.clone(), false)
        } else {
            (lighting_layout.clone(), tint_layout.is_some())
        };
        if let Some(slot) = pipelines.variants.get_mut(variant as usize) {
            *slot = Some(fullscreen_pipeline(
                &format!("deferred {label}"),
                layout,
                shader,
                entry,
                def,
                tinted,
            ));
        }
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
    let present = asset_server.load("embedded://mcrs_minecraft_render/shaders/core/present.wesl");
    pipelines.present = Some(cache.queue_render_pipeline(RenderPipelineDescriptor {
        vertex: fullscreen.to_vertex_state(),
        ..common(
            "present".into(),
            vec![present::layout()],
            &present,
            String::new(),
            "present".into(),
            view,
            None,
        )
    }));
    queue_lighting_and_views(
        pipelines,
        tint_layout,
        terrain,
        fullscreen,
        asset_server,
        cache,
    );
    for (kind, wireframe) in gbuffer_slots().chain(forward_slots()) {
        let descriptor = terrain_descriptor(
            &terrain.binds,
            &terrain.shaders,
            terrain.path(),
            kind,
            wireframe,
        );
        if let Some(slot) = pipelines.terrain.get_mut(terrain_slot(kind, wireframe)) {
            *slot = Some(cache.queue_render_pipeline(descriptor));
        }
    }
    info!("queued the deferred pipelines");
}

fn queue_transparency(
    tint_layout: Option<&BindGroupLayoutDescriptor>,
    terrain: &Terrain,
    fullscreen: &FullscreenShader,
    asset_server: &AssetServer,
    cache: &PipelineCache,
) -> TransparencyPipelines {
    let lighting_shader =
        asset_server.load("embedded://mcrs_minecraft_render/shaders/core/lighting_pass.wesl");
    let groups = [terrain.view_layout().clone(), terrain.draw_layout().clone()];
    let mut lighting = RenderPipelineDescriptor {
        vertex: fullscreen.to_vertex_state(),
        ..descriptor(
            "deferred lighting with improved transparency".into(),
            groups
                .iter()
                .cloned()
                .chain([transparent_gbuffer::layout()])
                .chain(tint_layout.cloned())
                .collect(),
            &lighting_shader,
            String::new(),
            "lighting".into(),
            WORLD_IMAGE_FORMAT,
            None,
        )
    };
    if let Some(fragment) = lighting.fragment.as_mut() {
        fragment.shader_defs = terrain.path().shader_defs();
        fragment.shader_defs.push("IMPROVED".into());
        if tint_layout.is_some() {
            fragment.shader_defs.push("LIGHT_TINT".into());
        }
    }
    let translucent = QuadFormat::ALL.map(|format| {
        let kind = DrawKind::new(Pass::Translucent, format);
        let mut descriptor = terrain_descriptor(
            &terrain.binds,
            &terrain.shaders,
            terrain.path(),
            kind,
            false,
        );
        descriptor.label = Some(format!("terrain translucent {} improved", format.label()).into());
        descriptor.layout.push(transparency::layout());
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.shader_defs.push("IMPROVED".into());
        }
        cache.queue_render_pipeline(descriptor)
    });
    info!("queued the improved transparency pipelines");
    TransparencyPipelines {
        lighting: cache.queue_render_pipeline(lighting),
        translucent,
    }
}

fn gbuffer_slots() -> impl Iterator<Item = (DrawKind, bool)> {
    [Pass::Solid, Pass::Cutout].into_iter().flat_map(|layer| {
        QuadFormat::ALL.into_iter().flat_map(move |format| {
            let kind = DrawKind::new(layer, format);
            [false, true].map(move |wireframe| (kind, wireframe))
        })
    })
}

fn forward_slots() -> impl Iterator<Item = (DrawKind, bool)> {
    QuadFormat::ALL.into_iter().flat_map(|format| {
        let kind = DrawKind::new(Pass::Translucent, format);
        [false, true].map(move |wireframe| (kind, wireframe))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_deferred_table_fills_every_terrain_slot_exactly_once() {
        let mut slots: Vec<usize> = gbuffer_slots()
            .chain(forward_slots())
            .map(|(kind, wireframe)| terrain_slot(kind, wireframe))
            .collect();
        slots.sort_unstable();
        assert_eq!(slots, (0..TERRAIN_PIPELINES).collect::<Vec<_>>());
        assert!(gbuffer_slots().all(|(kind, _)| kind.layer != Pass::Translucent));
        assert!(forward_slots().all(|(kind, _)| kind.layer == Pass::Translucent));
    }

    #[test]
    fn the_table_holds_one_pipeline_per_draw_kind_and_wireframe() {
        let mut slots: Vec<usize> = DrawKind::ALL
            .iter()
            .flat_map(|&kind| [false, true].map(|wireframe| terrain_slot(kind, wireframe)))
            .collect();
        slots.sort_unstable();
        assert_eq!(slots, (0..TERRAIN_PIPELINES).collect::<Vec<_>>());
    }
}
