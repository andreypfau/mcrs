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

pub(crate) const FIGURE_RIGS: u32 = 16;

const EYE_DISTANCE: f32 = 1024.0;

#[repr(C)]
#[derive(Clone, Copy, Debug, Default, PartialEq, Pod, Zeroable)]
struct FigureLight {
    light0: [f32; 4],
    light1: [f32; 4],
}

struct LightmapBind {
    view: TextureView,
    sampler: Sampler,
    group: BindGroup,
}

pub(crate) struct FigureGpu {
    pub(crate) targets: FigureTargets,
    pub(crate) frame: FrameBuffers,
    pub(crate) binds: Binds,
    pub(crate) pipelines: FigurePipelines,
    light: Buffer,
    layout: BindGroupLayout,
    lightmap: LightmapBind,
}

impl FigureGpu {
    #[cfg_attr(
        mcrs_lint,
        expect(
            mcrs::too_many_arguments,
            reason = "the parts of the figure view come from the world view and from the host"
        )
    )]
    pub(crate) fn new(
        device: &Device,
        layouts: &Layouts,
        frame: FrameBuffers,
        binds: Binds,
        tables: DrawTables,
        targets: FigureTargets,
        lightmap: &TextureView,
        sampler: &Sampler,
    ) -> Self {
        let layout = figure_layout(device);
        let pipelines = FigurePipelines::new(device, layouts, &layout, tables, targets);
        let light = device.create_buffer(&BufferDescriptor {
            label: Some("entity figure light"),
            size: size_of::<FigureLight>() as u64,
            usage: BufferUsages::UNIFORM | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let lightmap = lightmap_bind(device, &layout, &light, lightmap, sampler);
        Self {
            targets,
            frame,
            binds,
            pipelines,
            light,
            layout,
            lightmap,
        }
    }

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

    pub(crate) fn write_light(&self, queue: &Queue, figure: &FigureFrame) {
        let [light0, light1] = figure
            .lights
            .map(|light| light.normalize_or_zero().extend(0.0).to_array());
        queue.write_buffer(
            &self.light,
            0,
            bytemuck::bytes_of(&FigureLight { light0, light1 }),
        );
    }

    pub(crate) fn bind_lightmap(
        &mut self,
        device: &Device,
        lightmap: &TextureView,
        sampler: &Sampler,
    ) {
        if self.lightmap.view != *lightmap || self.lightmap.sampler != *sampler {
            self.lightmap = lightmap_bind(device, &self.layout, &self.light, lightmap, sampler);
        }
    }

    pub(crate) fn light_group(&self) -> &BindGroup {
        &self.lightmap.group
    }
}

fn lightmap_bind(
    device: &Device,
    layout: &BindGroupLayout,
    light: &Buffer,
    lightmap: &TextureView,
    sampler: &Sampler,
) -> LightmapBind {
    LightmapBind {
        view: lightmap.clone(),
        sampler: sampler.clone(),
        group: device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("entity figure light"),
            layout,
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: light.as_entire_binding(),
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
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct DrawTables {
    pub(crate) models: usize,
    pub(crate) variants: usize,
}

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
        view: EntityViews::FIGURE.frame_bit(),
        ..FrameUniform::default()
    }
}

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
