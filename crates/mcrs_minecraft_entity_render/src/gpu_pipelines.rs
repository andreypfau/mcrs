use mcrs_minecraft_render::{gbuffer_encode_wgsl, visibility_wgsl};
use wgpu::util::DeviceExt;
use wgpu::{
    BindGroupEntry, BindGroupLayout, BindingResource, ComputePipeline, Device, RenderPipeline,
    TextureFormat,
};

use crate::entity_gpu::DrawTargets;
use crate::entity_library::{EntityLibrary, MAX_ARRAYS};
use crate::gpu_layouts::Layouts;
use crate::impostor_frame::{
    IMPOSTOR_ELEVATIONS, IMPOSTOR_SMALL_HEIGHT, IMPOSTOR_TILE, IMPOSTOR_VIEWS, IMPOSTOR_YAWS,
};

const COMMON: &str = include_str!("shaders/common.wgsl");
const COMPUTE: &str = include_str!("shaders/compute.wgsl");
const POSE: &str = include_str!("shaders/pose.wgsl");
const EXPAND: &str = include_str!("shaders/expand.wgsl");
const DRAW: &str = include_str!("shaders/draw.wgsl");
const SHADING: &str = include_str!("shaders/shading.wgsl");
const IMPOSTOR: &str = include_str!("shaders/impostor.wgsl");
const IMPOSTOR_PREPARE: &str = include_str!("shaders/impostor_prepare.wgsl");
/// The format of the colour and of the normal images of an impostor.
pub(crate) const IMPOSTOR_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;
pub(crate) const BAKE_DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;

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
    /// The draw of the face records with a cutout texel.
    pub(crate) draw: RenderPipeline,
    /// The draw of the other face records, with no discard.
    pub(crate) draw_opaque: RenderPipeline,
    /// The draw of the face records into the images of an impostor.
    pub(crate) bake: RenderPipeline,
    pub(crate) impostor: RenderPipeline,
    /// The draw of the small impostors, with no discard.
    pub(crate) impostor_small: RenderPipeline,
    pub(crate) impostor_draw_args: ComputePipeline,
    /// The pass that writes the draw record of each impostor.
    pub(crate) impostor_prepare: ComputePipeline,
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
        let header = compute_header(library);
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
        let impostor_draw_args =
            pipeline(&args_pipeline_layout, &compute_module, "impostor_draw_args");
        let impostor_prepare = pipeline(
            &main_pipeline_layout,
            &module(
                "entity impostor prepare",
                &(header.clone() + &impostor_constants(IMPOSTOR_PREPARE)),
            ),
            "prepare",
        );
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

        let (draw, draw_opaque, bake, impostor, impostor_small) =
            render_pipelines(device, layouts, targets);
        Self {
            scatter,
            cull,
            dispatch_args,
            draw_args,
            draw,
            draw_opaque,
            bake,
            impostor,
            impostor_small,
            impostor_draw_args,
            impostor_prepare,
            definitions,
        }
    }
}

/// The common records and the compute bindings, with the constants of `library`.
fn compute_header(library: &EntityLibrary) -> String {
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
    let box_sizes: Vec<String> = (0..definitions_count)
        .map(|index| {
            let (height, baby) = library
                .definitions
                .get(index)
                .map_or((1.0, 1.0), |definition| {
                    (definition.box_height, definition.baby_scale)
                });
            format!("vec2<f32>({height:?}, {baby:?})")
        })
        .collect();
    let variants_count = library.variants.len().max(1);
    COMMON.to_owned()
        + visibility_wgsl()
        + &COMPUTE
            .replace("#{DEFINITIONS}", &definitions_count.to_string())
            .replace("#{ARRAYS}", &arrays_count.to_string())
            .replace(
                "#{EXPAND_LANES}",
                &format!("array<u32, {definitions_count}>({})", lanes.join(", ")),
            )
            .replace(
                "#{BOX_SIZES}",
                &format!(
                    "array<vec2<f32>, {definitions_count}>({})",
                    box_sizes.join(", ")
                ),
            )
            .replace("#{VARIANTS}", &variants_count.to_string())
}

/// `source` with the constants of the impostor image sets.
pub(crate) fn impostor_constants(source: &str) -> String {
    source
        .replace("#{TILE}", &IMPOSTOR_TILE.to_string())
        .replace("#{SMALL_HEIGHT}", &format!("{IMPOSTOR_SMALL_HEIGHT:?}"))
        .replace("#{YAWS}", &IMPOSTOR_YAWS.to_string())
        .replace("#{VIEWS}", &IMPOSTOR_VIEWS.to_string())
        .replace("#{ELEVATION_COUNT}", &IMPOSTOR_ELEVATIONS.len().to_string())
        .replace(
            "#{ELEVATIONS}",
            &format!(
                "array<f32, {}>({})",
                IMPOSTOR_ELEVATIONS.len(),
                IMPOSTOR_ELEVATIONS
                    .map(|elevation| format!("{elevation:?}"))
                    .join(", ")
            ),
        )
}

