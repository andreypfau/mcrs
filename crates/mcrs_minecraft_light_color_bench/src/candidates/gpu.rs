use std::collections::{BTreeMap, BTreeSet, HashMap};
use std::sync::{Mutex, OnceLock};
use std::time::{Duration, Instant};

use bevy_math::IVec3;
use futures_lite::future::block_on;
use mcrs_minecraft_core::{BlockPos, SectionPos};
use mcrs_minecraft_light_color::colors::LightType;
use mcrs_minecraft_light_color::propagate::Lanes;
use mcrs_minecraft_light_color::region::{EdgeCosts, Palette, REACH, Region, section_output};
use mcrs_minecraft_light_color::resolve::resolve;
use wgpu::util::{BufferInitDescriptor, DeviceExt};

use super::{Outcome, Stages};
use crate::fixture::{SECTIONS, SIDE, Scene};

/// Emission is at most 15 and every step costs at least 1, so after 15 waves
/// no level can rise.
const WAVES: usize = 15;
const WORKGROUP: u32 = 64;
const BRICK: usize = SectionPos::VOLUME;
pub const BRICK_BYTES: usize = BRICK * 4;
const WIDTH: i32 = SectionPos::SIZE as i32;
const PARAMS_WORDS: usize = 8 + 64;
pub const PARAMS_BYTES: usize = PARAMS_WORDS * 4;
const TIMESTAMP_BYTES: u64 = 2 * 8;

pub struct Gpu {
    pub adapter: String,
    device: wgpu::Device,
    queue: wgpu::Queue,
    gather: wgpu::ComputePipeline,
    waves: wgpu::ComputePipeline,
    cut: wgpu::ComputePipeline,
    timestamps: Mutex<wgpu::QuerySet>,
    period_ns: f64,
    uploaded: Mutex<HashMap<String, wgpu::Buffer>>,
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

/// Per section of a scene, x fastest then z then y, one word per cell: entry
/// cost in bits 0-3, the east, up and south vetoes in 4-6, emission in 8-11
/// and light type in 16-23.
pub struct Bricks {
    cells: Vec<u32>,
    /// Per section and light type, which of the 27 zones hold one of its
    /// emitters, splitting each axis into the first layer, the middle and the
    /// last layer, because a neighbour's region reaches all but its far layer.
    zones: Vec<BTreeMap<LightType, u32>>,
}

impl Bricks {
    pub fn new(scene: &Scene) -> Bricks {
        let (registry, colours) = (&scene.registry, &scene.colours);
        let scene_min = scene.origin.0 * WIDTH;
        let region = Region::new(
            BlockPos::new(
                scene_min.x + REACH,
                scene_min.y + REACH,
                scene_min.z + REACH,
            ),
            SIDE * WIDTH - 2 * REACH,
            scene.bounds,
            registry,
            scene.cells(),
        );
        let costs = EdgeCosts::new(&region, registry);

        let mut cells = vec![0u32; SECTIONS * BRICK];
        let mut zones = vec![BTreeMap::new(); SECTIONS];
        for (slot, zones) in zones.iter_mut().enumerate() {
            let base = (scene.section_at(slot).0 - scene.origin.0) * WIDTH;
            for local in 0..BRICK {
                let (x, z, y) = (local & 15, (local >> 4) & 15, local >> 8);
                let cell = region.index(base.x + x as i32, base.y + y as i32, base.z + z as i32);
                let id = region.blocks[cell];
                let entry = costs.entry[cell];
                assert!(entry <= 15, "entering {id:?} costs {entry}, past four bits");
                let emission = registry.emission(id).get();
                let t = colours.light_type(id);
                cells[slot * BRICK + local] = entry as u32
                    | (costs.veto[cell] as u32) << 4
                    | (emission as u32) << 8
                    | (t.0 as u32) << 16;
                if emission > 0 {
                    *zones.entry(t).or_default() |= 1 << (zone(x) + 3 * zone(z) + 9 * zone(y));
                }
            }
        }
        Bricks { cells, zones }
    }

