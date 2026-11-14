use std::num::NonZeroU64;

use bevy::core_pipeline::core_3d::CORE_3D_DEPTH_FORMAT;
use bevy::prelude::*;
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer_sized};
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;
use bevy::render::view::ViewDepthStencilTexture;

use crate::gbuffer::{WORLD_IMAGE_FORMAT, attachment, depth_view};
use crate::reconstruct::LIGHTING_UNIFORM_SIZE;

/// `cl` and `ca` of the clouds, blended as the target blends.
pub(crate) const CLOUD_IMAGE_FORMAT: TextureFormat = TextureFormat::Rgba16Float;

const UNFILTERED: TextureSampleType = TextureSampleType::Float { filterable: false };

/// The sky image, the cloud image and the cloud depth that `transparency.wesl` reads.
pub(crate) fn transparency_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "improved transparency",
        &BindGroupLayoutEntries::with_indices(
            ShaderStages::FRAGMENT,
            (
                (6, texture_2d(UNFILTERED)),
                (7, texture_2d(UNFILTERED)),
                (8, texture_2d(UNFILTERED)),
                (9, sampler(SamplerBindingType::NonFiltering)),
            ),
        ),
    )
}

/// The gbuffer of `gbuffer_layout` and the images of `transparency_layout` in one group, as the
/// lighting pass has no group left for a fifth.
pub(crate) fn transparent_gbuffer_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "deferred gbuffer with improved transparency",
        &BindGroupLayoutEntries::with_indices(
            ShaderStages::FRAGMENT,
            (
                (0, texture_2d(UNFILTERED)),
                (1, texture_2d(UNFILTERED)),
                (2, texture_2d(UNFILTERED)),
                (3, texture_2d(UNFILTERED)),
                (
                    4,
                    uniform_buffer_sized(false, NonZeroU64::new(LIGHTING_UNIFORM_SIZE)),
                ),
                (5, sampler(SamplerBindingType::NonFiltering)),
                (6, texture_2d(UNFILTERED)),
                (7, texture_2d(UNFILTERED)),
                (8, texture_2d(UNFILTERED)),
                (9, sampler(SamplerBindingType::NonFiltering)),
            ),
        ),
    )
}

/// What a frame with Improved Transparency draws beside the gbuffer, at the size of the view.
/// Built when the option turns on or the view changes, dropped when it turns off.
pub(crate) struct TransparencyTargets {
    /// In image values, as the world image, which the sky pipelines draw to.
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
        let lighting = device.create_bind_group(
            "deferred gbuffer with improved transparency",
            &cache.get_bind_group_layout(&transparent_gbuffer_layout()),
            &BindGroupEntries::with_indices((
                (0, &gbuffer[0]),
                (1, &gbuffer[1]),
                (2, &gbuffer[2]),
                (3, &main_depth),
                (4, lighting.as_entire_binding()),
                (5, nearest),
                (6, &sky),
                (7, &clouds),
                (8, &cloud_depth_read),
                (9, nearest),
            )),
        );
        let forward = device.create_bind_group(
            "improved transparency",
            &cache.get_bind_group_layout(&transparency_layout()),
            &BindGroupEntries::with_indices((
                (6, &sky),
                (7, &clouds),
                (8, &cloud_depth_read),
                (9, nearest),
            )),
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