/// The pipelines of the gbuffer draw of the face records, of the bake of the impostor images and
/// of the impostor draw.
fn render_pipelines(
    device: &Device,
    layouts: &Layouts,
    targets: DrawTargets,
) -> (
    RenderPipeline,
    RenderPipeline,
    RenderPipeline,
    RenderPipeline,
    RenderPipeline,
) {
    let module = |label: &str, source: &str| {
        device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some(label),
            source: wgpu::ShaderSource::Wgsl(source.into()),
        })
    };
    let draw_module = module(
        "entity draw",
        &(COMMON.to_owned() + gbuffer_encode_wgsl() + SHADING + DRAW),
    );
    let draw_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("entity draw"),
        bind_group_layouts: &[Some(&layouts.draw), Some(&layouts.texture)],
        immediate_size: 0,
    });
    let gbuffer = targets.colors.map(|format| Some(color_target(format)));
    let depth = depth_state(targets.depth, targets.depth_compare);
    let draw = render_pipeline(
        device,
        &draw_layout,
        Stages {
            module: &draw_module,
            vertex: "vertex",
            fragment: "fragment",
            topology: wgpu::PrimitiveTopology::TriangleStrip,
        },
        &gbuffer,
        depth.clone(),
    );
    let draw_opaque = render_pipeline(
        device,
        &draw_layout,
        Stages {
            module: &draw_module,
            vertex: "vertex",
            fragment: "fragment_opaque",
            topology: wgpu::PrimitiveTopology::TriangleStrip,
        },
        &gbuffer,
        depth.clone(),
    );
    let bake = render_pipeline(
        device,
        &draw_layout,
        Stages {
            module: &draw_module,
            vertex: "bake_vertex",
            fragment: "bake_fragment",
            topology: wgpu::PrimitiveTopology::TriangleList,
        },
        &[
            Some(color_target(IMPOSTOR_FORMAT)),
            Some(color_target(IMPOSTOR_FORMAT)),
        ],
        depth_state(BAKE_DEPTH_FORMAT, wgpu::CompareFunction::GreaterEqual),
    );
    let impostor_source =
        COMMON.to_owned() + gbuffer_encode_wgsl() + SHADING + &impostor_constants(IMPOSTOR);
    let impostor_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
        label: Some("entity impostor"),
        bind_group_layouts: &[Some(&layouts.impostor), Some(&layouts.impostor_textures)],
        immediate_size: 0,
    });
    let impostor_module = module("entity impostor", &impostor_source);
    let impostor = render_pipeline(
        device,
        &impostor_layout,
        Stages {
            module: &impostor_module,
            vertex: "vertex",
            fragment: "fragment",
            topology: wgpu::PrimitiveTopology::TriangleList,
        },
        &gbuffer,
        depth.clone(),
    );
    let impostor_small = render_pipeline(
        device,
        &impostor_layout,
        Stages {
            module: &impostor_module,
            vertex: "vertex",
            fragment: "fragment_small",
            topology: wgpu::PrimitiveTopology::TriangleList,
        },
        &gbuffer,
        depth,
    );
    (draw, draw_opaque, bake, impostor, impostor_small)
}

/// The shader module and the entry points of a render pipeline.
struct Stages<'a> {
    module: &'a wgpu::ShaderModule,
    vertex: &'a str,
    fragment: &'a str,
    topology: wgpu::PrimitiveTopology,
}

fn color_target(format: TextureFormat) -> wgpu::ColorTargetState {
    wgpu::ColorTargetState {
        format,
        blend: None,
        write_mask: wgpu::ColorWrites::ALL,
    }
}

fn depth_state(format: TextureFormat, compare: wgpu::CompareFunction) -> wgpu::DepthStencilState {
    wgpu::DepthStencilState {
        format,
        depth_write_enabled: Some(true),
        depth_compare: Some(compare),
        stencil: Default::default(),
        bias: Default::default(),
    }
}

