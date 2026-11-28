use std::sync::atomic::Ordering;

use bevy::asset::embedded_asset;
use bevy::core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy::ecs::system::SystemParam;
use bevy::prelude::*;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_phase::TrackedRenderPass;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderDevice, RenderQueue};
use bevy::render::texture::GpuImage;
use bevy::render::view::{ExtractedView, ViewUniforms};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};
use bevy::shader::Shader;
use bitflags::bitflags;

use crate::gbuffer::WORLD_IMAGE_FORMAT;
use crate::pipeline::descriptor;
use crate::shared_bindings::{sky_textures, sky_view};
use crate::shared_types::SKY_SIZE;
use crate::transparency_targets::CLOUD_IMAGE_FORMAT;
use crate::{DEPTH_COMPARE, FrameCounts, ImprovedTransparency, SkyUniform, uniform_buffer};

bitflags! {
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
                (prepare_sky, write_sky)
                    .in_set(RenderSystems::Prepare)
                    .run_if(resource_exists::<Sky>)
                    .run_if(resource_exists::<ExtractedSky>),
                prepare_sky_bind_groups
                    .in_set(RenderSystems::PrepareBindGroups)
                    .run_if(resource_exists::<Sky>)
                    .run_if(resource_exists::<ExtractedSky>),
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
    cloud_image: Option<CachedRenderPipelineId>,
    textures: Option<(AssetId<Image>, AssetId<Image>, BindGroup)>,
    view: Option<(BufferId, BindGroup)>,
}

#[derive(Resource)]
pub struct SkyDrawsOnly(pub SkyEffects);

#[derive(Resource)]
pub struct ExtractedSky {
    pub uniform: SkyUniform,
    pub effects: SkyEffects,
    pub celestials: AssetId<Image>,
    pub clouds: AssetId<Image>,
}

#[derive(Resource, PartialEq)]
pub(crate) struct SkyBindGroups {
    view: BindGroup,
    textures: BindGroup,
}

fn init_sky(mut commands: Commands, asset_server: Res<AssetServer>, device: Res<RenderDevice>) {
    commands.insert_resource(Sky {
        view_layout: sky_view::layout(),
        texture_layout: sky_textures::layout(),
        shader: asset_server.load("embedded://mcrs_minecraft_render/shaders/sky.wesl"),
        uniform: uniform_buffer("sky", SKY_SIZE, &device),
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

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::alloc_in_tick,
        reason = "the pipelines are queued only when the sky effects or the transparency mode change"
    )
)]
#[cfg_attr(mcrs_lint, mcrs::alloc_sites(5))]
#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::missed_set_if_neq,
        reason = "the system writes a cached pipeline only after it finds the entry missing or stale, so each write is a change"
    )
)]
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

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::missed_set_if_neq,
        reason = "the system writes a cached bind group only after it finds the entry missing or stale, so each write is a change"
    )
)]
fn prepare_sky_bind_groups(
    mut commands: Commands,
    binds: Option<ResMut<SkyBindGroups>>,
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
                &sky_textures::entries(sky_textures::Entries {
                    celestials: &celestials.texture_view,
                    celestial_sampler: &celestials.sampler,
                    clouds: &clouds.texture_view,
                }),
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
                &sky_view::entries(sky_view::Entries {
                    view: view_buffer,
                    sky: &sky.uniform,
                }),
            );
            sky.view = Some((view_buffer.id(), bind_group.clone()));
            bind_group
        }
    };
    let next = SkyBindGroups { view, textures };
    match binds {
        Some(mut binds) => {
            binds.set_if_neq(next);
        }
        None => commands.insert_resource(next),
    }
}

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
