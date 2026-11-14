use bevy::tasks::block_on;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BindingResource, BlendState, Buffer,
    BufferDescriptor, BufferUsages, Color, ColorTargetState, ColorWrites, ComputePipeline,
    ComputePipelineDescriptor, Device, DeviceDescriptor, Extent3d, FragmentState, Instance,
    InstanceDescriptor, LoadOp, MapMode, Operations, PipelineCompilationOptions, PollType,
    PowerPreference, Queue, RenderPassColorAttachment, RenderPassDescriptor, RenderPipeline,
    RenderPipelineDescriptor, RequestAdapterOptions, ShaderModule, ShaderModuleDescriptor,
    ShaderSource, StoreOp, TexelCopyBufferInfo, TexelCopyBufferLayout, TextureDescriptor,
    TextureFormat, TextureUsages, TextureView, VertexState,
};

use crate::world_shaders::{cull_defs, wgsl};

const SECTION_SLOTS: u32 = 64;
const CAVE_WORDS: usize = 2;
/// Slots between two columns of a run, so a run reaches the second word of the bitset.
const COLUMN_STRIDE: usize = 4;
/// Word places in the `Camera` uniform of `frame.wesl`.
const CAMERA_WORDS: usize = 64;
const FRUSTUM_WORD: usize = 16;
const FRUSTUM_PLANES: usize = 5;
const SECTION_WORD: usize = 36;
const RENDER_DISTANCE_WORD: usize = 53;
const CAMERA_SECTION: [i32; 3] = [5, 4, -3];

struct Column {
    offset: [i32; 2],
    drawn: bool,
}

struct Run {
    render_distance: u8,
    columns: &'static [Column],
}

const fn column(dx: i32, dz: i32, drawn: bool) -> Column {
    Column {
        offset: [dx, dz],
        drawn,
    }
}

const RUNS: [Run; 3] = [
    Run {
        render_distance: 2,
        columns: &[
            column(0, 0, true),
            column(1, 0, true),
            column(-1, 1, true),
            column(1, -1, true),
            column(2, 0, true),
            column(0, -2, true),
            column(3, 0, false),
            column(-3, 0, false),
            column(0, 3, false),
            column(2, 2, true),
            column(-2, -2, true),
            column(3, 2, false),
            column(2, -3, false),
        ],
    },
    Run {
        render_distance: 5,
        columns: &[
            column(5, 0, true),
            column(4, 4, true),
            column(4, 5, false),
            column(-5, -4, false),
        ],
    },
    Run {
        render_distance: 96,
        columns: &[
            column(0, 0, true),
            column(1, 1, true),
            column(96, 0, true),
            column(0, -96, true),
            column(97, 0, false),
            column(-97, 1, false),
            column(1, 97, false),
            column(68, 68, true),
            column(-68, 68, true),
            column(69, -68, true),
            column(69, 69, false),
            column(-69, -69, false),
            column(300, 0, false),
        ],
    },
];

/// A point of CFOG-TEST-001 or an edge of the ramp: its offset from the camera, the environmental
/// and the render-distance ramps, and its fog value.
struct FogCase {
    offset: [f32; 3],
    environmental: [f32; 2],
    render_distance: [f32; 2],
    value: f32,
}

const NOON: [[f32; 2]; 2] = [[0.0, 1024.0], [172.8, 192.0]];
const NO_FOG: [f32; 2] = [f32::MAX, f32::MAX];

const fn fog_case(offset: [f32; 3], ramps: [[f32; 2]; 2], value: f32) -> FogCase {
    FogCase {
        offset,
        environmental: ramps[0],
        render_distance: ramps[1],
        value,
    }
}

const FOG_CASES: [FogCase; 7] = [
    fog_case([180.0, 0.0, 0.0], NOON, 0.375),
    fog_case([100.0, -20.0, 50.0], NOON, 0.110916),
    fog_case([150.0, -100.0, 0.0], NOON, 0.176052),
    fog_case([0.0, -200.0, 0.0], NOON, 1.0),
    fog_case([1.0e6, 3.0e5, -2.0e6], [NO_FOG, NO_FOG], 0.0),
    fog_case([0.0, 0.0, 10.0], [NO_FOG, [10.0, 10.0]], 0.0),
    fog_case([0.0, 0.0, 10.5], [NO_FOG, [10.0, 4.0]], 1.0),
];

