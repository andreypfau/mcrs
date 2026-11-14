use bevy::prelude::*;
use bytemuck::{Pod, Zeroable};
use wgpu::{
    BindGroup, BindGroupEntry, BindGroupLayout, BindGroupLayoutEntry, BindingResource, BindingType,
    Buffer, BufferBindingType, BufferDescriptor, BufferUsages, CompareFunction, Device, Queue,
    Sampler, ShaderStages, TextureView,
};

use crate::entity_gpu::Binds;
use crate::entity_views::EntityViews;
use crate::figure_targets::FigureTargets;
use crate::figure_view::FigureFrame;
use crate::frame_uniform::FrameUniform;
use crate::gpu_buffers::FrameBuffers;
use crate::gpu_layouts::Layouts;
use crate::gpu_pipelines::FigurePipelines;

/// The full rigs that the frame buffers of the figure view hold.
pub(crate) const FIGURE_RIGS: u32 = 16;

/// The distance of the eye of the figure view from the origin of figure space, in blocks.
const EYE_DISTANCE: f32 = 1024.0;

/// The two light directions of the figure view, as `shaders/figure.wgsl` declares them.
#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
struct FigureLight {
    light0: [f32; 4],
    light1: [f32; 4],
}

/// The bind group of the light and of the lightmap that the host gives.
struct LightmapBind {
    view: TextureView,
    sampler: Sampler,
    group: BindGroup,
}

/// The GPU state that the figure view owns: its frame buffers, its bind groups and its forward
/// pipelines. The slot records, the static tables and the compute pipelines are those of the
/// world view.
pub(crate) struct FigureGpu {
    pub(crate) targets: FigureTargets,
    pub(crate) frame: FrameBuffers,
    pub(crate) binds: Binds,
    pub(crate) pipelines: FigurePipelines,
    light: Buffer,
    layout: BindGroupLayout,
    lightmap: Option<LightmapBind>,
}

impl FigureGpu {
    pub(crate) fn new(
        device: &Device,
        layouts: &Layouts,
        frame: FrameBuffers,
        binds: Binds,
        tables: DrawTables,
        targets: FigureTargets,
    ) -> Self {
        let layout = figure_layout(device);
        let pipelines = FigurePipelines::new(device, layouts, &layout, tables, targets);
        Self {
            targets,
            frame,
            binds,
            pipelines,
            light: device.create_buffer(&BufferDescriptor {
                label: Some("entity figure light"),
                size: size_of::<FigureLight>() as u64,
                usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
                mapped_at_creation: false,
            }),
            layout,
            lightmap: None,
        }
    }

    /// Builds the forward pipelines again for other targets.
    pub(crate) fn retarget(
        &mut self,
        device: &Device,
        layouts: &Layouts,
        tables: DrawTables,
        targets: FigureTargets,
    ) {
        self.pipelines = FigurePipelines::new(device, layouts, &self.layout, tables, targets);
        self.targets = targets;
    }

    /// Writes the light directions of `figure`.
    pub(crate) fn write_light(&self, queue: &Queue, figure: &FigureFrame) {
        // The fourth component of the first light tells the shader that the target encodes.
        let encodes = if self.targets.color.is_srgb() { 1.0 } else { 0.0 };
        let [light0, light1] = figure
            .lights
            .map(|light| light.normalize_or_zero().extend(encodes).to_array());
        queue.write_buffer(
            &self.light,
            0,
            bytemuck::bytes_of(&FigureLight { light0, light1 }),
        );
    }

    /// Binds the light and `lightmap`; the bind group is made again only when the host gives
    /// another lightmap.
    pub(crate) fn bind_lightmap(
        &mut self,
        device: &Device,
        lightmap: &TextureView,
        sampler: &Sampler,
    ) {
        let held = self
            .lightmap
            .take()
            .filter(|held| held.view == *lightmap && held.sampler == *sampler);
        let bind = held.unwrap_or_else(|| LightmapBind {
            view: lightmap.clone(),
            sampler: sampler.clone(),
            group: device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("entity figure light"),
                layout: &self.layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: self.light.as_entire_binding(),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::TextureView(lightmap),
                    },
                    BindGroupEntry {
                        binding: 2,
                        resource: BindingResource::Sampler(sampler),
                    },
                ],
            }),
        });
        self.lightmap = Some(bind);
    }

    /// The bind group of the light and the lightmap, after [`Self::bind_lightmap`].
    pub(crate) fn lightmap(&self) -> Option<&BindGroup> {
        self.lightmap.as_ref().map(|bind| &bind.group)
    }
}

/// The sizes of the model and the variant tables that the draw shader declares.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DrawTables {
    pub(crate) models: usize,
    pub(crate) variants: usize,
}

/// The frame uniform of the figure view: the projection of the host, the eye on the axis of the
/// view, no terrain and no impostor.
pub(crate) fn figure_uniform(
    figure: &FigureFrame,
    depth_compare: CompareFunction,
    tick: u32,
    partial_tick: f32,
) -> FrameUniform {
    FrameUniform {
        clip_from_relative: figure.clip_from_figure.to_cols_array(),
        frustum: [[0.0, 0.0, 0.0, 1.0]; 5],
        tick,
        camera_offset: figure_eye(figure.clip_from_figure, depth_compare).to_array(),
        partial_tick,
        height_threshold: 0.0,
        view: u32::from(EntityViews::FIGURE.0),
        ..FrameUniform::default()
    }
}

/// The eye of the figure view: the point at [`EYE_DISTANCE`] from the origin, on the side of the
/// viewer of the axis of clip depth.
fn figure_eye(clip_from_figure: Mat4, depth_compare: CompareFunction) -> Vec3 {
    let near = match depth_compare {
        CompareFunction::Greater | CompareFunction::GreaterEqual => 1.0,
        _ => -1.0,
    };
    let toward_viewer = clip_from_figure.inverse() * Vec4::new(0.0, 0.0, near, 0.0);
    toward_viewer.truncate().try_normalize().unwrap_or(Vec3::Z) * EYE_DISTANCE
}

fn figure_layout(device: &Device) -> BindGroupLayout {
    let fragment = ShaderStages::FRAGMENT;
    device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: Some("entity figure light"),
        entries: &[
            BindGroupLayoutEntry {
                binding: 0,
                visibility: fragment,
                ty: BindingType::Buffer {
                    ty: BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 1,
                visibility: fragment,
                ty: BindingType::Texture {
                    sample_type: wgpu::TextureSampleType::Float { filterable: true },
                    view_dimension: wgpu::TextureViewDimension::D2,
                    multisampled: false,
                },
                count: None,
            },
            BindGroupLayoutEntry {
                binding: 2,
                visibility: fragment,
                ty: BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                count: None,
            },
        ],
    })
}