    /// The light types with an emitter in the section's region, the same set
    /// `Palette::of` finds by walking the region's blocks.
    pub fn palette(&self, scene: &Scene, section: SectionPos) -> Vec<LightType> {
        let mut types = BTreeSet::new();
        for dy in -1..=1 {
            for dz in -1..=1 {
                for dx in -1..=1 {
                    let slot = scene
                        .slot(SectionPos(section.0 + IVec3::new(dx, dy, dz)))
                        .expect("an inner section's neighbours are in the scene");
                    let reached = reached_zones(dx, dy, dz);
                    types.extend(
                        self.zones[slot]
                            .iter()
                            .filter(|(_, zones)| *zones & reached != 0)
                            .map(|(&t, _)| t),
                    );
                }
            }
        }
        types.into_iter().collect()
    }
}

fn zone(local: usize) -> usize {
    match local {
        0 => 0,
        15 => 2,
        _ => 1,
    }
}

/// A region reaches `REACH + 1` cells past its section, which is every layer
/// of a neighbour but the far one.
fn reached_zones(dx: i32, dy: i32, dz: i32) -> u32 {
    let axis = |d: i32| match d {
        -1 => 0b110,
        0 => 0b111,
        _ => 0b011,
    };
    let (ax, ay, az) = (axis(dx), axis(dy), axis(dz));
    let mut reached = 0;
    for zy in 0..3 {
        for zz in 0..3 {
            for zx in 0..3 {
                if (ax >> zx) & (ay >> zy) & (az >> zz) & 1 != 0 {
                    reached |= 1 << (zx + 3 * zz + 9 * zy);
                }
            }
        }
    }
    reached
}

struct Propagated {
    levels: Vec<u8>,
    elapsed: Option<Duration>,
    round_trip: Duration,
    device_memory: usize,
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
            source: wgpu::ShaderSource::Wgsl(include_str!("gpu.wgsl").into()),
        });
        let pipeline = |entry_point| {
            device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
                label: Some(entry_point),
                layout: None,
                module: &module,
                entry_point: Some(entry_point),
                compilation_options: Default::default(),
                cache: None,
            })
        };
        let (gather, waves, cut) = (pipeline("gather"), pipeline("waves"), pipeline("cut"));
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
            gather,
            waves,
            cut,
            timestamps: Mutex::new(timestamps),
            period_ns,
            uploaded: Mutex::default(),
        };

        // Metal reads back zero timestamps around the first dispatch a device
        // runs, so that dispatch must not be a measured one.
        let bricks = gpu.device.create_buffer_init(&BufferInitDescriptor {
            label: Some("priming bricks"),
            contents: &[0; BRICK_BYTES],
            usage: wgpu::BufferUsages::STORAGE,
        });
        gpu.propagate(&bricks, &params(1, 1, 1, 0, IVec3::ZERO, &[]));
        Ok(gpu)
    }

    pub fn run(&self, scene: &Scene, section: SectionPos, stages: &mut Stages) -> Option<Outcome> {
        *stages = Stages::default();

        let clock = Instant::now();
        let bricks = Bricks::new(scene);
        let share = clock.elapsed() / SECTIONS as u32;
        let clock = Instant::now();
        let palette = Palette {
            types: bricks.palette(scene, section),
        };
        stages.snapshot = share + clock.elapsed();
        if palette.types.is_empty() {
            return None;
        }

        let uploaded = self
            .uploaded
            .lock()
            .unwrap()
            .entry(scene.name.clone())
            .or_insert_with(|| {
                self.device.create_buffer_init(&BufferInitDescriptor {
                    label: Some("bricks"),
                    contents: bytemuck::cast_slice(&bricks.cells),
                    usage: wgpu::BufferUsages::STORAGE,
                })
            })
            .clone();
        let words = palette.types.len().div_ceil(4);
        let base = (section.0 - scene.origin.0) * WIDTH - (REACH + 1);
        let (min, size) = section_output(section);
        let propagated = self.propagate(
            &uploaded,
            &params(size + 2 * REACH, words, SIDE, REACH, base, &palette.types),
        );
        stages.propagation = propagated.elapsed.expect("the pass has ordered timestamps");
        stages.device_memory = propagated.device_memory + BRICK_BYTES;
        stages.round_trip = propagated.round_trip;

        let clock = Instant::now();
        let lanes = Lanes {
            bytes: 4 * words,
            levels: propagated.levels.into_boxed_slice(),
        };
        let output = Region {
            min,
            size,
            blocks: Box::default(),
        };
        let texels = resolve(&output, &lanes, &palette, &scene.colours, min, size);
        stages.resolve = clock.elapsed();

        let cells = output.size.pow(3) as usize;
        let lanes = palette
            .types
            .iter()
            .enumerate()
            .map(|(lane, &t)| (t, (0..cells).map(|c| lanes.level(c, lane)).collect()))
            .collect();
        Some(Outcome {
            texels,
            lanes: Some(lanes),
            undetermined: 0,
        })
    }

    fn propagate(&self, bricks: &wgpu::Buffer, params: &[u32]) -> Propagated {
        let clock = Instant::now();
        let device = &self.device;
        let [size, words, _, reach, ..] = params[..] else {
            unreachable!()
        };
        let (size, words) = (size as u64, words as u64);
        let out = size - 2 * reach as u64;
        let (cells, output_cells) = (size.pow(3), out.pow(3));
        assert!(
            (cells * words).div_ceil(WORKGROUP as u64) <= u16::MAX as u64,
            "{words} words per cell exceed one dispatch"
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
        let params = device.create_buffer_init(&BufferInitDescriptor {
            label: Some("params"),
            contents: bytemuck::cast_slice(params),
            usage: wgpu::BufferUsages::UNIFORM,
        });
        let cost = buffer("cost", cells * 4, storage);
        let ping = buffer("ping", cells * words * 4, storage);
        let pong = buffer("pong", cells * words * 4, storage);
        let output = buffer(
            "output",
            output_cells * words * 4,
            storage | wgpu::BufferUsages::COPY_SRC,
        );
        let bind = |pipeline: &wgpu::ComputePipeline, entries: &[(u32, &wgpu::Buffer)]| {
            let entries: Vec<_> = entries
                .iter()
                .map(|&(binding, buffer)| wgpu::BindGroupEntry {
                    binding,
                    resource: buffer.as_entire_binding(),
                })
                .collect();
            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: None,
                layout: &pipeline.get_bind_group_layout(0),
                entries: &entries,
            })
        };
        let gather = bind(
            &self.gather,
            &[(0, &params), (1, bricks), (2, &cost), (4, &ping)],
        );
        let forth = bind(
            &self.waves,
            &[(0, &params), (2, &cost), (3, &ping), (4, &pong)],
        );
        let back = bind(
            &self.waves,
            &[(0, &params), (2, &cost), (3, &pong), (4, &ping)],
        );
        let last = if WAVES % 2 == 1 { &pong } else { &ping };
        let cut = bind(&self.cut, &[(0, &params), (3, last), (5, &output)]);

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
            let groups = |n: u64| n.div_ceil(WORKGROUP as u64) as u32;
            pass.set_pipeline(&self.gather);
            pass.set_bind_group(0, &gather, &[]);
            pass.dispatch_workgroups(groups(cells), 1, 1);
            pass.set_pipeline(&self.waves);
            for wave in 0..WAVES {
                pass.set_bind_group(0, if wave % 2 == 0 { &forth } else { &back }, &[]);
                pass.dispatch_workgroups(groups(cells * words), 1, 1);
            }
            pass.set_pipeline(&self.cut);
            pass.set_bind_group(0, &cut, &[]);
            pass.dispatch_workgroups(groups(output_cells * words), 1, 1);
        }
        let resolved = buffer(
            "timestamps",
            TIMESTAMP_BYTES,
            wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        );
        encoder.resolve_query_set(&timestamps, 0..2, &resolved, 0);
        let times = self.readback(&mut encoder, &resolved);
        let levels = self.readback(&mut encoder, &output);
        self.queue.submit([encoder.finish()]);
        let device_memory = [
            &params, &cost, &ping, &pong, &output, &resolved, &times, &levels,
        ]
        .iter()
        .map(|b| b.size() as usize)
        .sum();

        let levels = self.map(&levels);
        let times: Vec<u64> = bytemuck::cast_slice(&self.map(&times)).to_vec();
        let round_trip = clock.elapsed();
        drop(timestamps);
        let elapsed = (times[1] > times[0])
            .then(|| Duration::from_nanos(((times[1] - times[0]) as f64 * self.period_ns) as u64));
        Propagated {
            levels,
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

/// The shader's `Params`: sizes, the region's first cell relative to the
/// scene's, and a byte per light type naming its lane, `0xff` for none.
fn params(
    size: i32,
    words: usize,
    side: i32,
    reach: i32,
    base: IVec3,
    types: &[LightType],
) -> Vec<u32> {
    let mut lanes = [0xffu8; 256];
    for (lane, t) in types.iter().enumerate() {
        lanes[t.0 as usize] = lane as u8;
    }
    let base = base.as_uvec3();
    let mut params = vec![
        size as u32,
        words as u32,
        side as u32,
        reach as u32,
        base.x,
        base.y,
        base.z,
        0,
    ];
    params.extend(
        lanes
            .chunks(4)
            .map(|b| u32::from_le_bytes(b.try_into().unwrap())),
    );
    debug_assert_eq!(params.len(), PARAMS_WORDS);
    params
}
