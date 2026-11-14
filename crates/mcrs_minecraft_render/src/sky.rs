use std::num::NonZeroU64;
use std::sync::atomic::Ordering;

use bevy::asset::embedded_asset;
use bevy::core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_phase::TrackedRenderPass;
use bevy::render::render_resource::binding_types::{
    sampler, texture_2d, texture_2d_array, uniform_buffer_sized,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::texture::GpuImage;
use bevy::render::view::{ExtractedView, ViewUniform, ViewUniforms};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::Shader;
use bitflags::bitflags;

use crate::gbuffer::WORLD_IMAGE_FORMAT;
use crate::pipeline::descriptor;
use crate::transparency_targets::CLOUD_IMAGE_FORMAT;
use crate::{DEPTH_COMPARE, FrameCounts, ImprovedTransparency, uniform_buffer};

/// The per-frame GPU block. Its colours are in linear light, but `disc` and `fog`, which are image
/// values; the shader turns the linear ones into image values before the world image blends them.
#[repr(C)]
#[derive(Copy, Clone, Debug, Default, PartialEq, bytemuck::Pod, bytemuck::Zeroable)]
pub struct SkyUniform {
    pub disc: [f32; 4],
    pub sunrise: [f32; 4],
    pub angles: [f32; 4],
    pub moon: [f32; 4],
    pub fog: [f32; 4],
    pub cloud_color: [f32; 4],
    pub cloud: [f32; 4],
    pub sky_light: [f32; 4],
    pub block_light: [f32; 4],
    pub ambient: [f32; 4],
    /// The two elevations in degrees over which the occluder fades in, then 1 when the frame has
    /// a sky occluder and 0 when it has none.
    pub occluder: [f32; 4],
}

bitflags! {
    /// Which of the sky draws exist at all. A draw whose effect is off is not
    /// skipped at runtime — its pipeline is never built.
    #[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
    pub struct SkyEffects: u8 {
        const DISC = 1;
        const TWILIGHT = 1 << 1;
        const CELESTIAL = 1 << 2;
        const STARS = 1 << 3;
        const CLOUDS = 1 << 4;
        const OCCLUDER = 1 << 5;
    }
}

impl SkyEffects {
    /// Parses the comma-separated draw list a profiling run uses to leave
    /// individual passes out, e.g. `disc,twilight,celestial,stars`.
    pub fn parse(list: &str) -> Result<Self, String> {
        list.split(',')
            .map(str::trim)
            .filter(|name| !name.is_empty())
            .try_fold(Self::empty(), |effects, name| {
                Self::from_name(&name.to_ascii_uppercase())
                    .map(|bit| effects | bit)
                    .ok_or_else(|| format!("no sky draw is called {name}"))
            })
    }
}

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

const SKY_DRAWS: [SkyDraw; 6] = [
    SkyDraw {
        label: "sky disc",
        effect: SkyEffects::DISC,
        vertex: "vertex_disc",
        fragment: "fragment_disc",
        blend: None,
        vertices: 24,
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
        label: "sky occluder",
        effect: SkyEffects::OCCLUDER,
        vertex: "vertex_occluder",
        fragment: "fragment_occluder",
        blend: Some(BlendState::ALPHA_BLENDING),
        vertices: 3,
        writes_depth: false,
        visible: |_| true,
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

const CLOUDS: usize = 5;

pub struct SkyRenderPlugin;

impl Plugin for SkyRenderPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shaders/sky.wesl");

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app.add_systems(RenderStartup, init_sky).add_systems(
            Render,
            (
                (prepare_sky, write_sky).in_set(RenderSystems::Prepare),
                prepare_sky_bind_groups.in_set(RenderSystems::PrepareBindGroups),
            ),
        );
    }
}

#[derive(Resource)]
pub(crate) struct Sky {
    view_layout: BindGroupLayoutDescriptor,
    texture_layout: BindGroupLayoutDescriptor,
    shader: Handle<Shader>,
    uniform: Buffer,
    pipelines: Option<(
        SkyEffects,
        [Option<CachedRenderPipelineId>; SKY_DRAWS.len()],
    )>,
    /// The clouds into the cloud image, queued on the first frame of Improved Transparency.
    cloud_image: Option<CachedRenderPipelineId>,
    textures: Option<(AssetId<Image>, AssetId<Image>, BindGroup)>,
    view: Option<(BufferId, BindGroup)>,
}

/// Restricts the sky to a subset of its draws, so a profiling run can price
/// one pass by leaving it out.
#[derive(Resource)]
pub struct SkyDrawsOnly(pub SkyEffects);

#[derive(Resource)]
pub struct ExtractedSky {
    pub uniform: SkyUniform,
    pub effects: SkyEffects,
    pub celestials: AssetId<Image>,
    pub clouds: AssetId<Image>,
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
        shader: asset_server.load("embedded://mcrs_minecraft_render/shaders/sky.wesl"),
        uniform: uniform_buffer("sky", size_of::<SkyUniform>() as u64, &device),
        pipelines: None,
        cloud_image: None,
        textures: None,
        view: None,
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
    views: Query<&ExtractedView, With<Camera3d>>,
    improved: Option<Res<ImprovedTransparency>>,
    pipeline_cache: Res<PipelineCache>,
) {
    let (Some(mut sky), Some(extracted), Some(_)) = (sky, extracted, views.iter().next()) else {
        return;
    };
    if sky.cloud_image.is_none() && improved.is_some_and(|improved| improved.0) {
        let mut descriptor = sky_pipeline(&sky, &SKY_DRAWS[CLOUDS]);
        descriptor.label = Some("cloud image".into());
        if let Some(fragment) = descriptor.fragment.as_mut() {
            fragment.entry_point = Some("fragment_cloud_image".into());
            if let Some(Some(target)) = fragment.targets.first_mut() {
                target.format = CLOUD_IMAGE_FORMAT;
            }
        }
        sky.cloud_image = Some(pipeline_cache.queue_render_pipeline(descriptor));
    }
    if sky
        .pipelines
        .as_ref()
        .is_some_and(|(effects, _)| *effects == extracted.effects)
    {
        return;
    }
    let queued = SKY_DRAWS.each_ref().map(|draw| {
        extracted
            .effects
            .contains(draw.effect)
            .then(|| pipeline_cache.queue_render_pipeline(sky_pipeline(&sky, draw)))
    });
    info!(?extracted.effects, "queued the sky pipelines");
    sky.pipelines = Some((extracted.effects, queued));
}

fn sky_pipeline(sky: &Sky, draw: &SkyDraw) -> RenderPipelineDescriptor {
    RenderPipelineDescriptor {
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
        ..descriptor(
            draw.label.into(),
            vec![sky.view_layout.clone(), sky.texture_layout.clone()],
            &sky.shader,
            draw.vertex.into(),
            draw.fragment.into(),
            WORLD_IMAGE_FORMAT,
            draw.blend,
        )
    }
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

/// The sky opens the world pass and the clouds open the forward pass.
#[derive(SystemParam)]
pub struct SkyDraws<'w> {
    sky: Option<Res<'w, Sky>>,
    extracted: Option<Res<'w, ExtractedSky>>,
    binds: Option<Res<'w, SkyBindGroups>>,
    counts: Res<'w, FrameCounts>,
}

impl SkyDraws<'_> {
    fn ready(
        &self,
    ) -> Option<(
        &[Option<CachedRenderPipelineId>; SKY_DRAWS.len()],
        &ExtractedSky,
        &SkyBindGroups,
    )> {
        let (sky, extracted, binds) = (
            self.sky.as_deref()?,
            self.extracted.as_deref()?,
            self.binds.as_deref()?,
        );
        let (_, pipelines) = sky.pipelines.as_ref()?;
        Some((pipelines, extracted, binds))
    }

    pub fn draw_sky<'pass>(
        &'pass self,
        pass: &mut TrackedRenderPass<'pass>,
        view_offset: u32,
        cache: &'pass PipelineCache,
    ) {
        let Some((pipelines, extracted, binds)) = self.ready() else {
            self.counts.sky_draws.store(0, Ordering::Relaxed);
            return;
        };
        pass.set_bind_group(1, &binds.textures, &[]);
        let mut draws = 0;
        for (index, (draw, &pipeline)) in SKY_DRAWS.iter().zip(pipelines).enumerate() {
            if index != CLOUDS {
                draws += issue(pass, draw, pipeline, extracted, binds, view_offset, cache) as u32;
            }
        }
        self.counts.sky_draws.store(draws, Ordering::Relaxed);
    }

    /// The pipeline of the cloud image is compiled, or the dimension draws no clouds.
    pub fn cloud_image_ready(&self, cache: &PipelineCache) -> bool {
        let draws_clouds = self
            .extracted
            .as_deref()
            .is_some_and(|extracted| extracted.effects.contains(SkyEffects::CLOUDS));
        !draws_clouds
            || self
                .sky
                .as_deref()
                .and_then(|sky| sky.cloud_image)
                .is_some_and(|id| cache.get_render_pipeline(id).is_some())
    }

    /// Into the cloud image, with no other draw: the clouds are only tested against themselves.
    pub fn draw_cloud_image<'pass>(
        &'pass self,
        pass: &mut TrackedRenderPass<'pass>,
        view_offset: u32,
        cache: &'pass PipelineCache,
    ) {
        let (Some((pipelines, extracted, binds)), Some(sky)) = (self.ready(), self.sky.as_deref())
        else {
            return;
        };
        let pipeline = pipelines[CLOUDS].and(sky.cloud_image);
        pass.set_bind_group(1, &binds.textures, &[]);
        issue(
            pass,
            &SKY_DRAWS[CLOUDS],
            pipeline,
            extracted,
            binds,
            view_offset,
            cache,
        );
    }

    pub fn draw_clouds<'pass>(
        &'pass self,
        pass: &mut TrackedRenderPass<'pass>,
        view_offset: u32,
        cache: &'pass PipelineCache,
    ) -> u32 {
        let Some((pipelines, extracted, binds)) = self.ready() else {
            return 0;
        };
        pass.set_bind_group(1, &binds.textures, &[]);
        issue(
            pass,
            &SKY_DRAWS[CLOUDS],
            pipelines[CLOUDS],
            extracted,
            binds,
            view_offset,
            cache,
        ) as u32
    }
}

fn issue<'pass>(
    pass: &mut TrackedRenderPass<'pass>,
    draw: &SkyDraw,
    pipeline: Option<CachedRenderPipelineId>,
    extracted: &ExtractedSky,
    binds: &'pass SkyBindGroups,
    view_offset: u32,
    cache: &'pass PipelineCache,
) -> bool {
    if !(draw.visible)(&extracted.uniform) {
        return false;
    }
    let Some(pipeline) = pipeline.and_then(|id| cache.get_render_pipeline(id)) else {
        return false;
    };
    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, &binds.view, &[view_offset]);
    pass.draw(0..draw.vertices, 0..1);
    true
}

#[cfg(test)]
mod tests {
    use super::*;

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
}
