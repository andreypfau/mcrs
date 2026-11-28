use bevy::asset::embedded_asset;
use bevy::core_pipeline::FullscreenShader;
use bevy::core_pipeline::core_3d::main_transparent_pass_3d;
use bevy::core_pipeline::schedule::{Core3d, Core3dSystems};
use bevy::math::{Mat3, Mat4};
use bevy::prelude::*;
use bevy::render::extract_resource::{ExtractResource, ExtractResourcePlugin};
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::binding_types::{
    sampler, texture_2d, texture_cube, uniform_buffer_sized,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, RenderDevice, RenderQueue, ViewQuery};
use bevy::render::texture::GpuImage;
use bevy::render::view::{ExtractedView, Msaa, ViewTarget};
use bevy::render::{Render, RenderApp, RenderStartup, RenderSystems};

use crate::WorldPass;
use crate::pipeline::descriptor;
use crate::uniform_buffer;

const FIELD_OF_VIEW_DEGREES: f32 = 85.0;
const PITCH_DOWN_DEGREES: f32 = 10.0;
const BLUR_PAIRS: usize = 3;

pub const CUBE_FACES: [usize; 6] = [1, 3, 4, 5, 0, 2];

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum PanoramaDraw {
    #[default]
    Hidden,
    Plain,
    Blurred,
}

#[derive(Resource, Clone, Copy, Debug, Default, PartialEq, ExtractResource)]
#[extract_app(RenderApp)]
pub struct PanoramaView {
    pub spin_degrees: f32,
    pub draw: PanoramaDraw,
}

#[derive(Resource, Clone, Debug, ExtractResource)]
#[extract_app(RenderApp)]
pub struct PanoramaImages {
    pub cube: Handle<Image>,
    pub overlay: Handle<Image>,
}

#[derive(SystemSet, Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct PanoramaPass;

pub struct PanoramaPlugin;

impl Plugin for PanoramaPlugin {
    fn build(&self, app: &mut App) {
        embedded_asset!(app, "shaders/panorama.wesl");
        bevy::shader::load_shader_library!(app, "shaders/include/finish.wesl");
        app.init_resource::<PanoramaView>()
            .add_plugins(ExtractResourcePlugin::<PanoramaView>::default())
            .add_plugins(ExtractResourcePlugin::<PanoramaImages>::default());

        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app
            .init_resource::<SpecializedRenderPipelines<Panorama>>()
            .add_systems(RenderStartup, init_panorama)
            .add_systems(
                Render,
                (
                    (queue_pipelines, write_ray).in_set(RenderSystems::Prepare),
                    prepare_bind_group.in_set(RenderSystems::PrepareBindGroups),
                )
                    .run_if(drawn),
            )
            .configure_sets(
                Core3d,
                PanoramaPass
                    .in_set(Core3dSystems::MainPass)
                    .after(WorldPass::Forward)
                    .before(main_transparent_pass_3d),
            )
            .add_systems(
                Core3d,
                (
                    draw_panorama.run_if(drawn),
                    blur_view_target.run_if(blurred),
                )
                    .chain()
                    .in_set(PanoramaPass),
            );
    }
}

fn ray_from_ndc(spin_degrees: f32, aspect: f32) -> Mat3 {
    let half_height = (FIELD_OF_VIEW_DEGREES / 2.0).to_radians().tan();
    let look = Mat3::from_rotation_y(-spin_degrees.to_radians())
        * Mat3::from_rotation_x(-PITCH_DOWN_DEGREES.to_radians());
    look * Mat3::from_cols(
        Vec3::X * half_height * aspect,
        Vec3::Y * half_height,
        Vec3::NEG_Z,
    )
}

fn drawn(view: Option<Res<PanoramaView>>, images: Option<Res<PanoramaImages>>) -> bool {
    images.is_some() && view.is_some_and(|view| view.draw != PanoramaDraw::Hidden)
}

fn blurred(view: Option<Res<PanoramaView>>, images: Option<Res<PanoramaImages>>) -> bool {
    images.is_some() && view.is_some_and(|view| view.draw == PanoramaDraw::Blurred)
}

#[derive(Resource)]
struct Panorama {
    layout: BindGroupLayoutDescriptor,
    blur_layout: BindGroupLayoutDescriptor,
    shader: Handle<Shader>,
    fullscreen: VertexState,
    ray: Buffer,
    sampler: Sampler,
    bind_group: Option<(AssetId<Image>, AssetId<Image>, BindGroup)>,
}