const FOG_KERNEL: &str = "
@group(0) @binding(0) var<storage, read> cases: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read_write> values: array<f32>;

@compute @workgroup_size(1)
fn fog_value_kernel(@builtin(global_invocation_id) id: vec3<u32>) {
    let ramps = cases[id.x * 2u + 1u];
    values[id.x] = fog_value(cases[id.x * 2u].xyz, ramps.xy, ramps.zw);
}
";

/// A face of CFOG-TEST-005: its two fog values, the cloud image at its pixel, and its colour.
struct ImprovedCase {
    values: [f32; 2],
    cloud: [f32; 4],
    cloud_behind: bool,
    pixel: [f32; 3],
}

const FACE: [f32; 3] = [0.4, 0.5, 0.1];
const NOON_FOG: [f32; 3] = [0.711644, 0.818954, 1.0];
const SKY_BYTES: [f32; 3] = [155.0 / 255.0, 191.0 / 255.0, 1.0];
const NO_CLOUD: [f32; 4] = [0.0; 4];
const CLOUD: [f32; 4] = [0.6, 0.6, 0.6, 0.6];
const AT_180: [f32; 2] = [0.178206, 0.375];
const AT_192: [f32; 2] = [0.178206, 1.0];
const RAIN_AT_10: [f32; 2] = [0.183190, 0.0];
const NO_CLOUD_PIXEL: [f32; 3] = [0.502270, 0.609773, 0.4375];

const fn improved_case(
    values: [f32; 2],
    cloud: [f32; 4],
    cloud_behind: bool,
    pixel: [f32; 3],
) -> ImprovedCase {
    ImprovedCase {
        values,
        cloud,
        cloud_behind,
        pixel,
    }
}

fn plain_fog_mix(value: f32) -> [f32; 3] {
    std::array::from_fn(|channel| FACE[channel] + value * (NOON_FOG[channel] - FACE[channel]))
}

const IMPROVED_KERNEL: &str = "
@group(0) @binding(0) var<storage, read> cases: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read_write> pixels: array<vec4<f32>>;

@compute @workgroup_size(1)
fn improved_fog(@builtin(global_invocation_id) id: vec3<u32>) {
    let face = cases[id.x * 5u];
    let fog = cases[id.x * 5u + 1u];
    let color = cases[id.x * 5u + 2u].rgb;
    let sky = cases[id.x * 5u + 3u].rgb;
    let cloud = cases[id.x * 5u + 4u];
    pixels[id.x] = vec4<f32>(improved_fog_mix(face.rgb, fog.xy, color, sky, cloud, fog.z > 0.5), 1.0);
}
";

/// A grey pixel of CFOG-TIME-002 under the cloud image: `m`, `cl`, `ca`, the depth of the cloud
/// image and of the pixel, and the composited value.
const CLOUD_OVER_CASES: [[f32; 6]; 2] = [
    [0.2, 0.5, 0.5, 0.4, 0.0, 0.6],
    [0.2, 0.5, 0.5, 0.4, 0.7, 0.2],
];

const CLOUD_OVER_KERNEL: &str = "
@group(0) @binding(0) var<storage, read> cases: array<vec4<f32>>;
@group(0) @binding(1) var<storage, read_write> values: array<f32>;

@compute @workgroup_size(1)
fn cloud_over_kernel(@builtin(global_invocation_id) id: vec3<u32>) {
    let row = cases[id.x * 2u];
    let depths = cases[id.x * 2u + 1u];
    values[id.x] = cloud_over(vec3<f32>(row.x), vec4<f32>(vec3<f32>(row.y), row.z), depths.x, depths.y).r;
}
";

/// A fullscreen triangle with the output of Bevy's fullscreen vertex shader, and a fragment of white
/// at half alpha.
const HALF_WHITE: &str = "
struct FullscreenVertexOutput {
    @builtin(position) position: vec4<f32>,
    @location(0) uv: vec2<f32>,
};

