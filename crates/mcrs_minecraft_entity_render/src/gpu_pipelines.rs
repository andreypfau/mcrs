use wgpu::util::DeviceExt;
use wgpu::{
    BindGroupEntry, BindGroupLayout, BindingResource, ComputePipeline, Device, RenderPipeline,
    TextureFormat,
};

use crate::entity_gpu::DrawTargets;
use crate::entity_library::{EntityLibrary, MAX_ARRAYS};
use crate::gpu_layouts::Layouts;

const COMMON: &str = include_str!("shaders/common.wgsl");
const COMPUTE: &str = include_str!("shaders/compute.wgsl");
const POSE: &str = include_str!("shaders/pose.wgsl");
const EXPAND: &str = include_str!("shaders/expand.wgsl");
const DRAW: &str = include_str!("shaders/draw.wgsl");

/// The pose and the expand pipelines of one definition.
pub(crate) struct DefinitionGpu {
    /// `None` for a definition with no GPU pose, which draws nothing.
    pub(crate) pose: Option<ComputePipeline>,
    pub(crate) expand: ComputePipeline,
}

/// The pipelines of the entity passes.
pub(crate) struct Pipelines {
    pub(crate) scatter: ComputePipeline,
    pub(crate) cull: ComputePipeline,
    pub(crate) dispatch_args: ComputePipeline,
    pub(crate) draw_args: ComputePipeline,
    pub(crate) draw: RenderPipeline,
    pub(crate) definitions: Vec<DefinitionGpu>,
}

impl Pipelines {
    pub(crate) fn new(
        device: &Device,
        library: &EntityLibrary,
        layouts: &Layouts,
        targets: DrawTargets,
    ) -> Self {
        let compute_layout = &layouts.compute;
        let args_layout = &layouts.args;
        let definitions_count = library.definitions.len().max(1);
        let arrays_count = library.arrays.len().clamp(1, MAX_ARRAYS);
        let lanes: Vec<String> = (0..definitions_count)
            .map(|index| {
                library
                    .definitions
                    .get(index)
                    .map_or(1, |definition| definition.expand_lanes)
                    .to_string()
                    + "u"
            })
            .collect();
        let header = COMMON.to_owned()
            + &COMPUTE
                .replace("#{DEFINITIONS}", &definitions_count.to_string())
                .replace("#{ARRAYS}", &arrays_count.to_string())
                .replace(
                    "#{EXPAND_LANES}",
                    &format!("array<u32, {definitions_count}>({})", lanes.join(", ")),
                );
        let compute_layout_of = |layout: &BindGroupLayout| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("entity compute"),
                bind_group_layouts: &[Some(layout)],
                immediate_size: 0,
            })
        };
        let main_pipeline_layout = compute_layout_of(compute_layout);
        let args_pipeline_layout = compute_layout_of(args_layout);
        let module = |label: &str, source: &str| {
            device.create_shader_module(wgpu::ShaderModuleDescriptor {
                label: Some(label),
                source: wgpu::ShaderSource::Wgsl(source.into()),
            })
        };
        let compute_module = module("entity compute", &header);
        let pipeline = |layout: &wgpu::PipelineLayout, module: &wgpu::ShaderModule, entry: &str| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry),
                layout: Some(layout),
                module,
                entry_point: Some(entry),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let scatter = pipeline(&main_pipeline_layout, &compute_module, "scatter_ticks");
        let cull = pipeline(&main_pipeline_layout, &compute_module, "cull");
        let dispatch_args = pipeline(&args_pipeline_layout, &compute_module, "dispatch_args");
        let draw_args = pipeline(&args_pipeline_layout, &compute_module, "draw_args");
        let definitions = library
            .definitions
            .iter()
            .enumerate()
            .map(|(index, definition)| {
                let pose = definition.program.as_ref().ok().map(|program| {
                    let source = header.clone()
                        + mcrs_molang_wgsl::LIBRARY
                        + &program.wgsl
                        + &POSE
                            .replace("#{PALETTE}", &definition.palette.len().max(1).to_string())
                            .replace("#{DEFINITION}", &index.to_string())
                            .replace("#{DEATH_FLIP}", &format!("{:?}", definition.death_flip));
                    let module = module(&definition.identifier, &source);
                    pipeline(&main_pipeline_layout, &module, "pose")
                });
                let source = header.clone()
                    + &EXPAND
                        .replace("#{DEFINITION}", &index.to_string())
                        .replace("#{LANES}", &definition.expand_lanes.to_string());
                let expand = pipeline(
                    &main_pipeline_layout,
                    &module("entity expand", &source),
                    "expand",
                );
                DefinitionGpu { pose, expand }
            })
            .collect();

        let draw = draw_pipeline(device, layouts, targets);
        Self {
            scatter,
            cull,
            dispatch_args,
            draw_args,
            draw,
            definitions,
        }
    }
}

/// The pipeline of the gbuffer draw of the face records.
fn draw_pipeline(device: &Device, layouts: &Layouts, targets: DrawTargets) -> RenderPipeline {
    let draw_module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("entity draw"),
        source: wgpu::ShaderSource::Wgsl((COMMON.to_owned() + DRAW).into()),
    });
    let draw_pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("entity draw"),
        bind_group_layouts: &[Some(&layouts.draw), Some(&layouts.texture)],
        immediate_size: 0,
    });
    let color_targets = targets.colors.map(|format| {
        Some(wgpu::ColorTargetState {
            format,
            blend: None,
            write_mask: wgpu::ColorWrites::ALL,
        })
    });
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("entity draw"),
        layout: Some(&draw_pipeline_layout),
        vertex: wgpu::VertexState {
            module: &draw_module,
            entry_point: Some("vertex"),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState {
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(wgpu::DepthStencilState {
            format: targets.depth,
            depth_write_enabled: Some(true),
            depth_compare: Some(targets.depth_compare),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: &draw_module,
            entry_point: Some("fragment"),
            compilation_options: Default::default(),
            targets: &color_targets,
        }),
        multiview_mask: None,
        cache: None,
    })
}

/// The texture arrays of `library`, each with its bind group.
pub(crate) fn texture_arrays(
    device: &Device,
    queue: &wgpu::Queue,
    library: &EntityLibrary,
    texture_layout: &BindGroupLayout,
) -> Vec<wgpu::BindGroup> {
    let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
        label: Some("entity textures"),
        ..Default::default()
    });
    library
        .arrays
        .iter()
        .take(MAX_ARRAYS)
        .map(|array| {
            let layers = array.layers.len().max(1) as u32;
            let texture = device.create_texture_with_data(
                queue,
                &wgpu::TextureDescriptor {
                    label: Some("entity texture array"),
                    size: wgpu::Extent3d {
                        width: array.side,
                        height: array.side,
                        depth_or_array_layers: layers,
                    },
                    mip_level_count: 1,
                    sample_count: 1,
                    dimension: wgpu::TextureDimension::D2,
                    format: TextureFormat::Rgba8Unorm,
                    usage: wgpu::TextureUsages::TEXTURE_BINDING,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                &array.layers.concat(),
            );
            let view = texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("entity texture array"),
                layout: texture_layout,
                entries: &[
                    BindGroupEntry {
                        binding: 0,
                        resource: BindingResource::TextureView(&view),
                    },
                    BindGroupEntry {
                        binding: 1,
                        resource: BindingResource::Sampler(&sampler),
                    },
                ],
            })
        })
        .collect()
}
