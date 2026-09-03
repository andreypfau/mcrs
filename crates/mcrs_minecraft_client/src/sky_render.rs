use std::num::NonZeroU64;

use crate::sky_state::{SkyEffects, SkyFrame, SkyKey};
use bevy::asset::embedded_asset;
use bevy::core_pipeline::core_3d::{CORE_3D_DEPTH_FORMAT, main_opaque_pass_3d};
use bevy::core_pipeline::schedule::{Core3d, Core3dSystems};
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_phase::TrackedRenderPass;
use bevy::render::render_resource::binding_types::{
    sampler, texture_2d, texture_2d_array, uniform_buffer_sized,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery};
use bevy::render::texture::GpuImage;
use bevy::render::view::{
    ExtractedView, ViewDepthTexture, ViewTarget, ViewUniform, ViewUniformOffset, ViewUniforms,
};
use bevy::render::{Extract, ExtractSchedule, Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::Shader;
use mcrs_minecraft_world::world_clock::WorldClocks;

use crate::render::{DEPTH_COMPARE, pipeline_descriptor, uniform_buffer};
use crate::sky::{SkyEnvironment, SkyTextures, SkyUniform};

const STAR_COUNT: u32 = 1500;

// The alpha component keeps the destination rather than replacing it: these
// draws add light, and a star whose colour has faded to zero would otherwise
// leave the pixel under it transparent. That is invisible in an opaque native
// window and punches holes through to the page behind a browser canvas.
const ADDITIVE: BlendState = BlendState {
    color: BlendComponent {
        src_factor: BlendFactor::SrcAlpha,
        dst_factor: BlendFactor::One,
        operation: BlendOperation::Add,
    },
    alpha: BlendComponent {
        src_factor: BlendFactor::Zero,
        dst_factor: BlendFactor::One,
        operation: BlendOperation::Add,
    },
};

struct SkyDraw {
    label: &'static str,
    effect: SkyEffects,
    vertex: &'static str,
    fragment: &'static str,
    blend: Option<BlendState>,
    vertices: u32,
    writes_depth: bool,
    /// Whether the draw would put any colour on screen with these values.
    visible: fn(&SkyUniform) -> bool,
}

const SKY_DRAWS: [SkyDraw; 5] = [
    SkyDraw {
        label: "sky disc",
        effect: SkyEffects::DISC,
        vertex: "vertex_disc",
        fragment: "fragment_disc",
        blend: None,
        vertices: 48,
        writes_depth: false,
        visible: |_| true,
    },
    SkyDraw {
        label: "sky twilight",
        effect: SkyEffects::TWILIGHT,
        vertex: "vertex_sunrise",
        fragment: "fragment_flat",
        blend: Some(BlendState::ALPHA_BLENDING),
        vertices: 48,
        writes_depth: false,
        visible: |sky| sky.sunrise[3] > 0.001,
    },
    SkyDraw {
        label: "sky celestial",
        effect: SkyEffects::CELESTIAL,
        vertex: "vertex_celestial",
        fragment: "fragment_celestial",
        blend: Some(ADDITIVE),
        vertices: 12,
        writes_depth: false,
        visible: |_| true,
    },
    SkyDraw {
        label: "sky stars",
        effect: SkyEffects::STARS,
        vertex: "vertex_stars",
        fragment: "fragment_flat",
        blend: Some(ADDITIVE),
        vertices: STAR_COUNT * 6,
        writes_depth: false,
        visible: |sky| sky.angles[3] > 0.0,
    },
    SkyDraw {
        label: "sky clouds",
        effect: SkyEffects::CLOUDS,
        vertex: "vertex_clouds",
        fragment: "fragment_clouds",
        blend: Some(BlendState::ALPHA_BLENDING),
        vertices: 6,
        writes_depth: true,
        visible: |_| true,
    },
];

const CLOUDS: usize = 4;

pub struct SkyRenderPlugin;

impl Plugin for SkyRenderPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shaders/sky.wgsl");

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .add_systems(RenderStartup, init_sky)
            .add_systems(ExtractSchedule, extract_sky)
            .add_systems(
                Render,
                (
                    (prepare_sky, write_sky).in_set(RenderSystems::Prepare),
                    prepare_sky_bind_groups.in_set(RenderSystems::PrepareBindGroups),
                ),
            )
            .add_systems(
                Core3d,
                draw_sky
                    .in_set(Core3dSystems::MainPass)
                    .after(main_opaque_pass_3d),
            );
    }
}

