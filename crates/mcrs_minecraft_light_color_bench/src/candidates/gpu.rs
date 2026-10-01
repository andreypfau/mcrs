use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use futures_lite::future::block_on;
use mcrs_minecraft_core::SectionPos;
use mcrs_minecraft_light_color::colors::LightType;
use mcrs_minecraft_light_color::layout::{
    Emitter, JOB_WORDS, Lane, REGION_CELLS, REGION_SIDE, SLOT_ABOVE, SLOT_UNLOADED, WAVES,
    job_words, lane_words, lanes, neighbours, pack, reaching_types,
};
use mcrs_minecraft_light_color::region::{REACH, section_bricks};
use wgpu::util::{BufferInitDescriptor, DeviceExt};

use super::{Outcome, Stages};
use crate::fixture::{SECTIONS, Scene};

const WORKGROUP: u32 = 64;
pub const BRICK_BYTES: usize = SectionPos::VOLUME * 4;
pub const JOB_BYTES: usize = JOB_WORDS * 4;
const OUTPUT: u32 = SectionPos::SIZE as u32 + 2;
const OUTPUT_FIRST: u32 = REACH as u32;
const TEXEL_BYTES: u32 = 4;
const TIMESTAMP_BYTES: u64 = 2 * 8;

pub struct Gpu {
    pub adapter: String,
    device: wgpu::Device,
    queue: wgpu::Queue,
    layout: wgpu::BindGroupLayout,
    gather: wgpu::ComputePipeline,
    waves: wgpu::ComputePipeline,
    resolve: wgpu::ComputePipeline,
    timestamps: Mutex<wgpu::QuerySet>,
    period_ns: f64,
    uploaded: Mutex<Option<(Vec<u32>, wgpu::Buffer)>>,
}

pub fn gpu() -> &'static Gpu {
    static GPU: OnceLock<Result<Gpu, String>> = OnceLock::new();
    GPU.get_or_init(Gpu::new)
        .as_ref()
        .unwrap_or_else(|e| panic!("{e}"))
}

pub fn run(scene: &Scene, section: SectionPos, stages: &mut Stages) -> Option<Outcome> {
    gpu().run(scene, section, stages)
}

pub fn run_batch(scene: &Scene, sections: &[SectionPos]) -> Vec<Option<Outcome>> {
    gpu().run_batch(scene, sections)
}

/// The scene's sections as the renderer holds them: a brick per loaded section
/// and per section below the world, and a sentinel slot for the rest.
pub struct Pool {
    words: Vec<u32>,
    emitters: Vec<Vec<Emitter>>,
    slots: Vec<u32>,
}

impl Pool {
    pub fn new(scene: &Scene) -> Pool {
        let mut pool = Pool {
            words: Vec::new(),
            emitters: Vec::new(),
            slots: Vec::with_capacity(SECTIONS),
        };
        for slot in 0..SECTIONS {
            let section = scene.section_at(slot);
            let pool_slot = if section.y > scene.bounds.max_section_y {
                SLOT_ABOVE
            } else if section.y >= scene.bounds.min_section_y && scene.blocks[slot].is_none() {
                SLOT_UNLOADED
            } else {
                let brick = pack(&section_bricks(
                    section,
                    scene.bounds,
                    &scene.registry,
                    &scene.colours,
                    scene.cells(),
                ));
                pool.words.extend_from_slice(&brick.words);
                pool.emitters.push(brick.emitters);
                (pool.emitters.len() - 1) as u32
            };
            pool.slots.push(pool_slot);
        }
        pool
    }

    fn slot(&self, scene: &Scene, section: SectionPos) -> u32 {
        self.slots[scene
            .slot(section)
            .expect("a job's neighbours are in the scene")]
    }

    fn emitters(&self, scene: &Scene, section: SectionPos) -> Option<&[Emitter]> {
        self.emitters
            .get(self.slot(scene, section) as usize)
            .map(Vec::as_slice)
    }
}

