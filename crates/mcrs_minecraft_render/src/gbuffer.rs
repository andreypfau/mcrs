use std::num::NonZeroU64;

use bevy::prelude::*;
use bevy::render::render_resource::binding_types::{sampler, texture_2d, uniform_buffer_sized};
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;
use bevy::render::view::ViewDepthStencilTexture;

use crate::ImprovedTransparency;
use crate::present::present_bind_group;
use crate::reconstruct::LIGHTING_UNIFORM_SIZE;
use crate::transparency_targets::TransparencyTargets;
use crate::uniform_buffer;

/// Albedo and occlusion, normal and motion, then emissive, block light, sky light and the mark of
/// a terrain pixel.
/// Albedo stays in the bytes vanilla shades, so it is never an sRGB format: the lighting pass
/// encodes for the target once.
pub const GBUFFER_FORMATS: [TextureFormat; 3] = [
    TextureFormat::Rgba8Unorm,
    TextureFormat::Rgba16Float,
    TextureFormat::Rgba16Float,
];

/// The world is drawn and blended in image values, and the present pass encodes it once for the
/// view target.
pub(crate) const WORLD_IMAGE_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;

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
    /// What the world passes draw into, in [`WORLD_IMAGE_FORMAT`].
    pub(crate) world: TextureView,
    pub(crate) present: BindGroup,
    pub(crate) lighting: Buffer,
    /// Reads the depth, which a shader samples rather than loads (`depth.wesl`).
    nearest: Sampler,
    pub(crate) bind_group: BindGroup,
    /// Present while Improved Transparency is on.
    pub(crate) transparency: Option<TransparencyTargets>,
    size: UVec2,
    depth: TextureId,
}

impl DeferredFrame {
    fn new(depth: &ViewDepthStencilTexture, device: &RenderDevice, cache: &PipelineCache) -> Self {
        let size = size_of(depth);
        let targets = targets(size, device);
        let world = world_image(size, device);
        let present = present_bind_group(&world, device, cache);
        let lighting = uniform_buffer("deferred lighting", LIGHTING_UNIFORM_SIZE, device);
        let nearest = nearest_sampler(device);
        let bind_group = bind_group(&targets, &lighting, &nearest, depth, device, cache);
        Self {
            targets,
            world,
            present,
            lighting,
            nearest,
            bind_group,
            transparency: None,
            size,
            depth: depth.texture().id(),
        }
    }

    fn fit(
        &mut self,
        depth: &ViewDepthStencilTexture,
        improved: bool,
        device: &RenderDevice,
        cache: &PipelineCache,
    ) {
        let size = size_of(depth);
        let rebuilt = size != self.size;
        if rebuilt {
            self.targets = targets(size, device);
            self.world = world_image(size, device);
            self.present = present_bind_group(&self.world, device, cache);
            self.size = size;
        }
        let rebound = rebuilt || self.depth != depth.texture().id();
        if rebound {
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
        if !improved {
            self.transparency = None;
            return;
        }
        if rebound || self.transparency.is_none() {
            self.transparency = Some(TransparencyTargets::new(
                &self.targets,
                &self.lighting,
                &self.nearest,
                depth,
                device,
                cache,
            ));
        }
    }
}

fn size_of(depth: &ViewDepthStencilTexture) -> UVec2 {
    UVec2::new(depth.texture().width(), depth.texture().height())
}

fn targets(size: UVec2, device: &RenderDevice) -> [TextureView; 3] {
    GBUFFER_FORMATS
        .map(|format| attachment("gbuffer", size, format, device).create_view(&default()))
}

fn world_image(size: UVec2, device: &RenderDevice) -> TextureView {
    attachment("world image", size, WORLD_IMAGE_FORMAT, device).create_view(&default())
}

/// A texture that a pass draws into and a later pass reads.
pub(crate) fn attachment(
    label: &'static str,
    size: UVec2,
    format: TextureFormat,
    device: &RenderDevice,
) -> Texture {
    device.create_texture(&TextureDescriptor {
        label: Some(label),
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
}

/// The depth of `depth` read as a float texture, which is how `depth.wesl` takes it.
pub(crate) fn depth_view(depth: &Texture) -> TextureView {
    depth.create_view(&TextureViewDescriptor {
        label: Some("gbuffer depth"),
        aspect: TextureAspect::DepthOnly,
        ..default()
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
    let depth_view = depth_view(depth.texture());
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
    improved: Option<Res<ImprovedTransparency>>,
    device: Res<RenderDevice>,
    cache: Res<PipelineCache>,
) {
    let Some(depth) = views.iter().next() else {
        return;
    };
    let improved = improved.is_some_and(|improved| improved.0);
    match frame {
        Some(mut frame) => frame.fit(depth, improved, &device, &cache),
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