#[derive(Resource)]
pub(crate) struct Sky {
    view_layout: BindGroupLayoutDescriptor,
    texture_layout: BindGroupLayoutDescriptor,
    shader: Handle<Shader>,
    uniform: Buffer,
    pipelines: Option<(SkyKey, [Option<CachedRenderPipelineId>; SKY_DRAWS.len()])>,
    textures: Option<(AssetId<Image>, AssetId<Image>, BindGroup)>,
    view: Option<(BufferId, BindGroup)>,
}

/// Restricts the sky to a subset of its draws, so a profiling run can price
/// one pass by leaving it out.
#[derive(Resource)]
pub struct SkyDrawsOnly(pub SkyEffects);

#[derive(Resource)]
pub(crate) struct ExtractedSky {
    pub uniform: SkyUniform,
    key: SkyKey,
    celestials: AssetId<Image>,
    clouds: AssetId<Image>,
}

#[derive(Resource)]
pub(crate) struct SkyBindGroups {
    view: BindGroup,
    textures: BindGroup,
}

fn init_sky(mut commands: Commands, asset_server: Res<AssetServer>, device: Res<RenderDevice>) {
    commands.insert_resource(Sky {
        view_layout: BindGroupLayoutDescriptor::new(
            "sky view",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::VERTEX_FRAGMENT,
                (
                    uniform_buffer_sized(true, Some(ViewUniform::min_size())),
                    uniform_buffer_sized(false, NonZeroU64::new(size_of::<SkyUniform>() as u64)),
                ),
            ),
        ),
        texture_layout: BindGroupLayoutDescriptor::new(
            "sky textures",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::VERTEX_FRAGMENT,
                (
                    texture_2d_array(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                    texture_2d(TextureSampleType::Float { filterable: true }),
                ),
            ),
        ),
        shader: asset_server.load("embedded://mcrs_minecraft_client/shaders/sky.wgsl"),
        uniform: uniform_buffer("sky", size_of::<SkyUniform>() as u64, &device),
        pipelines: None,
        textures: None,
        view: None,
    });
}

fn extract_sky(
    mut commands: Commands,
    environment: Extract<Option<Res<SkyEnvironment>>>,
    textures: Extract<Option<Res<SkyTextures>>>,
    frame: Extract<Res<SkyFrame>>,
    clocks: Extract<Res<WorldClocks>>,
    only: Extract<Option<Res<SkyDrawsOnly>>>,
) {
    let (Some(environment), Some(textures)) = (environment.as_ref(), textures.as_ref()) else {
        return;
    };
    let mut key = environment.key();
    if let Some(only) = only.as_ref() {
        key.effects &= only.0;
    }
    commands.insert_resource(ExtractedSky {
        uniform: environment.uniform(&frame, environment.drift(&clocks)),
        key,
        celestials: textures.celestials.id(),
        clouds: textures.clouds.id(),
    });
}

fn write_sky(sky: Option<Res<Sky>>, extracted: Option<Res<ExtractedSky>>, queue: Res<RenderQueue>) {
    let (Some(sky), Some(extracted)) = (sky, extracted) else {
        return;
    };
    queue.write_buffer(&sky.uniform, 0, bytemuck::bytes_of(&extracted.uniform));
}

