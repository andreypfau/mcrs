use bevy::prelude::*;
use bevy::render::render_resource::binding_types::{
    texture_2d, texture_depth_2d, texture_storage_2d,
};
use bevy::render::render_resource::*;
use bevy::render::renderer::RenderDevice;
use bevy::render::view::ViewDepthTexture;

use crate::probe::{self, GpuTimings, Queries};

const THREADS: u32 = 8;

/// The last frame's depth as a pyramid, each level the farthest depth of the level below, so
/// the cull can ask whether a box is behind everything drawn over its footprint.
pub(super) struct Hiz {
    texture: Texture,
    pub view: TextureView,
    levels: Vec<TextureView>,
    binds: Vec<BindGroup>,
    depth: Option<TextureId>,
    from_depth_layout: BindGroupLayoutDescriptor,
    down_layout: BindGroupLayoutDescriptor,
    from_depth: CachedComputePipelineId,
    down: CachedComputePipelineId,
}

impl Hiz {
    pub fn new(
        device: &RenderDevice,
        asset_server: &AssetServer,
        pipeline_cache: &PipelineCache,
    ) -> Self {
        let from_depth_layout = BindGroupLayoutDescriptor::new(
            "hiz from depth",
            &BindGroupLayoutEntries::sequential(
                ShaderStages::COMPUTE,
                (
                    texture_depth_2d(),
                    texture_storage_2d(TextureFormat::R32Float, StorageTextureAccess::WriteOnly),
                ),
            ),
        );
        let down_layout = BindGroupLayoutDescriptor::new(
            "hiz down",
            &BindGroupLayoutEntries::with_indices(
                ShaderStages::COMPUTE,
                (
                    (
                        1,
                        texture_storage_2d(
                            TextureFormat::R32Float,
                            StorageTextureAccess::WriteOnly,
                        ),
                    ),
                    (
                        2,
                        texture_2d(TextureSampleType::Float { filterable: false }),
                    ),
                ),
            ),
        );
        let shader =
            asset_server.load("embedded://mcrs_minecraft_client/render/shaders/core/hiz.wgsl");
        let pipeline = |label: &str, layout: &BindGroupLayoutDescriptor, entry: &str| {
            pipeline_cache.queue_compute_pipeline(ComputePipelineDescriptor {
                label: Some(label.to_owned().into()),
                layout: vec![layout.clone()],
                shader: shader.clone(),
                entry_point: Some(entry.to_owned().into()),
                ..default()
            })
        };
        let from_depth = pipeline("hiz from depth", &from_depth_layout, "hiz_from_depth");
        let down = pipeline("hiz down", &down_layout, "hiz_down");
        let (texture, view, levels) = pyramid(UVec2::ONE, device);
        Self {
            texture,
            view,
            levels,
            binds: Vec::new(),
            depth: None,
            from_depth_layout,
            down_layout,
            from_depth,
            down,
        }
    }

    pub fn levels(&self) -> u32 {
        self.levels.len() as u32
    }

    /// Fits the pyramid to the depth texture, answering whether the pyramid was rebuilt and so
    /// has to be bound again.
    pub fn fit(
        &mut self,
        depth: &ViewDepthTexture,
        device: &RenderDevice,
        pipeline_cache: &PipelineCache,
    ) -> bool {
        let wanted =
            UVec2::new(depth.texture.width(), depth.texture.height()).max(UVec2::splat(2)) / 2;
        let size = UVec2::new(self.texture.width(), self.texture.height());
        let rebuilt = wanted != size;
        if rebuilt {
            let (texture, view, levels) = pyramid(wanted, device);
            self.texture = texture;
            self.view = view;
            self.levels = levels;
        }
        if rebuilt || self.depth != Some(depth.texture.id()) {
            self.depth = Some(depth.texture.id());
            let depth_view = depth.texture.create_view(&TextureViewDescriptor {
                label: Some("hiz depth"),
                aspect: TextureAspect::DepthOnly,
                ..default()
            });
            self.binds.clear();
            self.binds.push(device.create_bind_group(
                "hiz from depth",
                &pipeline_cache.get_bind_group_layout(&self.from_depth_layout),
                &BindGroupEntries::sequential((&depth_view, &self.levels[0])),
            ));
            for level in 1..self.levels.len() {
                self.binds.push(device.create_bind_group(
                    "hiz down",
                    &pipeline_cache.get_bind_group_layout(&self.down_layout),
                    &BindGroupEntries::with_indices((
                        (1, &self.levels[level]),
                        (2, &self.levels[level - 1]),
                    )),
                ));
            }
        }
        rebuilt
    }

    pub fn build(
        &self,
        pipeline_cache: &PipelineCache,
        queries: Option<&Queries>,
        timings: &GpuTimings,
        encoder: &mut CommandEncoder,
    ) {
        let (Some(from_depth), Some(down)) = (
            pipeline_cache.get_compute_pipeline(self.from_depth),
            pipeline_cache.get_compute_pipeline(self.down),
        ) else {
            return;
        };
        let timestamps = queries.map(|q| q.compute(probe::HIZ, timings));
        let mut pass = encoder.begin_compute_pass(&ComputePassDescriptor {
            label: Some("hiz"),
            timestamp_writes: timestamps,
        });
        let mut size = UVec2::new(self.texture.width(), self.texture.height());
        for (level, bind) in self.binds.iter().enumerate() {
            pass.set_pipeline(if level == 0 { from_depth } else { down });
            pass.set_bind_group(0, bind, &[]);
            pass.dispatch_workgroups(size.x.div_ceil(THREADS), size.y.div_ceil(THREADS), 1);
            size = (size / 2).max(UVec2::ONE);
        }
    }
}

fn pyramid(size: UVec2, device: &RenderDevice) -> (Texture, TextureView, Vec<TextureView>) {
    let mip_level_count = size.max_element().ilog2() + 1;
    let texture = device.create_texture(&TextureDescriptor {
        label: Some("hiz"),
        size: Extent3d {
            width: size.x,
            height: size.y,
            depth_or_array_layers: 1,
        },
        mip_level_count,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::R32Float,
        usage: TextureUsages::STORAGE_BINDING | TextureUsages::TEXTURE_BINDING,
        view_formats: &[],
    });
    let view = texture.create_view(&TextureViewDescriptor {
        label: Some("hiz"),
        ..default()
    });
    let levels = (0..mip_level_count)
        .map(|level| {
            texture.create_view(&TextureViewDescriptor {
                label: Some("hiz level"),
                base_mip_level: level,
                mip_level_count: Some(1),
                ..default()
            })
        })
        .collect();
    (texture, view, levels)
}