@vertex
fn fullscreen(@builtin(vertex_index) index: u32) -> FullscreenVertexOutput {
    let uv = vec2<f32>(f32(index >> 1u), f32(index & 1u)) * 2.0;
    var out: FullscreenVertexOutput;
    out.position = vec4<f32>(uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 0.0, 1.0);
    out.uv = uv;
    return out;
}

@fragment
fn half_white() -> @location(0) vec4<f32> {
    return vec4<f32>(1.0, 1.0, 1.0, 0.5);
}
";

/// The format of the world image, which the present pass encodes for an sRGB view target.
const WORLD_IMAGE: TextureFormat = TextureFormat::Rgba8Unorm;
const VIEW_TARGET: TextureFormat = TextureFormat::Rgba8UnormSrgb;
/// The row pitch a texture copy into a buffer must align to.
const COPY_ROW: u32 = 256;

struct Gpu {
    device: Device,
    queue: Queue,
}

impl Gpu {
    fn new() -> Option<Self> {
        let instance = Instance::new(InstanceDescriptor {
            backends: wgpu::Backends::PRIMARY,
            ..InstanceDescriptor::new_without_display_handle()
        });
        let adapter = block_on(instance.request_adapter(&RequestAdapterOptions {
            power_preference: PowerPreference::HighPerformance,
            ..Default::default()
        }))
        .ok()?;
        let (device, queue) = block_on(adapter.request_device(&DeviceDescriptor {
            label: Some("cull sections"),
            required_limits: adapter.limits(),
            ..Default::default()
        }))
        .ok()?;
        Some(Self { device, queue })
    }

