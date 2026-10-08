use bevy::camera::{CameraUpdateSystems, ImageRenderTarget, NormalizedRenderTarget, RenderTarget};
use bevy::core_pipeline::blit::{BlitPipeline, BlitPipelineKey};
use bevy::core_pipeline::core_2d::main_transparent_pass_2d;
use bevy::core_pipeline::{Core2d, Core2dSystems};
use bevy::image::ToExtents;
use bevy::prelude::*;
use bevy::render::RenderApp;
use bevy::render::camera::ExtractedCamera;
use bevy::render::render_asset::RenderAssets;
use bevy::render::render_resource::*;
use bevy::render::renderer::{RenderContext, ViewQuery};
use bevy::render::texture::GpuImage;
use bevy::render::view::{ExtractedView, Msaa, ViewTarget};
use bevy::window::PrimaryWindow;

use crate::player::PlayerCamera;
use crate::render::gui_items::draw_gui;

#[derive(Resource, Clone, Copy)]
pub struct Raster(pub f32);

impl Default for Raster {
    fn default() -> Self {
        Self(1.0)
    }
}

pub struct RasterPlugin;

impl Plugin for RasterPlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(
            PostUpdate,
            fit_world_target
                .before(CameraUpdateSystems)
                .run_if(|raster: Res<Raster>| raster.0 < 1.0),
        );
        let Some(render_app) = app.get_sub_app_mut(RenderApp) else {
            return;
        };
        render_app.add_systems(
            Core2d,
            (present_world, draw_gui)
                .chain()
                .after(main_transparent_pass_2d)
                .in_set(Core2dSystems::MainPass),
        );
    }
}

/// The image's scale factor keeps its logical size equal to the window's, so projecting through
/// the world camera still lands in window coordinates.
fn fit_world_target(
    raster: Res<Raster>,
    window: Single<&Window, With<PrimaryWindow>>,
    camera: Single<(Entity, &mut RenderTarget), With<PlayerCamera>>,
    mut images: ResMut<Assets<Image>>,
    mut commands: Commands,
) {
    let size = (window.physical_size().as_vec2() * raster.0)
        .round()
        .as_uvec2()
        .max(UVec2::ONE);
    let (camera, mut target) = camera.into_inner();
    let handle = match &*target {
        RenderTarget::Image(image) => image.handle.clone(),
        _ => {
            commands.spawn((
                Camera2d,
                Camera {
                    order: 1,
                    ..default()
                },
                Msaa::Off,
                ChildOf(camera),
            ));
            images.add(Image::new_target_texture(
                size.x,
                size.y,
                TextureFormat::Rgba8UnormSrgb,
                None,
            ))
        }
    };
    if images
        .get(&handle)
        .is_some_and(|image| image.size() != size)
        && let Some(mut image) = images.get_mut(&handle)
    {
        image.resize(size.to_extents());
    }
    let wanted = ImageRenderTarget {
        handle,
        scale_factor: window.scale_factor() * raster.0,
    };
    if !matches!(&*target, RenderTarget::Image(image) if *image == wanted) {
        *target = RenderTarget::Image(wanted);
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "a Bevy system: each resource and query is its own parameter so the scheduler sees the access"
)]
fn present_world(
    view: ViewQuery<(&ViewTarget, &ExtractedView)>,
    worlds: Query<&ExtractedCamera, With<Camera3d>>,
    images: Res<RenderAssets<GpuImage>>,
    blit: Res<BlitPipeline>,
    mut specialized: ResMut<SpecializedRenderPipelines<BlitPipeline>>,
    pipeline_cache: Res<PipelineCache>,
    mut cached: Local<Option<(TextureViewId, BindGroup)>>,
    mut ctx: RenderContext,
) {
    let (target, extracted) = view.into_inner();
    let Some(world) = worlds.iter().find_map(|camera| match &camera.target {
        Some(NormalizedRenderTarget::Image(image)) => images.get(&image.handle),
        _ => None,
    }) else {
        return;
    };
    let pipeline = specialized.specialize(
        &pipeline_cache,
        &blit,
        BlitPipelineKey {
            target_format: extracted.target_format,
            blend_state: None,
            samples: 1,
            source_space: None,
        },
    );
    let Some(pipeline) = pipeline_cache.get_render_pipeline(pipeline) else {
        return;
    };
    let bind_group = match &mut *cached {
        Some((id, bind_group)) if *id == world.texture_view.id() => bind_group,
        cached => {
            let bind_group =
                blit.create_bind_group(ctx.render_device(), &world.texture_view, &pipeline_cache);
            &mut cached.insert((world.texture_view.id(), bind_group)).1
        }
    };
    let mut pass = ctx.begin_tracked_render_pass(RenderPassDescriptor {
        label: Some("present world"),
        color_attachments: &[Some(RenderPassColorAttachment {
            view: target.main_texture_view(),
            depth_slice: None,
            resolve_target: None,
            ops: Operations {
                load: LoadOp::Load,
                store: StoreOp::Store,
            },
        })],
        depth_stencil_attachment: None,
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    pass.set_render_pipeline(pipeline);
    pass.set_bind_group(0, bind_group, &[]);
    pass.draw(0..3, 0..1);
}