struct Job {
    lanes: Vec<Lane>,
    lane_words: u32,
    lane_base: u32,
    atlas_origin: [u32; 3],
    words: [u32; JOB_WORDS],
}

struct Jobs {
    list: Vec<Job>,
    of_section: Vec<Option<usize>>,
    atlas_side: u32,
}

impl Jobs {
    fn plan(pool: &Pool, scene: &Scene, sections: &[SectionPos]) -> Jobs {
        let palettes: Vec<Vec<LightType>> = sections
            .iter()
            .map(|&section| reaching_types(section, |n| pool.emitters(scene, n)))
            .collect();
        let count = palettes.iter().filter(|p| !p.is_empty()).count() as u32;
        let mut atlas_side = 1;
        while atlas_side * atlas_side * atlas_side < count {
            atlas_side += 1;
        }
        let (mut list, mut of_section, mut lane_base) = (Vec::new(), Vec::new(), 0u32);
        for (&section, types) in sections.iter().zip(&palettes) {
            if types.is_empty() {
                of_section.push(None);
                continue;
            }
            let lanes = lanes(types, &scene.colours);
            let lane_words = lane_words(lanes.len());
            let i = list.len() as u32;
            let atlas_origin = [
                i % atlas_side,
                i / atlas_side % atlas_side,
                i / (atlas_side * atlas_side),
            ]
            .map(|c| c * OUTPUT);
            let mut slots = [0u32; 27];
            for (slot, neighbour) in slots.iter_mut().zip(neighbours(section)) {
                *slot = pool.slot(scene, neighbour);
            }
            let words = job_words(&lanes, slots, lane_base, atlas_origin);
            of_section.push(Some(list.len()));
            list.push(Job {
                lanes,
                lane_words,
                lane_base,
                atlas_origin,
                words,
            });
            lane_base += REGION_CELLS as u32 * lane_words;
        }
        Jobs {
            list,
            of_section,
            atlas_side,
        }
    }
}

struct Dispatched {
    lanes: Vec<u8>,
    texels: Vec<u8>,
    texel_row_bytes: u32,
    atlas_extent: u32,
    elapsed: Option<Duration>,
    round_trip: Duration,
    device_memory: usize,
}

impl Dispatched {
    fn outcome(&self, job: &Job) -> Outcome {
        let texels = output_cells()
            .map(|[x, y, z]| {
                let [ox, oy, oz] = job.atlas_origin;
                let at = ((oz + z) * self.atlas_extent + oy + y) * self.texel_row_bytes
                    + (ox + x) * TEXEL_BYTES;
                let at = at as usize;
                self.texels[at..at + 4].try_into().unwrap()
            })
            .collect();
        let lanes = job
            .lanes
            .iter()
            .enumerate()
            .map(|(lane, l)| {
                let levels = output_cells()
                    .map(|cell| {
                        let [x, y, z] = cell.map(|c| c + OUTPUT_FIRST);
                        let region_cell = x + REGION_SIDE * (z + REGION_SIDE * y);
                        let word = job.lane_base + region_cell * job.lane_words + lane as u32 / 4;
                        self.lanes[word as usize * 4 + lane % 4]
                    })
                    .collect();
                (l.light_type, levels)
            })
            .collect();
        Outcome {
            texels,
            lanes: Some(lanes),
        }
    }
}

/// `[x, y, z]` of each output cell, x fastest then z then y.
fn output_cells() -> impl Iterator<Item = [u32; 3]> {
    (0..OUTPUT).flat_map(|y| (0..OUTPUT).flat_map(move |z| (0..OUTPUT).map(move |x| [x, y, z])))
}