fn init_panorama(
    mut commands: Commands,
    asset_server: Res<AssetServer>,
    device: Res<RenderDevice>,
    fullscreen: Res<FullscreenShader>,
) {
    commands.insert_resource(Panorama {
        layout: BindGroupLayoutDescriptor::new(
            "panorama",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::FRAGMENT,
                (
                    uniform_buffer_sized(false, BufferSize::new(size_of::<Mat4>() as u64)),
                    texture_cube(TextureSampleType::Float { filterable: true }),
                    texture_2d(TextureSampleType::Float { filterable: true }),
                    sampler(SamplerBindingType::Filtering),
                ),
            ),
        ),
        blur_layout: BindGroupLayoutDescriptor::new(
            "panorama blur",
            &BindGroupLayoutEntries::with_indices(
                ShaderStages::FRAGMENT,
                ((
                    4,
                    texture_2d(TextureSampleType::Float { filterable: false }),
                ),),
            ),
        ),
        shader: asset_server.load("embedded://mcrs_minecraft_render/shaders/panorama.wesl"),
        fullscreen: fullscreen.to_vertex_state(),
        ray: uniform_buffer("panorama", size_of::<Mat4>() as u64, &device),
        sampler: device.create_sampler(&SamplerDescriptor {
            label: Some("panorama"),
            mag_filter: FilterMode::Linear,
            min_filter: FilterMode::Linear,
            ..default()
        }),
        bind_group: None,
    });
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
enum PanoramaEntry {
    Panorama,
    BlurHorizontal,
    BlurVertical,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
struct PanoramaKey {
    entry: PanoramaEntry,
    format: TextureFormat,
    samples: u32,
}

impl SpecializedRenderPipeline for Panorama {
    type Key = PanoramaKey;

    fn specialize(&self, key: PanoramaKey) -> RenderPipelineDescriptor {
        let (layout, entry) = match key.entry {
            PanoramaEntry::Panorama => (&self.layout, "panorama"),
            PanoramaEntry::BlurHorizontal => (&self.blur_layout, "blur_horizontal"),
            PanoramaEntry::BlurVertical => (&self.blur_layout, "blur_vertical"),
        };
        let mut descriptor = RenderPipelineDescriptor {
            vertex: self.fullscreen.clone(),
            ..descriptor(
                format!("panorama {entry}"),
                vec![layout.clone()],
                &self.shader,
                String::new(),
                entry.into(),
                key.format,
                None,
            )
        };
        descriptor.multisample.count = key.samples;
        descriptor
    }
}

#[derive(Component, Clone, Copy, PartialEq)]
struct ViewPanorama {
    pipeline: CachedRenderPipelineId,
    blur: [CachedRenderPipelineId; 2],
}

fn queue_pipelines(
    mut commands: Commands,
    panorama: Option<Res<Panorama>>,
    mut pipelines: ResMut<SpecializedRenderPipelines<Panorama>>,
    mut views: Query<(Entity, &ExtractedView, &Msaa, Option<&mut ViewPanorama>), With<Camera3d>>,
    cache: Res<PipelineCache>,
) {
    let Some(panorama) = panorama else {
        return;
    };
    for (entity, view, msaa, current) in &mut views {
        let mut specialize = |entry, samples| {
            pipelines.specialize(
                &cache,
                &panorama,
                PanoramaKey {
                    entry,
                    format: view.target_format,
                    samples,
                },
            )
        };
        let wanted = ViewPanorama {
            pipeline: specialize(PanoramaEntry::Panorama, msaa.samples()),
            blur: [
                specialize(PanoramaEntry::BlurHorizontal, 1),
                specialize(PanoramaEntry::BlurVertical, 1),
            ],
        };
        match current {
            Some(mut current) => {
                current.set_if_neq(wanted);
            }
            None => {
                commands.entity(entity).insert(wanted);
            }
        }
    }
}

fn write_ray(
    panorama: Option<Res<Panorama>>,
    view: Res<PanoramaView>,
    views: Query<&ExtractedView, With<Camera3d>>,
    queue: Res<RenderQueue>,
) {
    let (Some(panorama), Some(target)) = (panorama, views.iter().next()) else {
        return;
    };
    let [.., width, height] = target.viewport.to_array();
    if height == 0 {
        return;
    }
    let ray = Mat4::from_mat3(ray_from_ndc(
        view.spin_degrees,
        width as f32 / height as f32,
    ));
    queue.write_buffer(&panorama.ray, 0, bytemuck::bytes_of(&ray));
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::missed_set_if_neq,
        reason = "the system returns early while the cached bind group matches the images, so each write is a change"
    )
)]
fn prepare_bind_group(
    panorama: Option<ResMut<Panorama>>,
    images: Res<PanoramaImages>,
    gpu_images: Res<RenderAssets<GpuImage>>,
    device: Res<RenderDevice>,
    cache: Res<PipelineCache>,
) {
    let Some(mut panorama) = panorama else {
        return;
    };
    let (cube, overlay) = (images.cube.id(), images.overlay.id());
    if panorama
        .bind_group
        .as_ref()
        .is_some_and(|(c, o, _)| *c == cube && *o == overlay)
    {
        return;
    }
    let (Some(cube_image), Some(overlay_image)) = (gpu_images.get(cube), gpu_images.get(overlay))
    else {
        return;
    };
    let bind_group = device.create_bind_group(
        "panorama",
        &cache.get_bind_group_layout(&panorama.layout),
        &BindGroupEntries::sequential((
            panorama.ray.as_entire_binding(),
            &cube_image.texture_view,
            &overlay_image.texture_view,
            &panorama.sampler,
        )),
    );
    panorama.bind_group = Some((cube, overlay, bind_group));
}