fn prepare_sky(
    sky: Option<ResMut<Sky>>,
    extracted: Option<Res<ExtractedSky>>,
    views: Query<&ExtractedView>,
    pipeline_cache: Res<PipelineCache>,
) {
    let (Some(mut sky), Some(extracted)) = (sky, extracted) else {
        return;
    };
    if sky
        .pipelines
        .as_ref()
        .is_some_and(|(key, _)| *key == extracted.key)
    {
        return;
    }
    let Some(view) = views.iter().next() else {
        return;
    };
    let layout = vec![sky.view_layout.clone(), sky.texture_layout.clone()];
    let queued = SKY_DRAWS.each_ref().map(|draw| {
        extracted.key.effects.contains(draw.effect).then(|| {
            pipeline_cache.queue_render_pipeline(RenderPipelineDescriptor {
                primitive: PrimitiveState {
                    topology: PrimitiveTopology::TriangleList,
                    front_face: FrontFace::Ccw,
                    cull_mode: None,
                    ..default()
                },
                depth_stencil: Some(DepthStencilState {
                    format: CORE_3D_DEPTH_FORMAT,
                    depth_write_enabled: Some(draw.writes_depth),
                    depth_compare: Some(if draw.writes_depth {
                        DEPTH_COMPARE
                    } else {
                        CompareFunction::Always
                    }),
                    stencil: default(),
                    bias: default(),
                }),
                ..pipeline_descriptor(
                    draw.label.into(),
                    layout.clone(),
                    &sky.shader,
                    draw.vertex.into(),
                    draw.fragment.into(),
                    view,
                    draw.blend,
                )
            })
        })
    });
    info!(?extracted.key, draws = extracted.key.draws(), "queued the sky pipelines");
    sky.pipelines = Some((extracted.key, queued));
}

fn prepare_sky_bind_groups(
    mut commands: Commands,
    sky: Option<ResMut<Sky>>,
    extracted: Option<Res<ExtractedSky>>,
    images: Res<RenderAssets<GpuImage>>,
    view_uniforms: Res<ViewUniforms>,
    device: Res<RenderDevice>,
    pipeline_cache: Res<PipelineCache>,
) {
    let (Some(mut sky), Some(extracted)) = (sky, extracted) else {
        return;
    };
    let Some(view_buffer) = view_uniforms.uniforms.buffer() else {
        return;
    };
    let textures = match &sky.textures {
        Some((celestials, clouds, bind_group))
            if *celestials == extracted.celestials && *clouds == extracted.clouds =>
        {
            bind_group.clone()
        }
        _ => {
            let (Some(celestials), Some(clouds)) = (
                images.get(extracted.celestials),
                images.get(extracted.clouds),
            ) else {
                return;
            };
            let bind_group = device.create_bind_group(
                "sky textures",
                &pipeline_cache.get_bind_group_layout(&sky.texture_layout),
                &BindGroupEntries::sequential((
                    &celestials.texture_view,
                    &celestials.sampler,
                    &clouds.texture_view,
                )),
            );
            sky.textures = Some((extracted.celestials, extracted.clouds, bind_group.clone()));
            bind_group
        }
    };
    let view = match &sky.view {
        Some((id, bind_group)) if *id == view_buffer.id() => bind_group.clone(),
        _ => {
            let bind_group = device.create_bind_group(
                "sky view",
                &pipeline_cache.get_bind_group_layout(&sky.view_layout),
                &BindGroupEntries::sequential((
                    BufferBinding {
                        buffer: view_buffer,
                        offset: 0,
                        size: Some(ViewUniform::min_size()),
                    },
                    sky.uniform.as_entire_buffer_binding(),
                )),
            );
            sky.view = Some((view_buffer.id(), bind_group.clone()));
            bind_group
        }
    };
    commands.insert_resource(SkyBindGroups { view, textures });
}

#[derive(SystemParam)]
pub(crate) struct CloudDraw<'w> {
    sky: Option<Res<'w, Sky>>,
    extracted: Option<Res<'w, ExtractedSky>>,
    binds: Option<Res<'w, SkyBindGroups>>,
}

impl CloudDraw<'_> {
    pub fn draw<'pass>(
        &'pass self,
        pass: &mut TrackedRenderPass<'pass>,
        view_offset: u32,
        cache: &'pass PipelineCache,
    ) {
        let (Some(sky), Some(extracted), Some(binds)) = (
            self.sky.as_deref(),
            self.extracted.as_deref(),
            self.binds.as_deref(),
        ) else {
            return;
        };
        let Some((_, pipelines)) = sky.pipelines.as_ref() else {
            return;
        };
        pass.set_bind_group(1, &binds.textures, &[]);
        issue(
            pass,
            CLOUDS,
            pipelines,
            extracted,
            binds,
            view_offset,
            cache,
        );
    }
}

