use bevy::core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy::prelude::*;
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;
use bevy::render::view::ViewDepthStencilTexture;

use crate::gbuffer::{WORLD_IMAGE_FORMAT, attachment, depth_view};
use crate::shared_bindings::{transparency, transparent_gbuffer};

pub(crate) const CLOUD_IMAGE_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

pub(crate) struct TransparencyTargets {
    pub sky: TextureView,
    pub clouds: TextureView,
    pub cloud_depth: TextureView,
    pub lighting: BindGroup,
    pub forward: BindGroup,
}

impl TransparencyTargets {
    pub(crate) fn new(
        gbuffer: &[TextureView; 3],
        lighting: &Buffer,
        nearest: &Sampler,
        depth: &ViewDepthStencilTexture,
        device: &RenderDevice,
        cache: &PipelineCache,
    ) -> Self {
        let size = UVec2::new(depth.texture().width(), depth.texture().height());
        let sky = attachment("sky image", size, WORLD_IMAGE_FORMAT, device).create_view(&default());
        let clouds =
            attachment("cloud image", size, CLOUD_IMAGE_FORMAT, device).create_view(&default());
        let cloud_depth_texture = attachment("cloud depth", size, CORE_3D_DEPTH_FORMAT, device);
        let cloud_depth = cloud_depth_texture.create_view(&default());
        let cloud_depth_read = depth_view(&cloud_depth_texture);
        let main_depth = depth_view(depth.texture());
        let [albedo_ao, normal_motion, light] = gbuffer;
        let lighting = device.create_bind_group(
            "deferred gbuffer with improved transparency",
            &cache.get_bind_group_layout(&transparent_gbuffer::layout()),
            &transparent_gbuffer::entries(transparent_gbuffer::Entries {
                albedo_ao,
                normal_motion,
                light,
                depth: &main_depth,
                reconstruction: lighting,
                nearest,
                sky_image: &sky,
                cloud_image: &clouds,
                cloud_depth: &cloud_depth_read,
                cloud_nearest: nearest,
            }),
        );
        let forward = device.create_bind_group(
            "improved transparency",
            &cache.get_bind_group_layout(&transparency::layout()),
            &transparency::entries(transparency::Entries {
                sky_image: &sky,
                cloud_image: &clouds,
                cloud_depth: &cloud_depth_read,
                cloud_nearest: nearest,
            }),
        );
        Self {
            sky,
            clouds,
            cloud_depth,
            lighting,
            forward,
        }
    }
}