    fn cull_sections(&self) -> ComputePipeline {
        let module = self.device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: ShaderSource::Wgsl(wgsl("cull", cull_defs(SECTION_SLOTS)).into()),
        });
        self.device
            .create_compute_pipeline(&ComputePipelineDescriptor {
                label: None,
                layout: None,
                module: &module,
                entry_point: Some("cull_sections"),
                compilation_options: PipelineCompilationOptions {
                    zero_initialize_workgroup_memory: true,
                    ..Default::default()
                },
                cache: None,
            })
    }

    fn buffer(&self, contents: &[u8], usage: BufferUsages) -> Buffer {
        self.device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents,
            usage,
        })
    }

    fn bind(
        &self,
        pipeline: &ComputePipeline,
        group: u32,
        entries: &[(u32, &Buffer)],
    ) -> BindGroup {
        let entries: Vec<BindGroupEntry<'_>> = entries
            .iter()
            .map(|&(binding, buffer)| BindGroupEntry {
                binding,
                resource: buffer.as_entire_binding(),
            })
            .collect();
        self.device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(group),
            entries: &entries,
        })
    }

    /// The bitset the kernel leaves for `run`, with every section reached by the cave walk and a
    /// frustum that holds everything.
    fn shown(&self, pipeline: &ComputePipeline, run: &Run) -> u64 {
        let mut camera = [0u32; CAMERA_WORDS];
        for plane in 0..FRUSTUM_PLANES {
            camera[FRUSTUM_WORD + plane * 4 + 3] = 1.0f32.to_bits();
        }
        for (axis, &coordinate) in CAMERA_SECTION.iter().enumerate() {
            camera[SECTION_WORD + axis] = bytemuck::cast(coordinate);
        }
        camera[RENDER_DISTANCE_WORD] = u32::from(run.render_distance);
        let mut sections = [[0i32; 5]; SECTION_SLOTS as usize];
        for (desc, column) in sections.iter_mut().step_by(COLUMN_STRIDE).zip(run.columns) {
            *desc = [
                CAMERA_SECTION[0] + column.offset[0],
                0,
                CAMERA_SECTION[2] + column.offset[1],
                1,
                0,
            ];
        }
        let mut cave = [0u32; 2 * CAVE_WORDS];
        cave[..CAVE_WORDS].fill(u32::MAX);

        let camera = self.buffer(bytemuck::cast_slice(&camera), BufferUsages::UNIFORM);
        let sections = self.buffer(bytemuck::cast_slice(&sections), BufferUsages::STORAGE);
        let cave = self.buffer(
            bytemuck::cast_slice(&cave),
            BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        );
        let read = self.device.create_buffer(&BufferDescriptor {
            label: None,
            size: cave.size(),
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let view = self.bind(pipeline, 0, &[(1, &camera)]);
        let terrain = self.bind(pipeline, 1, &[(3, &cave), (4, &sections)]);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(pipeline);
            pass.set_bind_group(0, &view, &[]);
            pass.set_bind_group(1, &terrain, &[]);
            pass.dispatch_workgroups(1, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&cave, 0, &read, 0, cave.size());
        self.queue.submit([encoder.finish()]);
        read.slice(..).map_async(MapMode::Read, |_| {});
        self.device.poll(PollType::wait_indefinitely()).unwrap();
        let words: Vec<u32> =
            bytemuck::cast_slice(&read.slice(..).get_mapped_range().unwrap()).to_vec();
        read.unmap();
        u64::from(words[CAVE_WORDS]) | (u64::from(words[CAVE_WORDS + 1]) << 32)
    }

    fn fog_values(&self) -> Vec<f32> {
        let cases: Vec<f32> = FOG_CASES
            .iter()
            .flat_map(|case| {
                [
                    case.offset[0],
                    case.offset[1],
                    case.offset[2],
                    0.0,
                    case.environmental[0],
                    case.environmental[1],
                    case.render_distance[0],
                    case.render_distance[1],
                ]
            })
            .collect();
        self.fog_kernel(FOG_KERNEL, "fog_value_kernel", &cases, FOG_CASES.len(), 1)
    }

    fn improved_pixels(&self, cases: &[ImprovedCase]) -> Vec<[f32; 4]> {
        let inputs: Vec<f32> = cases
            .iter()
            .flat_map(|case| {
                let behind = if case.cloud_behind { 1.0 } else { 0.0 };
                [
                    FACE[0],
                    FACE[1],
                    FACE[2],
                    0.0,
                    case.values[0],
                    case.values[1],
                    behind,
                    0.0,
                    NOON_FOG[0],
                    NOON_FOG[1],
                    NOON_FOG[2],
                    0.0,
                    SKY_BYTES[0],
                    SKY_BYTES[1],
                    SKY_BYTES[2],
                    0.0,
                ]
                .into_iter()
                .chain(case.cloud)
            })
            .collect();
        let pixels = self.fog_kernel(IMPROVED_KERNEL, "improved_fog", &inputs, cases.len(), 4);
        let (pixels, _) = pixels.as_chunks::<4>();
        pixels.to_vec()
    }

    fn cloud_over_values(&self) -> Vec<f32> {
        let inputs: Vec<f32> = CLOUD_OVER_CASES
            .iter()
            .flat_map(|&[m, cl, ca, cloud_depth, depth, _]| {
                [m, cl, ca, 0.0, cloud_depth, depth, 0.0, 0.0]
            })
            .collect();
        let cases = CLOUD_OVER_CASES.len();
        self.fog_kernel(CLOUD_OVER_KERNEL, "cloud_over_kernel", &inputs, cases, 1)
    }

    fn module(&self, source: String) -> ShaderModule {
        self.device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: ShaderSource::Wgsl(source.into()),
        })
    }

    fn fullscreen_pipeline(
        &self,
        vertex: &ShaderModule,
        fragment: &ShaderModule,
        entry: &str,
        target: ColorTargetState,
    ) -> RenderPipeline {
        self.device
            .create_render_pipeline(&RenderPipelineDescriptor {
                label: None,
                layout: None,
                vertex: VertexState {
                    module: vertex,
                    entry_point: Some("fullscreen"),
                    compilation_options: PipelineCompilationOptions::default(),
                    buffers: &[],
                },
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                fragment: Some(FragmentState {
                    module: fragment,
                    entry_point: Some(entry),
                    compilation_options: PipelineCompilationOptions::default(),
                    targets: &[Some(target)],
                }),
                multiview_mask: None,
                cache: None,
            })
    }

    fn pixel(&self, format: TextureFormat, usage: TextureUsages) -> wgpu::Texture {
        self.device.create_texture(&TextureDescriptor {
            label: None,
            size: Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: usage | TextureUsages::RENDER_ATTACHMENT,
            view_formats: &[],
        })
    }

    /// The red byte of the view target after white at half alpha is blended over black in the
    /// world image and the present pass encodes the world image for the target.
    fn presented_half_white(&self) -> u8 {
        let vertex = self.module(HALF_WHITE.to_owned());
        let present = self.module(wgsl("present", Vec::new()));
        let blended = self.fullscreen_pipeline(
            &vertex,
            &vertex,
            "half_white",
            ColorTargetState {
                format: WORLD_IMAGE,
                blend: Some(BlendState::ALPHA_BLENDING),
                write_mask: ColorWrites::ALL,
            },
        );
        let presented = self.fullscreen_pipeline(
            &vertex,
            &present,
            "present",
            ColorTargetState {
                format: VIEW_TARGET,
                blend: None,
                write_mask: ColorWrites::ALL,
            },
        );
        let world = self.pixel(WORLD_IMAGE, TextureUsages::TEXTURE_BINDING);
        let world = world.create_view(&Default::default());
        let target = self.pixel(VIEW_TARGET, TextureUsages::COPY_SRC);
        let target_view = target.create_view(&Default::default());
        let group = self.device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &presented.get_bind_group_layout(0),
            entries: &[BindGroupEntry {
                binding: 0,
                resource: BindingResource::TextureView(&world),
            }],
        });
        let read = self.device.create_buffer(&BufferDescriptor {
            label: None,
            size: u64::from(COPY_ROW),
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        let draw = |encoder: &mut wgpu::CommandEncoder,
                    view: &TextureView,
                    pipeline: &RenderPipeline,
                    group: Option<&BindGroup>| {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                color_attachments: &[Some(RenderPassColorAttachment {
                    view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(pipeline);
            if let Some(group) = group {
                pass.set_bind_group(0, group, &[]);
            }
            pass.draw(0..3, 0..1);
        };
        draw(&mut encoder, &world, &blended, None);
        draw(&mut encoder, &target_view, &presented, Some(&group));
        encoder.copy_texture_to_buffer(
            target.as_image_copy(),
            TexelCopyBufferInfo {
                buffer: &read,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(COPY_ROW),
                    rows_per_image: None,
                },
            },
            Extent3d {
                width: 1,
                height: 1,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        read.slice(..).map_async(MapMode::Read, |_| {});
        self.device.poll(PollType::wait_indefinitely()).unwrap();
        let red = read.slice(..).get_mapped_range().unwrap()[0];
        read.unmap();
        red
    }

    /// Runs `kernel` after `fog.wesl` in `invocations` workgroups over `inputs`, and reads back
    /// `width` floats of each.
    fn fog_kernel(
        &self,
        kernel: &str,
        entry: &str,
        inputs: &[f32],
        invocations: usize,
        width: usize,
    ) -> Vec<f32> {
        let source = format!(
            "{}{kernel}",
            include_str!("../../src/shaders/include/fog.wesl")
        );
        let module = self.device.create_shader_module(ShaderModuleDescriptor {
            label: None,
            source: ShaderSource::Wgsl(source.into()),
        });
        let pipeline = self
            .device
            .create_compute_pipeline(&ComputePipelineDescriptor {
                label: None,
                layout: None,
                module: &module,
                entry_point: Some(entry),
                compilation_options: PipelineCompilationOptions::default(),
                cache: None,
            });
        let cases = self.buffer(bytemuck::cast_slice(inputs), BufferUsages::STORAGE);
        let values = self.buffer(
            &vec![0; invocations * width * 4],
            BufferUsages::STORAGE | BufferUsages::COPY_SRC,
        );
        let read = self.device.create_buffer(&BufferDescriptor {
            label: None,
            size: values.size(),
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let group = self.bind(&pipeline, 0, &[(0, &cases), (1, &values)]);
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&Default::default());
            pass.set_pipeline(&pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.dispatch_workgroups(invocations as u32, 1, 1);
        }
        encoder.copy_buffer_to_buffer(&values, 0, &read, 0, values.size());
        self.queue.submit([encoder.finish()]);
        read.slice(..).map_async(MapMode::Read, |_| {});
        self.device.poll(PollType::wait_indefinitely()).unwrap();
        let values: Vec<f32> =
            bytemuck::cast_slice(&read.slice(..).get_mapped_range().unwrap()).to_vec();
        read.unmap();
        values
    }
}

#[test]
fn the_fog_value_of_an_offset_is_the_larger_of_its_two_ramps() {
    let Some(gpu) = Gpu::new() else {
        panic!("no adapter");
    };
    let values = gpu.fog_values();
    let wrong: Vec<([f32; 3], f32, f32)> = FOG_CASES
        .iter()
        .zip(values)
        .filter(|(case, value)| !value.is_finite() || (value - case.value).abs() > 1e-5)
        .map(|(case, value)| (case.offset, case.value, value))
        .collect();
    assert!(
        wrong.is_empty(),
        "(offset, fog value of the rule, fog value of the shader): {wrong:?}"
    );
}

#[test]
fn improved_transparency_fades_the_far_face_into_the_sky_and_the_clouds_behind_it() {
    let Some(gpu) = Gpu::new() else {
        panic!("no adapter");
    };
    let cloud_behind_at_192 = [0.843137, 0.899608, 1.0];
    let cases = [
        improved_case(AT_180, NO_CLOUD, false, NO_CLOUD_PIXEL),
        improved_case(AT_180, CLOUD, true, [0.535358, 0.630950, 0.4375]),
        improved_case(AT_180, CLOUD, false, NO_CLOUD_PIXEL),
        improved_case(AT_192, NO_CLOUD, false, SKY_BYTES),
        improved_case(AT_192, CLOUD, true, cloud_behind_at_192),
        improved_case(RAIN_AT_10, CLOUD, true, plain_fog_mix(RAIN_AT_10[0])),
    ];
    let pixels = gpu.improved_pixels(&cases);
    let wrong: Vec<([f32; 2], bool, [f32; 3], [f32; 4])> = cases
        .iter()
        .zip(pixels)
        .filter(|(case, pixel)| {
            case.pixel
                .iter()
                .zip(pixel)
                .any(|(expected, actual)| !actual.is_finite() || (expected - actual).abs() > 1e-4)
        })
        .map(|(case, pixel)| (case.values, case.cloud_behind, case.pixel, pixel))
        .collect();
    assert!(
        wrong.is_empty(),
        "(fog values, cloud behind, pixel of the rule, pixel of the shader): {wrong:?}"
    );
}

#[test]
fn the_cloud_image_goes_over_the_pixel_in_image_values_unless_a_face_is_nearer() {
    let Some(gpu) = Gpu::new() else {
        panic!("no adapter");
    };
    let values = gpu.cloud_over_values();
    let wrong: Vec<([f32; 6], f32)> = CLOUD_OVER_CASES
        .into_iter()
        .zip(values)
        .filter(|(case, value)| !value.is_finite() || (value - case[5]).abs() > 1e-5)
        .collect();
    assert!(wrong.is_empty(), "(case, value of the shader): {wrong:?}");
}

#[test]
fn the_section_cull_keeps_a_column_only_inside_the_render_distance() {
    let Some(gpu) = Gpu::new() else {
        panic!("no adapter");
    };
    let pipeline = gpu.cull_sections();
    let mut wrong = Vec::new();
    for run in &RUNS {
        let shown = gpu.shown(&pipeline, run);
        for (index, column) in run.columns.iter().enumerate() {
            if (shown >> (index * COLUMN_STRIDE)) & 1 != u64::from(column.drawn) {
                wrong.push((run.render_distance, column.offset, column.drawn));
            }
        }
    }
    assert!(
        wrong.is_empty(),
        "(r, offset, drawn by the rule) the kernel decided otherwise for: {wrong:?}"
    );
}

#[test]
fn a_blended_draw_mixes_image_values_and_the_present_pass_keeps_them() {
    let Some(gpu) = Gpu::new() else {
        panic!("no adapter");
    };
    let red = gpu.presented_half_white();
    assert!(
        red.abs_diff(128) <= 1,
        "white at half alpha over black shows {red}; the over blend of image values gives 128, \
         a blend in linear light 188"
    );
}