fn draw_panorama(
    view: ViewQuery<(&ViewTarget, &ViewPanorama)>,
    panorama: Option<Res<Panorama>>,
    cache: Res<PipelineCache>,
    mut ctx: RenderContext,
) {
    let Some(panorama) = panorama.as_deref() else {
        return;
    };
    let (target, pipelines) = view.into_inner();
    let (Some(pipeline), Some((.., bind_group))) = (
        cache.get_render_pipeline(pipelines.pipeline),
        panorama.bind_group.as_ref(),
    ) else {
        return;
    };
    let color_attachments = [Some(target.get_color_attachment())];
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("panorama"),
        color_attachments: &color_attachments,
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, bind_group, &[]);
    pass.draw(0..3, 0..1);
}

fn blur_view_target(
    view: ViewQuery<(&ViewTarget, &ViewPanorama)>,
    panorama: Option<Res<Panorama>>,
    cache: Res<PipelineCache>,
    device: Res<RenderDevice>,
    mut sources: Local<Vec<(TextureViewId, BindGroup)>>,
    mut ctx: RenderContext,
) {
    let Some(panorama) = panorama.as_deref() else {
        return;
    };
    let (target, pipelines) = view.into_inner();
    let [Some(horizontal), Some(vertical)] = pipelines.blur.map(|id| cache.get_render_pipeline(id))
    else {
        return;
    };
    for pipeline in (0..BLUR_PAIRS).flat_map(|_| [horizontal, vertical]) {
        let write = target.post_process_write();
        let bind_group = match sources.iter().find(|(id, _)| *id == write.source.id()) {
            Some((_, bind_group)) => bind_group.clone(),
            None => {
                if sources.len() >= 2 {
                    sources.clear();
                }
                let bind_group = device.create_bind_group(
                    "panorama blur",
                    &cache.get_bind_group_layout(&panorama.blur_layout),
                    &BindGroupEntries::with_indices(((4, write.source),)),
                );
                sources.push((write.source.id(), bind_group.clone()));
                bind_group
            }
        };
        let color_attachments = [Some(RenderPassColorAttachment {
            view: write.destination,
            depth_slice: None,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Clear(default()),
                store: StoreOp::Store,
            },
        })];
        let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
            label: Some("panorama blur"),
            color_attachments: &color_attachments,
            depth_stencil_attachment: None,
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_render_pipeline(pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.draw(0..3, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn bearing(ray: Vec3) -> (f32, f32) {
        let ray = ray.normalize();
        (ray.y.asin().to_degrees(), ray.x.atan2(-ray.z).to_degrees())
    }

    #[test]
    fn the_projection_looks_ten_degrees_down_over_85_degrees_and_turns_right_with_the_spin() {
        let cases = [
            (0.0, [0.0, 0.0], (-10.0, 0.0)),
            (0.0, [0.0, 1.0], (32.5, 0.0)),
            (0.0, [0.0, -1.0], (-52.5, 0.0)),
            (90.0, [0.0, 0.0], (-10.0, 90.0)),
            (270.0, [0.0, 0.0], (-10.0, -90.0)),
            (30.0, [0.0, 1.0], (32.5, 30.0)),
        ];
        for (spin, [x, y], expected) in cases {
            let (elevation, azimuth) = bearing(ray_from_ndc(spin, 1.5) * Vec3::new(x, y, 1.0));
            assert!(
                (elevation - expected.0).abs() < 1e-3 && (azimuth - expected.1).abs() < 1e-3,
                "spin {spin} at {x}, {y}: {elevation}, {azimuth}, expected {expected:?}"
            );
        }
        let wide = ray_from_ndc(0.0, 16.0 / 9.0);
        let half_width = (wide * Vec3::Z).angle_between(wide * Vec3::new(1.0, 0.0, 1.0));
        assert!(
            (half_width.to_degrees() - 58.456).abs() < 1e-3,
            "{half_width}"
        );
    }
}