impl Gpu {
    pub fn new() -> Result<Gpu, String> {
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::METAL,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let adapter = block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: None,
        }))
        .map_err(|e| format!("no Metal adapter: {e}"))?;
        let name = adapter.get_info().name;
        if !adapter.features().contains(wgpu::Features::TIMESTAMP_QUERY) {
            return Err(format!(
                "{name} lacks TIMESTAMP_QUERY, so passes cannot be timed"
            ));
        }
        let (device, queue) = block_on(adapter.request_device(&wgpu::DeviceDescriptor {
            label: Some("light colour"),
            required_features: wgpu::Features::TIMESTAMP_QUERY,
            ..Default::default()
        }))
        .map_err(|e| format!("{name}: {e}"))?;

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("light colour"),
            source: wgpu::ShaderSource::Wgsl(
                include_str!("../../../mcrs_minecraft_render_light_color/src/shaders/colour.wgsl")
                    .into(),
            ),
        });
        let storage = |binding, read_only| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::COMPUTE,
            ty: wgpu::BindingType::Buffer {
                ty: wgpu::BufferBindingType::Storage { read_only },
                has_dynamic_offset: false,
                min_binding_size: None,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("light colour"),
            entries: &[
                storage(0, true),
                storage(1, true),
                storage(2, false),
                storage(3, true),
                storage(4, false),
                wgpu::BindGroupLayoutEntry {
                    binding: 5,
                    visibility: wgpu::ShaderStages::COMPUTE,
                    ty: wgpu::BindingType::StorageTexture {
                        access: wgpu::StorageTextureAccess::WriteOnly,
                        format: wgpu::TextureFormat::Rgba8Unorm,
                        view_dimension: wgpu::TextureViewDimension::D3,
                    },
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("light colour"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let pipeline = |entry_point| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry_point),
                layout: Some(&pipeline_layout),
                module: &module,
                entry_point: Some(entry_point),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let (gather, waves, resolve) = (pipeline("gather"), pipeline("waves"), pipeline("resolve"));
        let timestamps = device.create_query_set(&wgpu::QuerySetDescriptor {
            label: Some("light colour pass"),
            ty: wgpu::QueryType::Timestamp,
            count: 2,
        });
        let period_ns = queue.get_timestamp_period() as f64;
        let gpu = Gpu {
            adapter: name,
            device,
            queue,
            layout,
            gather,
            waves,
            resolve,
            timestamps: Mutex::new(timestamps),
            period_ns,
            uploaded: Mutex::default(),
        };

        // Metal reads back zero timestamps around the first dispatch a device
        // runs, so that dispatch must not be a measured one.
        let pool = gpu.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("priming bricks"),
            contents: &[0; BRICK_BYTES],
            usage: wgpu::BufferUsages::STORAGE,
        });
        let lanes = [Lane {
            light_type: LightType::DEFAULT,
            colour: None,
        }];
        let priming = Jobs {
            list: vec![Job {
                words: job_words(&lanes, [SLOT_ABOVE; 27], 0, [0; 3]),
                lanes: lanes.to_vec(),
                lane_words: 1,
                lane_base: 0,
                atlas_origin: [0; 3],
            }],
            of_section: vec![Some(0)],
            atlas_side: 1,
        };
        gpu.dispatch(&pool, &priming);
        Ok(gpu)
    }

    pub fn run(&self, scene: &Scene, section: SectionPos, stages: &mut Stages) -> Option<Outcome> {
        *stages = Stages::default();

        let clock = Instant::now();
        let pool = Pool::new(scene);
        let share = clock.elapsed() / SECTIONS as u32;
        let clock = Instant::now();
        let jobs = Jobs::plan(&pool, scene, &[section]);
        stages.snapshot = share + clock.elapsed();
        let job = jobs.of_section[0]?;

        let dispatched = self.dispatch(&self.upload(pool.words), &jobs);
        stages.propagation = dispatched.elapsed.expect("the pass has ordered timestamps");
        stages.device_memory = dispatched.device_memory + BRICK_BYTES;
        stages.round_trip = dispatched.round_trip;
        Some(dispatched.outcome(&jobs.list[job]))
    }

    /// Every section some light reaches as one job of a single dispatch per stage.
    pub fn run_batch(&self, scene: &Scene, sections: &[SectionPos]) -> Vec<Option<Outcome>> {
        let pool = Pool::new(scene);
        let jobs = Jobs::plan(&pool, scene, sections);
        if jobs.list.is_empty() {
            return sections.iter().map(|_| None).collect();
        }
        let dispatched = self.dispatch(&self.upload(pool.words), &jobs);
        jobs.of_section
            .iter()
            .map(|job| job.map(|job| dispatched.outcome(&jobs.list[job])))
            .collect()
    }

    fn upload(&self, words: Vec<u32>) -> wgpu::Buffer {
        let mut uploaded = self.uploaded.lock().unwrap();
        match &*uploaded {
            Some((cells, buffer)) if *cells == words => buffer.clone(),
            _ => {
                let buffer = self.device.create_buffer_init(&BufferInitDescriptor {
                    label: Some("bricks"),
                    contents: bytemuck::cast_slice(&words),
                    usage: wgpu::BufferUsages::STORAGE,
                });
                *uploaded = Some((words, buffer.clone()));
                buffer
            }
        }
    }

    fn dispatch(&self, pool: &wgpu::Buffer, jobs: &Jobs) -> Dispatched {
        let clock = Instant::now();
        let device = &self.device;
        let count = jobs.list.len() as u32;
        let max_words = jobs.list.iter().map(|j| j.lane_words).max().unwrap_or(0);
        let lane_words: u32 = jobs.list.iter().map(|j| j.lane_words).sum();
        let groups = |n: u64| n.div_ceil(WORKGROUP as u64) as u32;
        assert!(
            count <= u16::MAX as u32
                && groups(REGION_CELLS as u64 * max_words as u64) <= u16::MAX as u32,
            "{count} jobs of up to {max_words} lane words exceed one dispatch"
        );

        let buffer = |label, bytes: u64, usage| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: Some(label),
                size: bytes.max(4),
                usage,
                mapped_at_creation: false,
            })
        };
        let storage = wgpu::BufferUsages::STORAGE;
        let records: Vec<u32> = jobs.list.iter().flat_map(|j| j.words).collect();
        let records = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("jobs"),
            contents: bytemuck::cast_slice(&records),
            usage: storage,
        });
        let cost = buffer("cost", count as u64 * REGION_CELLS as u64 * 4, storage);
        let lane_bytes = REGION_CELLS as u64 * lane_words as u64 * 4;
        let lanes = [
            buffer("lanes", lane_bytes, storage | wgpu::BufferUsages::COPY_SRC),
            buffer("lanes", lane_bytes, storage | wgpu::BufferUsages::COPY_SRC),
        ];
        let atlas_extent = OUTPUT * jobs.atlas_side;
        let atlas = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("colour atlas"),
            size: wgpu::Extent3d {
                width: atlas_extent,
                height: atlas_extent,
                depth_or_array_layers: atlas_extent,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D3,
            format: wgpu::TextureFormat::Rgba8Unorm,
            usage: wgpu::TextureUsages::STORAGE_BINDING | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let atlas_view = atlas.create_view(&Default::default());
        // Group `g` reads `lanes[1 - g]` and writes `lanes[g]`.
        let groups_of = [0, 1].map(|g| {
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &self.layout,
                entries: &[
                    (0, records.as_entire_binding()),
                    (1, pool.as_entire_binding()),
                    (2, cost.as_entire_binding()),
                    (3, lanes[1 - g].as_entire_binding()),
                    (4, lanes[g].as_entire_binding()),
                    (5, wgpu::BindingResource::TextureView(&atlas_view)),
                ]
                .map(|(binding, resource)| wgpu::BindGroupEntry { binding, resource }),
            })
        });

        let timestamps = self.timestamps.lock().unwrap();
        let mut encoder = device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
                label: Some("light colour"),
                timestamp_writes: Some(wgpu::ComputePassTimestampWrites {
                    query_set: &timestamps,
                    beginning_of_pass_write_index: Some(0),
                    end_of_pass_write_index: Some(1),
                }),
            });
            pass.set_pipeline(&self.gather);
            pass.set_bind_group(0, &groups_of[0], &[]);
            pass.dispatch_workgroups(groups(REGION_CELLS as u64), count, 1);
            pass.set_pipeline(&self.waves);
            for wave in 0..WAVES {
                pass.set_bind_group(0, &groups_of[(wave + 1) % 2], &[]);
                pass.dispatch_workgroups(groups(REGION_CELLS as u64 * max_words as u64), count, 1);
            }
            pass.set_pipeline(&self.resolve);
            pass.set_bind_group(0, &groups_of[(WAVES + 1) % 2], &[]);
            pass.dispatch_workgroups(groups(OUTPUT.pow(3) as u64), count, 1);
        }
        let resolved = buffer(
            "timestamps",
            TIMESTAMP_BYTES,
            wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        );
        encoder.resolve_query_set(&timestamps, 0..2, &resolved, 0);
        let times = self.readback(&mut encoder, &resolved);
        let final_lanes = self.readback(&mut encoder, &lanes[WAVES % 2]);
        let texel_row_bytes =
            (atlas_extent * TEXEL_BYTES).next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT);
        let texels = buffer(
            "texels",
            texel_row_bytes as u64 * atlas_extent as u64 * atlas_extent as u64,
            wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
        );
        encoder.copy_texture_to_buffer(
            wgpu::TexelCopyTextureInfo {
                texture: &atlas,
                mip_level: 0,
                origin: wgpu::Origin3d::ZERO,
                aspect: wgpu::TextureAspect::All,
            },
            wgpu::TexelCopyBufferInfo {
                buffer: &texels,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(texel_row_bytes),
                    rows_per_image: Some(atlas_extent),
                },
            },
            atlas.size(),
        );
        self.queue.submit([encoder.finish()]);
        let device_memory = [
            &records,
            &cost,
            &lanes[0],
            &lanes[1],
            &resolved,
            &times,
            &final_lanes,
            &texels,
        ]
        .iter()
        .map(|b| b.size() as usize)
        .sum::<usize>()
            + atlas_extent.pow(3) as usize * TEXEL_BYTES as usize;

        let lanes = self.map(&final_lanes);
        let texels = self.map(&texels);
        let times: Vec<u64> = bytemuck::cast_slice(&self.map(&times)).to_vec();
        let round_trip = clock.elapsed();
        drop(timestamps);
        let elapsed = (times[1] > times[0])
            .then(|| Duration::from_nanos(((times[1] - times[0]) as f64 * self.period_ns) as u64));
        Dispatched {
            lanes,
            texels,
            texel_row_bytes,
            atlas_extent,
            elapsed,
            round_trip,
            device_memory,
        }
    }

    fn readback(&self, encoder: &mut wgpu::CommandEncoder, source: &wgpu::Buffer) -> wgpu::Buffer {
        let target = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("readback"),
            size: source.size(),
            usage: wgpu::BufferUsages::MAP_READ | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        encoder.copy_buffer_to_buffer(source, 0, &target, 0, source.size());
        target
    }

    fn map(&self, buffer: &wgpu::Buffer) -> Vec<u8> {
        let slice = buffer.slice(..);
        slice.map_async(wgpu::MapMode::Read, |result| {
            result.expect("a readback buffer maps");
        });
        self.device
            .poll(wgpu::PollType::wait_indefinitely())
            .expect("the device finishes its work");
        let bytes = slice.get_mapped_range().to_vec();
        buffer.unmap();
        bytes
    }
}
