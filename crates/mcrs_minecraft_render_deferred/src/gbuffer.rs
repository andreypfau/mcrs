use bevy::prelude::*;
use bevy::render::render_resource::binding_types::{texture_2d, texture_depth_2d};
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;
use bevy::render::view::ViewDepthTexture;
use mcrs_minecraft_render::RenderPath;

/// Albedo and occlusion, normal and motion, then emissive, block light, sky light and roughness.
/// Albedo stays in the bytes vanilla shades, so it is never an sRGB format: the lighting pass
/// encodes for the target once.
pub(crate) const GBUFFER_FORMATS: [TextureFormat; 3] = [
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
                texture_depth_2d(),
            ),
        ),
    )
}

/// Everything the deferred path allocates per pixel. Nothing else may hold a clone of these
/// textures or the bind group, or switching back to classic would not free them.
#[derive(Resource)]
pub(crate) struct DeferredFrame {
    pub targets: [TextureView; 3],
    pub bind_group: BindGroup,
    size: UVec2,
    depth: TextureId,
}

impl DeferredFrame {
    fn new(depth: &ViewDepthTexture, device: &RenderDevice, cache: &PipelineCache) -> Self {
        let size = size_of(depth);
        let targets = targets(size, device);
        let bind_group = bind_group(&targets, depth, device, cache);
        Self {
            targets,
            bind_group,
            size,
            depth: depth.texture.id(),
        }
    }

    fn fit(&mut self, depth: &ViewDepthTexture, device: &RenderDevice, cache: &PipelineCache) {
        let size = size_of(depth);
        let rebuilt = size != self.size;
        if rebuilt {
            self.targets = targets(size, device);
            self.size = size;
        }
        if rebuilt || self.depth != depth.texture.id() {
            self.depth = depth.texture.id();
            self.bind_group = bind_group(&self.targets, depth, device, cache);
        }
    }
}

fn size_of(depth: &ViewDepthTexture) -> UVec2 {
    UVec2::new(depth.texture.width(), depth.texture.height())
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

fn bind_group(
    targets: &[TextureView; 3],
    depth: &ViewDepthTexture,
    device: &RenderDevice,
    cache: &PipelineCache,
) -> BindGroup {
    let depth_view = depth.texture.create_view(&TextureViewDescriptor {
        label: Some("gbuffer depth"),
        aspect: TextureAspect::DepthOnly,
        ..default()
    });
    device.create_bind_group(
        "deferred gbuffer",
        &cache.get_bind_group_layout(&gbuffer_layout()),
        &BindGroupEntries::sequential((&targets[0], &targets[1], &targets[2], &depth_view)),
    )
}

pub(crate) fn fit_deferred_frame(
    mut commands: Commands,
    requested: Res<RenderPath>,
    frame: Option<ResMut<DeferredFrame>>,
    views: Query<&ViewDepthTexture>,
    device: Res<RenderDevice>,
    cache: Res<PipelineCache>,
) {
    if *requested == RenderPath::Classic {
        if frame.is_some() {
            commands.remove_resource::<DeferredFrame>();
        }
        return;
    }
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
        assert_eq!(bytes, 24);
        assert!(bytes <= WgpuLimits::default().max_color_attachment_bytes_per_sample);
    }
}
