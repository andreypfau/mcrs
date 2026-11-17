use mcrs_minecraft_render::{GBUFFER_ENCODE_WGSL, VISIBILITY_WGSL};
use wgpu::util::DeviceExt;
use wgpu::{
    BindGroupEntry, BindGroupLayout, BindingResource, ComputePipeline, Device, RenderPipeline,
    TextureFormat,
};

use crate::entity_gpu::DrawTargets;
use crate::entity_library::{EntityLibrary, MAX_ARRAYS};
use crate::figure_gpu::DrawTables;
use crate::figure_targets::FigureTargets;
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
const FIGURE: &str = include_str!("shaders/figure.wgsl");
const IMPOSTOR: &str = include_str!("shaders/impostor.wgsl");
const IMPOSTOR_PREPARE: &str = include_str!("shaders/impostor_prepare.wgsl");
pub(crate) const IMPOSTOR_FORMAT: TextureFormat = TextureFormat::Rgba8Unorm;
pub(crate) const BAKE_DEPTH_FORMAT: TextureFormat = TextureFormat::Depth32Float;

pub(crate) struct DefinitionGpu {
    pub(crate) pose: Option<ComputePipeline>,
    pub(crate) expand: ComputePipeline,
}

pub(crate) struct Pipelines {
    pub(crate) scatter: ComputePipeline,
    pub(crate) cull: ComputePipeline,
    pub(crate) dispatch_args: ComputePipeline,
    pub(crate) draw_args: ComputePipeline,
    pub(crate) draw: RenderPipeline,
    pub(crate) draw_opaque: RenderPipeline,
    pub(crate) bake: RenderPipeline,
    pub(crate) impostor: RenderPipeline,
    pub(crate) impostor_small: RenderPipeline,
    pub(crate) impostor_draw_args: ComputePipeline,
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
        let header = compute_header(library);
        let compute_layout_of = |layout: &BindGroupLayout| {
            device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
                label: Some("entity compute"),
                bind_group_layouts: &[Some(layout)],
                immediate_size: 0,
            })
        };
        let scatter_layout = compute_layout_of(&layouts.scatter);
        let cull_layout = compute_layout_of(&layouts.cull);
        let pose_layout = compute_layout_of(&layouts.pose);
        let expand_layout = compute_layout_of(&layouts.expand);
        let prepare_layout = compute_layout_of(&layouts.prepare);
        let args_pipeline_layout = compute_layout_of(&layouts.args);
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
        let scatter = pipeline(&scatter_layout, &compute_module, "scatter_ticks");
        let cull = pipeline(&cull_layout, &compute_module, "cull");
        let dispatch_args = pipeline(&args_pipeline_layout, &compute_module, "dispatch_args");
        let draw_args = pipeline(&args_pipeline_layout, &compute_module, "draw_args");
        let impostor_draw_args =
            pipeline(&args_pipeline_layout, &compute_module, "impostor_draw_args");
        let impostor_prepare = pipeline(
            &prepare_layout,
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
                    pipeline(&pose_layout, &module, "pose")
                });
                let source = header.clone()
                    + &EXPAND
                        .replace("#{DEFINITION}", &index.to_string())
                        .replace("#{LANES}", &definition.expand_lanes.to_string());
                let expand = pipeline(&expand_layout, &module("entity expand", &source), "expand");
                DefinitionGpu { pose, expand }
            })
            .collect();

        let (draw, draw_opaque, bake, impostor, impostor_small) =
            render_pipelines(device, library, layouts, targets);
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
    let first_cover = library.impostor_frames.len() * IMPOSTOR_VIEWS;
    COMMON.to_owned()
        + VISIBILITY_WGSL
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
            .replace("#{MODELS}", &library.models.len().max(1).to_string())
            .replace("#{BONES}", &library.bones.len().max(1).to_string())
            .replace("#{FIRST_COVER}", &first_cover.to_string())
}

fn draw_source(models: usize, variants: usize, depth_compare: wgpu::CompareFunction) -> String {
    let layer_depth_step = match depth_compare {
        wgpu::CompareFunction::Greater | wgpu::CompareFunction::GreaterEqual => "1.0 / 65536.0",
        _ => "-1.0 / 65536.0",
    };
    COMMON.to_owned()
        + GBUFFER_ENCODE_WGSL
        + SHADING
        + &DRAW
            .replace("#{MODELS}", &models.max(1).to_string())
            .replace("#{VARIANTS}", &variants.max(1).to_string())
            .replace("#{LAYER_DEPTH_STEP}", layer_depth_step)
}

fn figure_source(tables: DrawTables, targets: FigureTargets) -> String {
    draw_source(tables.models, tables.variants, targets.depth_compare)
        + &FIGURE.replace("#{TARGET_ENCODES}", &targets.color.is_srgb().to_string())
}

pub(crate) struct FigurePipelines {
    pub(crate) draw: RenderPipeline,
    pub(crate) draw_opaque: RenderPipeline,
}