fn issue<'pass>(
    pass: &mut TrackedRenderPass<'pass>,
    index: usize,
    pipelines: &[Option<CachedRenderPipelineId>; SKY_DRAWS.len()],
    extracted: &ExtractedSky,
    binds: &'pass SkyBindGroups,
    view_offset: u32,
    cache: &'pass PipelineCache,
) {
    let draw = &SKY_DRAWS[index];
    if !(draw.visible)(&extracted.uniform) {
        return;
    }
    let Some(pipeline) = pipelines[index].and_then(|id| cache.get_render_pipeline(id)) else {
        return;
    };
    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, &binds.view, &[view_offset]);
    pass.draw(0..draw.vertices, 0..1);
}

pub(crate) fn draw_sky(
    view: ViewQuery<(&ViewTarget, &ViewDepthTexture, &ViewUniformOffset)>,
    sky: Option<Res<Sky>>,
    extracted: Option<Res<ExtractedSky>>,
    binds: Option<Res<SkyBindGroups>>,
    pipeline_cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let (Some(sky), Some(extracted), Some(binds)) = (sky, extracted, binds) else {
        return;
    };
    let Some((_, pipelines)) = sky.pipelines.as_ref() else {
        return;
    };
    let (target, depth, view_offset) = view.into_inner();
    let color_attachments = [Some(target.get_color_attachment())];
    let depth_attachment = Some(depth.get_attachment(StoreOp::Store));
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("sky"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: depth_attachment,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_bind_group(1, &binds.textures, &[]);
    for index in (0..SKY_DRAWS.len()).filter(|&index| index != CLOUDS) {
        issue(
            &mut pass,
            index,
            pipelines,
            &extracted,
            &binds,
            view_offset.offset,
            &pipeline_cache,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_clouds_are_the_only_draw_that_writes_depth() {
        let writers: Vec<&str> = SKY_DRAWS
            .iter()
            .filter(|draw| draw.writes_depth)
            .map(|draw| draw.label)
            .collect();
        assert_eq!(writers, ["sky clouds"]);
        assert_eq!(SKY_DRAWS[CLOUDS].label, "sky clouds");
    }

    #[test]
    fn every_effect_bit_names_exactly_one_draw_in_order() {
        let effects: Vec<SkyEffects> = SKY_DRAWS.iter().map(|draw| draw.effect).collect();
        assert_eq!(
            effects,
            [
                SkyEffects::DISC,
                SkyEffects::TWILIGHT,
                SkyEffects::CELESTIAL,
                SkyEffects::STARS,
                SkyEffects::CLOUDS,
            ]
        );
        assert_eq!(
            effects
                .iter()
                .copied()
                .fold(SkyEffects::empty(), |all, bit| all | bit),
            SkyEffects::all()
        );
    }

    #[test]
    fn a_draw_is_skipped_only_while_it_would_put_nothing_on_screen() {
        let dark = SkyUniform::default();
        let lit = SkyUniform {
            angles: [0.0, 0.0, 0.0, f32::MIN_POSITIVE],
            sunrise: [0.0, 0.0, 0.0, 0.5],
            ..default()
        };
        for draw in &SKY_DRAWS {
            assert!((draw.visible)(&lit), "{}", draw.label);
            let fades = matches!(draw.effect, SkyEffects::STARS | SkyEffects::TWILIGHT);
            assert_eq!((draw.visible)(&dark), !fades, "{}", draw.label);
        }
    }

    #[test]
    fn a_skybox_less_dimension_issues_only_the_disc() {
        let issued: Vec<&str> = SKY_DRAWS
            .iter()
            .filter(|draw| SkyEffects::DISC.contains(draw.effect))
            .map(|draw| draw.label)
            .collect();
        assert_eq!(issued, ["sky disc"]);
    }
}
