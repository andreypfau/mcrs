use std::num::NonZeroU64;

use bevy::prelude::*;
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer_sized};
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;
use bevy::render::view::ViewDepthStencilTexture;

use crate::reconstruct::LIGHTING_UNIFORM_SIZE;
use crate::uniform_buffer;

/// Albedo and occlusion, normal and motion, then emissive, block light, sky light and roughness.
/// Albedo stays in the bytes vanilla shades, so it is never an sRGB format: the lighting pass
/// encodes for the target once.
pub const GBUFFER_FORMATS: [TextureFormat; 3] = [
    TextureFormat::Rgba8Unorm,
    TextureFormat::Rgba16Float,
    TextureFormat::Rgba16Float,
];

pub(crate) fn gbuffer_layout() -> BindGroupLayoutDescriptor {
    BindGroupLayoutDescriptor::new(
        "deferred gbuffer",
        &BindGroupLayoutEntries::sequential(
            ShaderStages::FRAGMENT,
            (
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d(TextureSampleType::Float { filterable: false }),
                texture_2d(TextureSampleType::Float { filterable: false }),
                uniform_buffer_sized(false, NonZeroU64::new(LIGHTING_UNIFORM_SIZE)),
                sampler(SamplerBindingType::NonFiltering),
            ),
        ),
    )
}

/// Everything the deferred path allocates. Nothing else may hold a clone of these
/// textures or the bind group, or a resize would not free the ones it replaces.
#[derive(Resource)]
pub struct DeferredFrame {
    /// Albedo and occlusion, normal and motion, then the light, in [`GBUFFER_FORMATS`].
    pub targets: [TextureView; 3],
    pub(crate) lighting: Buffer,
    /// Reads the depth, which a shader samples rather than loads (`depth.wesl`).
    nearest: Sampler,
    pub(crate) bind_group: BindGroup,
    size: UVec2,
    depth: TextureId,
}

impl DeferredFrame {
    fn new(depth: &ViewDepthStencilTexture, device: &RenderDevice, cache: &PipelineCache) -> Self {
        let size = size_of(depth);
        let targets = targets(size, device);
        let lighting = uniform_buffer("deferred lighting", LIGHTING_UNIFORM_SIZE, device);
        let nearest = nearest_sampler(device);
        let bind_group = bind_group(&targets, &lighting, &nearest, depth, device, cache);
        Self {
            targets,
            lighting,
            nearest,
            bind_group,
            size,
            depth: depth.texture().id(),
        }
    }

    fn fit(
        &mut self,
        depth: &ViewDepthStencilTexture,
        device: &RenderDevice,
        cache: &PipelineCache,
    ) {
        let size = size_of(depth);
        let rebuilt = size != self.size;
        if rebuilt {
            self.targets = targets(size, device);
            self.size = size;
        }
        if rebuilt || self.depth != depth.texture().id() {
            self.depth = depth.texture().id();
            self.bind_group = bind_group(
                &self.targets,
                &self.lighting,
                &self.nearest,
                depth,
                device,
                cache,
            );
        }
    }
}

fn size_of(depth: &ViewDepthStencilTexture) -> UVec2 {
    UVec2::new(depth.texture().width(), depth.texture().height())
}

fn targets(size: UVec2, device: &RenderDevice) -> [TextureView; 3] {
    GBUFFER_FORMATS.map(|format| {
        device
            .create_texture(&TextureDescriptor {
                label: Some("gbuffer"),
                size: Extent3d {
                    width: size.x,
                    height: size.y,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: TextureDimension::D2,
                format,
                usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::TEXTURE_BINDING,
                view_formats: &[],
            })
            .create_view(&TextureViewDescriptor::default())
    })
}

/// Nearest in every direction, which is what a sampler reading depth has to be.
pub(crate) fn nearest_sampler(device: &RenderDevice) -> Sampler {
    device.create_sampler(&SamplerDescriptor {
        label: Some("depth"),
        ..SamplerDescriptor::default()
    })
}

fn bind_group(
    targets: &[TextureView; 3],
    lighting: &Buffer,
    nearest: &Sampler,
    depth: &ViewDepthStencilTexture,
    device: &RenderDevice,
    cache: &PipelineCache,
) -> BindGroup {
    let depth_view = depth.texture().create_view(&TextureViewDescriptor {
        label: Some("gbuffer depth"),
        aspect: TextureAspect::DepthOnly,
        ..default()
    });
    device.create_bind_group(
        "deferred gbuffer",
        &cache.get_bind_group_layout(&gbuffer_layout()),
        &BindGroupEntries::sequential((
            &targets[0],
            &targets[1],
            &targets[2],
            &depth_view,
            lighting.as_entire_binding(),
            nearest,
        )),
    )
}

pub(crate) fn fit_deferred_frame(
    mut commands: Commands,
    frame: Option<ResMut<DeferredFrame>>,
    views: Query<&ViewDepthStencilTexture, With<Camera3d>>,
    device: Res<RenderDevice>,
    cache: Res<PipelineCache>,
) {
    let Some(depth) = views.iter().next() else {
        return;
    };
    match frame {
        Some(mut frame) => frame.fit(depth, &device, &cache),
        None => commands.insert_resource(DeferredFrame::new(depth, &device, &cache)),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_gbuffer_fits_the_attachment_budget() {
        let bytes: u32 = GBUFFER_FORMATS
            .iter()
            .map(|format| {
                format
                    .target_pixel_byte_cost()
                    .expect("colour formats have a cost")
            })
            .sum();
        assert!(bytes <= WgpuLimits::default().max_color_attachment_bytes_per_sample);
    }
}