impl FigurePipelines {
    pub(crate) fn new(
        device: &Device,
        layouts: &Layouts,
        figure: &BindGroupLayout,
        tables: DrawTables,
        targets: FigureTargets,
    ) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("entity figure"),
            source: wgpu::ShaderSource::Wgsl(figure_source(tables, targets).into()),
        });
        let layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("entity figure"),
            bind_group_layouts: &[Some(&layouts.draw), Some(&layouts.texture), Some(figure)],
            immediate_size: 0,
        });
        let colour = [Some(color_target(targets.color))];
        let pipeline = |fragment: &str| {
            render_pipeline(
                device,
                &layout,
                Stages {
                    module: &module,
                    vertex: "vertex",
                    fragment,
                    topology: wgpu::PrimitiveTopology::TriangleStrip,
                },
                &colour,
                depth_state(targets.depth, targets.depth_compare),
                targets.sample_count,
            )
        };
        Self {
            draw: pipeline("figure_fragment"),
            draw_opaque: pipeline("figure_fragment_opaque"),
        }
    }
}

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

fn render_pipelines(
    device: &Device,
    library: &EntityLibrary,
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
        &draw_source(
            library.models.len(),
            library.variants.len(),
            targets.depth_compare,
        ),
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
        1,
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
        1,
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
        1,
    );
    let impostor_source =
        COMMON.to_owned() + GBUFFER_ENCODE_WGSL + SHADING + &impostor_constants(IMPOSTOR);
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
        1,
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
        1,
    );
    (draw, draw_opaque, bake, impostor, impostor_small)
}

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

