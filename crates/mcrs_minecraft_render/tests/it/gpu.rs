use bevy::tasks::block_on;
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, Buffer, BufferDescriptor, BufferUsages,
    ComputePipeline, ComputePipelineDescriptor, Device, DeviceDescriptor, Instance,
    InstanceDescriptor, MapMode, PipelineCompilationOptions, PollType, PowerPreference, Queue,
    RequestAdapterOptions, ShaderModuleDescriptor, ShaderSource,
};

use crate::world_shaders::{cull_defs, wgsl};

const SECTION_SLOTS: u32 = 64;
const CAVE_WORDS: usize = 2;
/// Slots between two columns of a run, so a run reaches the second word of the bitset.
const COLUMN_STRIDE: usize = 4;
/// Word places in the `Camera` uniform of `frame.wesl`.
const CAMERA_WORDS: usize = 56;
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