/// An opaque render pipeline with no vertex buffers and no face culling.
fn render_pipeline(
    device: &Device,
    layout: &wgpu::PipelineLayout,
    stages: Stages<'_>,
    targets: &[Option<wgpu::ColorTargetState>],
    depth: wgpu::DepthStencilState,
) -> RenderPipeline {
    device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some(stages.vertex),
        layout: Some(layout),
        vertex: wgpu::VertexState {
            module: stages.module,
            entry_point: Some(stages.vertex),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: wgpu::PrimitiveState {
            topology: stages.topology,
            cull_mode: None,
            ..Default::default()
        },
        depth_stencil: Some(depth),
        multisample: Default::default(),
        fragment: Some(wgpu::FragmentState {
            module: stages.module,
            entry_point: Some(stages.fragment),
            compilation_options: Default::default(),
            targets,
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

#[cfg(test)]
mod tests {
    use mcrs_minecraft_client_entity::HURT_TICKS;
    use mcrs_minecraft_core::CardinalLight;
    use mcrs_minecraft_entity_model::{BONE_NEVER_RENDER, BoneRecord, ModelRecord, NO_PARENT};
    use mcrs_minecraft_mesh::face_shade;
    use naga::{Literal, Module, TypeInner};

    use super::*;
    use crate::counts_readback::{
        COUNTER_DROPPED, COUNTER_FACES, COUNTER_IMPOSTORS, COUNTER_VISIBLE,
    };
    use crate::entity_library::{NO_PALETTE, VariantRecord};
    use crate::frame_uniform::FrameUniform;
    use crate::records::{
        CULL_FULL_RIG_ONLY, CULL_VALID, CullRecord, InputRecord, STATE_BABY, STATE_DYING,
        STATE_HIDDEN_SHIFT, ScatterEntry, StateRecord, TickRecord,
    };
    use crate::slot_upload::SCATTER_FRESH;

    fn struct_size(module: &Module, name: &str) -> Option<usize> {
        module.types.iter().find_map(|(_, ty)| match &ty.inner {
            TypeInner::Struct { span, .. } if ty.name.as_deref() == Some(name) => {
                Some(*span as usize)
            }
            _ => None,
        })
    }

    fn constant(module: &Module, name: &str) -> Option<i64> {
        let (_, constant) = module
            .constants
            .iter()
            .find(|(_, constant)| constant.name.as_deref() == Some(name))?;
        match module.global_expressions[constant.init] {
            naga::Expression::Literal(Literal::U32(value)) => Some(i64::from(value)),
            naga::Expression::Literal(Literal::I32(value)) => Some(i64::from(value)),
            _ => None,
        }
    }

    #[test]
    fn the_wgsl_records_and_constants_match_the_rust_ones() {
        let source = compute_header(&EntityLibrary::default());
        let module = naga::front::wgsl::parse_str(&source).unwrap();
        let sizes = [
            ("Frame", size_of::<FrameUniform>()),
            ("TickRecord", size_of::<TickRecord>()),
            ("CullRecord", size_of::<CullRecord>()),
            ("StateRecord", size_of::<StateRecord>()),
            ("InputRecord", size_of::<InputRecord>()),
            ("ScatterEntry", size_of::<ScatterEntry>()),
            ("ModelRecord", size_of::<ModelRecord>()),
            ("BoneRecord", size_of::<BoneRecord>()),
            ("Variant", size_of::<VariantRecord>()),
        ];
        for (name, size) in sizes {
            assert_eq!(struct_size(&module, name), Some(size), "the size of {name}");
        }
        let constants = [
            ("CULL_VALID", i64::from(CULL_VALID)),
            ("CULL_FULL_RIG_ONLY", i64::from(CULL_FULL_RIG_ONLY)),
            ("STATE_BABY", i64::from(STATE_BABY)),
            ("STATE_DYING", i64::from(STATE_DYING)),
            ("STATE_HIDDEN_SHIFT", i64::from(STATE_HIDDEN_SHIFT)),
            ("SCATTER_FRESH", i64::from(SCATTER_FRESH)),
            ("NO_PALETTE", i64::from(NO_PALETTE)),
            ("NO_PARENT", i64::from(NO_PARENT)),
            ("BONE_NEVER_RENDER", i64::from(BONE_NEVER_RENDER)),
            ("HURT_TICKS", i64::from(HURT_TICKS)),
            (
                "SHADE_TABLE_NETHER",
                i64::from(face_shade::shader_index(CardinalLight::Nether)),
            ),
            ("COUNTER_VISIBLE", COUNTER_VISIBLE as i64),
            ("COUNTER_FACES", COUNTER_FACES as i64),
            ("COUNTER_DROPPED", COUNTER_DROPPED as i64),
            ("COUNTER_IMPOSTORS", COUNTER_IMPOSTORS as i64),
        ];
        for (name, value) in constants {
            assert_eq!(constant(&module, name), Some(value), "the value of {name}");
        }
    }
}