fn render_pipeline(
    device: &Device,
    layout: &wgpu::PipelineLayout,
    stages: Stages<'_>,
    targets: &[Option<wgpu::ColorTargetState>],
    depth: wgpu::DepthStencilState,
    samples: u32,
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
        multisample: wgpu::MultisampleState {
            count: samples,
            ..Default::default()
        },
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

pub(crate) fn texture_arrays(
    device: &Device,
    queue: &wgpu::Queue,
    library: &EntityLibrary,
    texture_layout: &BindGroupLayout,
) -> Vec<(wgpu::Texture, wgpu::BindGroup)> {
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
            let layer_bytes = (array.side * array.side * 4) as usize;
            let mut texels = Vec::with_capacity(layer_bytes * array.layers.len());
            for layer in &array.layers {
                let start = texels.len();
                texels.extend_from_slice(layer);
                texels.resize(start + layer_bytes, 0);
            }
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
                    usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
                    view_formats: &[],
                },
                wgpu::util::TextureDataOrder::LayerMajor,
                &texels,
            );
            let view = texture.create_view(&wgpu::TextureViewDescriptor {
                dimension: Some(wgpu::TextureViewDimension::D2Array),
                ..Default::default()
            });
            let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
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
            });
            (texture, bind)
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use mcrs_minecraft_core::CardinalLight;
    use mcrs_minecraft_entity_model::{BONE_NEVER_RENDER, BoneRecord, ModelRecord, NO_PARENT};
    use mcrs_minecraft_entity_visual::HURT_TICKS;
    use mcrs_minecraft_mesh::face_shade;
    use naga::{Literal, Module, TypeInner};

    use super::*;
    use crate::counts_readback::{
        COUNTER_DROPPED, COUNTER_FACES, COUNTER_IMPOSTORS, COUNTER_VISIBLE,
    };
    use crate::entity_library::{NO_PALETTE, VariantRecord};
    use crate::frame_uniform::FrameUniform;
    use crate::gpu_layouts as layouts;
    use crate::records::{
        CULL_FULL_RIG_ONLY, CULL_VALID, CULL_VIEWS_SHIFT, CullRecord, InputRecord, STATE_BABY,
        STATE_DYING, STATE_HIDDEN_SHIFT, ScatterEntry, StateRecord, TickRecord,
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
    fn the_draw_shader_with_the_shared_gbuffer_include_is_valid_wgsl() {
        naga::front::wgsl::parse_str(&draw_source(0, 0, wgpu::CompareFunction::GreaterEqual))
            .unwrap();
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
            ("CULL_VIEWS_SHIFT", i64::from(CULL_VIEWS_SHIFT)),
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

    fn check_tint_rules(label: &str, source: &str) {
        if let Err(error) = source.parse::<wgsl_parse::syntax::TranslationUnit>() {
            panic!("{label}: {error}");
        }
        for (at, _) in source.match_indices("bitcast<f32>(") {
            let argument = &source[at + "bitcast<f32>(".len()..];
            let argument = argument[..argument.find(')').unwrap()].trim();
            let digits = argument.trim_end_matches(['u', 'i']);
            let bits = match digits.strip_prefix("0x") {
                Some(hex) => u32::from_str_radix(hex, 16),
                None => digits.parse::<u32>(),
            };
            if let Ok(bits) = bits {
                assert!(
                    f32::from_bits(bits).is_finite(),
                    "{label}: the constant bitcast<f32>({argument}) is not a finite f32"
                );
            }
        }
    }

    fn check_bindings(label: &str, source: &str, passes: &[(&str, &[(u32, layouts::Slot)])]) {
        check_tint_rules(label, source);
        let module = naga::front::wgsl::parse_str(source).unwrap();
        let info = naga::valid::Validator::new(
            naga::valid::ValidationFlags::all(),
            naga::valid::Capabilities::all(),
        )
        .validate(&module)
        .unwrap();
        for (index, entry) in module.entry_points.iter().enumerate() {
            let Some((_, layout)) = passes.iter().find(|(name, _)| *name == entry.name) else {
                continue;
            };
            let function = info.get_entry_point(index);
            for (handle, global) in module.global_variables.iter() {
                let Some(binding) = &global.binding else {
                    continue;
                };
                if binding.group != 0 || function[handle].is_empty() {
                    continue;
                }
                let declared = match global.space {
                    naga::AddressSpace::Uniform => {
                        if binding.binding == 0 {
                            layouts::Slot::Frame
                        } else {
                            layouts::Slot::Table
                        }
                    }
                    naga::AddressSpace::Storage { access } => layouts::Slot::Storage {
                        read_only: !access.contains(naga::StorageAccess::STORE),
                    },
                    _ => layouts::Slot::Pyramid,
                };
                assert!(
                    layout.contains(&(binding.binding, declared)),
                    "{label} {}: binding {} as {declared:?} is not in the layout of its pass",
                    entry.name,
                    binding.binding
                );
            }
        }
    }

    #[test]
    fn each_pass_binds_what_its_shader_uses_within_the_limits_of_webgpu() {
        let pack = crate::EntityPack::repository();
        let white = crate::TextureImage {
            identifier: "test:white".into(),
            width: 16,
            height: 16,
            rgba: vec![255; 16 * 16 * 4],
        };
        let library = EntityLibrary::build(&pack.sources(), &[white], 2048);
        let header = compute_header(&library);
        check_bindings(
            "compute",
            &header,
            &[
                ("scatter_ticks", &layouts::SCATTER),
                ("cull", &layouts::CULL),
                ("dispatch_args", &layouts::ARGS),
                ("draw_args", &layouts::ARGS),
                ("impostor_draw_args", &layouts::ARGS),
            ],
        );
        check_bindings(
            "prepare",
            &(header.clone() + &impostor_constants(IMPOSTOR_PREPARE)),
            &[("prepare", &layouts::PREPARE)],
        );
        for (index, definition) in library.definitions.iter().enumerate() {
            let program = definition.program.as_ref().unwrap();
            let pose = header.clone()
                + mcrs_molang_wgsl::LIBRARY
                + &program.wgsl
                + &POSE
                    .replace("#{PALETTE}", &definition.palette.len().max(1).to_string())
                    .replace("#{DEFINITION}", &index.to_string())
                    .replace("#{DEATH_FLIP}", &format!("{:?}", definition.death_flip));
            check_bindings(&definition.identifier, &pose, &[("pose", &layouts::POSE)]);
            let expand = header.clone()
                + &EXPAND
                    .replace("#{DEFINITION}", &index.to_string())
                    .replace("#{LANES}", &definition.expand_lanes.to_string());
            check_bindings(
                &definition.identifier,
                &expand,
                &[("expand", &layouts::EXPAND)],
            );
        }
        check_bindings(
            "draw",
            &draw_source(
                library.models.len(),
                library.variants.len(),
                wgpu::CompareFunction::GreaterEqual,
            ),
            &[
                ("vertex", &layouts::DRAW),
                ("fragment", &[(0, layouts::Slot::Frame)]),
                ("fragment_opaque", &[(0, layouts::Slot::Frame)]),
            ],
        );
        check_bindings(
            "figure",
            &figure_source(
                DrawTables {
                    models: library.models.len(),
                    variants: library.variants.len(),
                },
                FigureTargets {
                    color: wgpu::TextureFormat::Rgba8UnormSrgb,
                    depth: wgpu::TextureFormat::Depth32Float,
                    depth_compare: wgpu::CompareFunction::LessEqual,
                    sample_count: 1,
                },
            ),
            &[
                ("vertex", &layouts::DRAW),
                ("figure_fragment", &[(0, layouts::Slot::Frame)]),
                ("figure_fragment_opaque", &[(0, layouts::Slot::Frame)]),
            ],
        );
        check_tint_rules(
            "impostor",
            &(COMMON.to_owned() + GBUFFER_ENCODE_WGSL + SHADING + &impostor_constants(IMPOSTOR)),
        );
        check_tint_rules(
            "impostor cover",
            &impostor_constants(crate::impostor_atlas::COVER).replace("#{FIRST_COVER}", "0"),
        );
        let webgpu = wgpu::Limits::default();
        assert!(layouts::passes_fit(&webgpu));
        assert!(layouts::tables_fit(&library, &webgpu));
    }
}
