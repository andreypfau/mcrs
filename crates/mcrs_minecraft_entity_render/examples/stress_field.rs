//! The stress scene S1 of `docs/entity-render-budget.md`, section 5.1: 100,000 pigs on a field
//! of 316 × 316 blocks, one pig on each block, 10% with a saddle, half of them walking. The
//! camera stands at a corner at the eye height 1.62 and looks along the diagonal, at 2560 × 1440.
//!
//! ```sh
//! cargo run --release -p mcrs_minecraft_entity_render --example stress_field
//! ```
//!
//! It times each compute pass of the entity passes with pass timestamps. It also prints the
//! difference of the wall time of frames with and without the draw, which does not measure the
//! draw on Metal; `tools/metal-trace` does (section 4.15). Each frame starts with a
//! compute pass of arithmetic, `MCRS_GPU_HOT` workgroups (16384 by default), that holds the GPU
//! at its clock, as `PERF.md` measures. It writes the last counter of the entity passes, so the
//! passes wait for it. It also times the CPU writes of the slot records in a tick and their
//! upload.
//!
//! Every pass has a label, so `tools/metal-trace` reads the GPU time of each one from a Metal
//! System Trace: `gbuffer entities` and `gbuffer no entities` alternate. With `MCRS_SPLIT_DRAW`
//! the draw is two passes, `gbuffer full rigs` and `gbuffer impostors`. `MCRS_DRAW_ONLY=full_rigs`
//! or `impostors` leaves the other draw out of `gbuffer entities`.
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "a developer tool reports a failure by panicking"
)]
#![expect(
    clippy::disallowed_methods,
    reason = "a developer tool takes the heater size from the environment"
)]

#[path = "../tests/support/mod.rs"]
#[expect(dead_code, reason = "the example uses a part of the test support")]
mod support;

use std::time::Instant;

use glam::{Mat4, Vec2, Vec3};
use mcrs_minecraft_core::CardinalLight;
use mcrs_minecraft_entity_render::{
    CULL_VALID, Capacities, ComputePass, CullRecord, EntityFrameCounts, EntityGpu, EntityLibrary,
    EntityPack, FULL_RIG_CAP, FrameUniform, HeightThreshold, PassQueries, SlotArena, StateRecord,
    TextureImage, TickRecord, f16_up,
};
use mcrs_minecraft_render::{CameraOrigin, FrameView};

const SIDE: i32 = 316;
const WIDTH: u32 = 2560;
const HEIGHT: u32 = 1440;
const FRAMES_PER_TICK: u32 = 3;
const WARMUP: u32 = 60;
const MEASURED: u32 = 256;

struct Pig {
    slot: u32,
    walking: bool,
    position: Vec3,
    heading: f32,
    walk_position: f32,
    walk_speed: f32,
}

fn random(state: &mut u64) -> f32 {
    *state ^= *state << 13;
    *state ^= *state >> 7;
    *state ^= *state << 17;
    (*state >> 40) as f32 / (1u64 << 24) as f32
}

fn texture(identifier: &str, side: u32) -> TextureImage {
    TextureImage {
        identifier: identifier.into(),
        width: side,
        height: side,
        rgba: (0..side * side)
            .flat_map(|texel| [200, 120 + (texel % 7) as u8, 130, 255])
            .collect(),
    }
}

fn record(pig: &Pig) -> TickRecord {
    let block = pig.position.floor();
    let fraction = ((pig.position - block) * 65536.0)
        .as_uvec3()
        .min(glam::UVec3::splat(65535));
    TickRecord {
        block: block.as_ivec3().to_array(),
        fraction: [fraction.x as u16, fraction.y as u16, fraction.z as u16],
        body_yaw: TickRecord::turn_from_degrees(pig.heading),
        head_yaw: TickRecord::turn_from_degrees(pig.heading + 20.0),
        pitch: 0,
        walk_position: pig.walk_position.to_bits(),
        walk_speed: pig.walk_speed.to_bits(),
    }
}

/// The pigs of the field, one on each block, with their slot records.
fn spawn(
    definition: u16,
    body: u16,
    saddle: u16,
    tick: u32,
    seed: &mut u64,
) -> (SlotArena, Vec<Pig>) {
    let mut arena = SlotArena::new();
    let mut pigs = Vec::new();
    for x in 0..SIDE {
        for z in 0..SIDE {
            if pigs.len() == 100_000 {
                break;
            }
            let slot = arena.allocate();
            let saddled = random(seed) < 0.1;
            let pig = Pig {
                slot,
                walking: random(seed) < 0.5,
                position: Vec3::new(x as f32 + 0.5, 64.0, z as f32 + 0.5),
                heading: random(seed) * 360.0,
                walk_position: 0.0,
                walk_speed: 0.0,
            };
            arena.write_tick(slot, tick, record(&pig), true);
            arena.write_cull(
                slot,
                CullRecord {
                    block: pig.position.floor().as_ivec3().to_array(),
                    radius: f16_up(1.6),
                    flags: CULL_VALID,
                },
            );
            arena.write_state(
                slot,
                StateRecord {
                    definition,
                    layer_mask: if saddled { 3 } else { 1 },
                    layers: [[body, 0], [saddle, 1], [0; 2], [0; 2]],
                    hurt_start: -1e9,
                    scale: f16_up(1.0),
                    block_light: 15,
                    sky_light: 15,
                    section: u32::MAX,
                    clock_offset: 0,
                    input: u32::MAX,
                    ..StateRecord::default()
                },
            );
            pigs.push(pig);
        }
    }

    (arena, pigs)
}

/// The gbuffer targets and the depth target of the frame.
fn gbuffer(device: &wgpu::Device) -> Vec<wgpu::TextureView> {
    [
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Rgba16Float,
        wgpu::TextureFormat::Rgba16Float,
        wgpu::TextureFormat::Depth32Float,
    ]
    .iter()
    .map(|format| {
        device
            .create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: WIDTH,
                    height: HEIGHT,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format: *format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
                view_formats: &[],
            })
            .create_view(&Default::default())
    })
    .collect()
}

/// One tick of the walking pigs: a step along a seeded path.
fn walk(pigs: &mut [Pig], seed: &mut u64) {
    for pig in pigs.iter_mut().filter(|pig| pig.walking) {
        let step = 0.1;
        pig.heading += (random(seed) - 0.5) * 10.0;
        let direction = pig.heading.to_radians();
        pig.position.x = (pig.position.x - direction.sin() * step).clamp(0.0, SIDE as f32);
        pig.position.z = (pig.position.z + direction.cos() * step).clamp(0.0, SIDE as f32);
        pig.walk_speed += (0.4 - pig.walk_speed) * 0.4;
        pig.walk_position += pig.walk_speed;
    }
}

/// The records of the pigs that moved in a tick, as the tick systems of the client write them.
fn write_records(pigs: &[Pig], arena: &mut SlotArena, tick: u32) {
    for pig in pigs.iter().filter(|pig| pig.walking) {
        arena.write_tick(pig.slot, tick, record(pig), false);
        arena.write_cull(
            pig.slot,
            CullRecord {
                block: pig.position.floor().as_ivec3().to_array(),
                radius: f16_up(1.6),
                flags: CULL_VALID,
            },
        );
    }
}

/// The camera at the corner of the field, at the eye height, along the diagonal.
fn camera() -> FrameView {
    let eye = Vec3::new(0.0, 64.0 + 1.62, 0.0);
    let section = (eye / 16.0).floor().as_ivec3();
    let offset = eye - section.as_vec3() * 16.0;
    let projection = Mat4::perspective_infinite_reverse_rh(
        70f32.to_radians(),
        WIDTH as f32 / HEIGHT as f32,
        0.05,
    );
    let view = Mat4::look_to_rh(offset, Vec3::new(1.0, -0.05, 1.0).normalize(), Vec3::Y);
    FrameView::new(
        projection,
        view,
        CameraOrigin { section, offset },
        Vec2::new(WIDTH as f32, HEIGHT as f32),
    )
}

/// Which entity draws a frame makes, each in its own labelled render pass so that a Metal
/// System Trace times it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum EntityDraw {
    None,
    All,
    /// The full rigs and the impostors in two passes; the second pass loads the targets.
    Split,
}

/// A gbuffer render pass on `targets`, cleared or loaded.
fn gbuffer_pass<'a>(
    encoder: &'a mut wgpu::CommandEncoder,
    targets: &'a [wgpu::TextureView],
    label: &str,
    clear: bool,
) -> wgpu::RenderPass<'a> {
    let colors: Vec<Option<wgpu::RenderPassColorAttachment<'_>>> = targets[..3]
        .iter()
        .map(|view| {
            Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: if clear {
                        wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT)
                    } else {
                        wgpu::LoadOp::Load
                    },
                    store: wgpu::StoreOp::Store,
                },
            })
        })
        .collect();
    encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some(label),
        color_attachments: &colors,
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &targets[3],
            depth_ops: Some(wgpu::Operations {
                load: if clear {
                    wgpu::LoadOp::Clear(0.0)
                } else {
                    wgpu::LoadOp::Load
                },
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    })
}

/// The gbuffer passes of a frame.
fn draw_gbuffer(
    encoder: &mut wgpu::CommandEncoder,
    targets: &[wgpu::TextureView],
    entity_gpu: &EntityGpu,
    draw: EntityDraw,
) {
    match draw {
        EntityDraw::None => {
            gbuffer_pass(encoder, targets, "gbuffer no entities", true);
        }
        EntityDraw::All => {
            let mut pass = gbuffer_pass(encoder, targets, "gbuffer entities", true);
            match std::env::var("MCRS_DRAW_ONLY").as_deref() {
                Ok("full_rigs") => entity_gpu.encode_draw_full_rigs(&mut pass),
                Ok("impostors") => entity_gpu.encode_draw_impostors(&mut pass),
                _ => entity_gpu.encode_draw(&mut pass),
            }
        }
        EntityDraw::Split => {
            entity_gpu.encode_draw_full_rigs(&mut gbuffer_pass(
                encoder,
                targets,
                "gbuffer full rigs",
                true,
            ));
            entity_gpu.encode_draw_impostors(&mut gbuffer_pass(
                encoder,
                targets,
                "gbuffer impostors",
                false,
            ));
        }
    }
}

/// A compute pass of arithmetic that keeps the GPU at its clock.
struct Heater {
    pipeline: wgpu::ComputePipeline,
    bind: wgpu::BindGroup,
    workgroups: u32,
}

impl Heater {
    /// A heater that writes the last word of `counters`.
    fn new(device: &wgpu::Device, counters: &wgpu::Buffer) -> Self {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("heater"),
            source: wgpu::ShaderSource::Wgsl(
                "@group(0) @binding(0) var<storage, read_write> sink: array<f32>;
                @compute @workgroup_size(64)
                fn heat(@builtin(global_invocation_id) id: vec3<u32>) {
                    var x = f32(id.x);
                    for (var i = 0u; i < 4096u; i = i + 1u) { x = fma(x, 1.0001, 0.5); }
                    if (x == 0.125) { sink[63] = x; }
                }"
                .into(),
            ),
        });
        let pipeline = device.create_compute_pipeline(&wgpu::ComputePipelineDescriptor {
            label: Some("heater"),
            layout: None,
            module: &module,
            entry_point: Some("heat"),
            compilation_options: Default::default(),
            cache: None,
        });
        let bind = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: None,
            layout: &pipeline.get_bind_group_layout(0),
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: counters.as_entire_binding(),
            }],
        });
        let workgroups = std::env::var("MCRS_GPU_HOT")
            .ok()
            .and_then(|text| text.parse().ok())
            .unwrap_or(16384);
        Self {
            pipeline,
            bind,
            workgroups,
        }
    }

    fn encode(&self, encoder: &mut wgpu::CommandEncoder) {
        let mut pass = encoder.begin_compute_pass(&wgpu::ComputePassDescriptor {
            label: Some("heater"),
            timestamp_writes: None,
        });
        pass.set_pipeline(&self.pipeline);
        pass.set_bind_group(0, &self.bind, &[]);
        pass.dispatch_workgroups(self.workgroups, 1, 1);
    }
}

/// The samples of the measured frames.
struct Measurements {
    pass_times: [Vec<f64>; 4],
    wall_with: Vec<f64>,
    wall_without: Vec<f64>,
    tick_cpu: Vec<f64>,
    upload_cpu: Vec<f64>,
    upload_bytes: Vec<f64>,
}

fn report(measured: &mut Measurements) {
    let mut sum = 0.0;
    for (pass, times) in ComputePass::ALL.iter().zip(&mut measured.pass_times) {
        let value = median(times);
        sum += value;
        println!(
            "{:<16} {value:.4} ms median of {}",
            pass.name(),
            times.len()
        );
    }
    let draw = median(&mut measured.wall_with) - median(&mut measured.wall_without);
    println!(
        "draw (wall with − without) {draw:.4} ms; wall with {:.3} ms, without {:.3} ms",
        median(&mut measured.wall_with),
        median(&mut measured.wall_without)
    );
    println!("all entity passes {:.4} ms", sum + draw);
    println!(
        "CPU tick writes {:.3} ms median per tick; upload {:.3} ms median, {:.2} MB per tick",
        median(&mut measured.tick_cpu),
        median(&mut measured.upload_cpu),
        median(&mut measured.upload_bytes) / 1e6
    );
}

/// The library of the pack with two opaque textures, the pig definition and its body and
/// saddle models.
fn pig_library(pack: &EntityPack) -> (EntityLibrary, u16, u16, u16) {
    let (definition, pig_entity) = pack.entity("minecraft:pig").unwrap();
    let textures = [texture("test:pig", 64), texture("test:saddle", 64)];
    let library = EntityLibrary::build(&pack.sources(), &textures, 2048);
    let tables = &library.definitions[definition];
    let model = |identifier: &str| {
        let short = pig_entity
            .resources
            .geometries
            .iter()
            .position(|(_, id)| id == identifier)
            .unwrap();
        tables.models[short].unwrap()
    };
    let (body, saddle) = (model("geometry.pig"), model("geometry.pig.saddle"));
    (library, definition as u16, body, saddle)
}

fn median(values: &mut [f64]) -> f64 {
    values.sort_by(f64::total_cmp);
    values.get(values.len() / 2).copied().unwrap_or(0.0)
}

#[expect(
    clippy::too_many_lines,
    reason = "one benchmark run reads top to bottom: set up, warm up, measure, report"
)]
fn main() {
    let gpu = support::Gpu::new().expect("an adapter");
    println!("adapter: {}", gpu.name);
    let pack = support::pack();
    let (library, definition, body, saddle) = pig_library(&pack);

    let mut seed = 0x5eed_u64;
    let mut tick = 1u32;
    let (mut arena, mut pigs) = spawn(definition, body, saddle, tick, &mut seed);
    let capacities = Capacities::for_cap(&library, FULL_RIG_CAP);
    let mut entity_gpu = gpu.entity_gpu(&library, capacities, arena.capacity());
    let targets = gbuffer(&gpu.device);
    let frame_view = camera();
    let heater = Heater::new(&gpu.device, entity_gpu.counters());
    let split = std::env::var_os("MCRS_SPLIT_DRAW").is_some();

    let queries = gpu.device.create_query_set(&wgpu::QuerySetDescriptor {
        label: None,
        ty: wgpu::QueryType::Timestamp,
        count: 8,
    });
    let resolve = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: None,
        size: 64,
        usage: wgpu::BufferUsages::QUERY_RESOLVE | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    let period = f64::from(gpu.queue.get_timestamp_period());

    let mut pass_times: [Vec<f64>; 4] = Default::default();
    let mut wall_with = Vec::new();
    let mut wall_without = Vec::new();
    let mut tick_cpu = Vec::new();
    let mut upload_cpu = Vec::new();
    let mut upload_bytes = Vec::new();
    let mut threshold = HeightThreshold::new();
    for frame_index in 0..WARMUP + 2 * MEASURED {
        let measuring = frame_index >= WARMUP;
        let draw = (frame_index - WARMUP.min(frame_index)) % 2 == 0;
        if frame_index % FRAMES_PER_TICK == 0 {
            tick += 1;
            walk(&mut pigs, &mut seed);
            let started = Instant::now();
            write_records(&pigs, &mut arena, tick);
            if measuring {
                tick_cpu.push(started.elapsed().as_secs_f64() * 1e3);
            }
        }
        let partial = (frame_index % FRAMES_PER_TICK) as f32 / FRAMES_PER_TICK as f32;
        let started = Instant::now();
        let upload = arena.take_upload();
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        heater.encode(&mut encoder);
        entity_gpu.upload(&gpu.device, &gpu.queue, &mut encoder, &upload);
        if measuring && !upload.ticks.is_empty() {
            upload_cpu.push(started.elapsed().as_secs_f64() * 1e3);
            upload_bytes.push(upload.bytes() as f64);
        }
        if let Some(counts) = entity_gpu.take_counts() {
            threshold.update(counts.full_rigs, FULL_RIG_CAP);
        }
        let uniform = FrameUniform {
            height_threshold: threshold.pixels(),
            ..FrameUniform::new(&frame_view, tick, partial, CardinalLight::Default)
        };
        entity_gpu.write_frame(&gpu.queue, &uniform, upload.slot_count);
        entity_gpu.encode_compute(
            &mut encoder,
            Some(PassQueries {
                set: &queries,
                first: 0,
            }),
        );
        let entity_draw = match (draw, split) {
            (false, _) => EntityDraw::None,
            (true, false) => EntityDraw::All,
            (true, true) => EntityDraw::Split,
        };
        draw_gbuffer(&mut encoder, &targets, &entity_gpu, entity_draw);
        encoder.resolve_query_set(&queries, 0..8, &resolve, 0);
        let submitted = Instant::now();
        gpu.queue.submit([encoder.finish()]);
        gpu.device
            .poll(wgpu::PollType::wait_indefinitely())
            .unwrap();
        let wall = submitted.elapsed().as_secs_f64() * 1e3;
        if !measuring {
            continue;
        }
        if draw {
            wall_with.push(wall);
        } else {
            wall_without.push(wall);
        }
        let stamps: Vec<u64> = bytemuck::cast_slice(&gpu.read(&resolve)).to_vec();
        for (pass, times) in ComputePass::ALL.iter().zip(&mut pass_times) {
            let index = *pass as usize * 2;
            let (begin, end) = (stamps[index], stamps[index + 1]);
            if end > begin {
                times.push((end - begin) as f64 * period / 1e6);
            }
        }
    }
    let counts = EntityFrameCounts::of(bytemuck::cast_slice(&gpu.read(entity_gpu.counters())));
    println!(
        "slots {} full rigs {} impostors {} faces {} dropped {}; height threshold {:.1} px; {} impostor image sets",
        pigs.len(),
        counts.full_rigs,
        counts.impostors,
        counts.faces,
        counts.dropped,
        threshold.pixels(),
        entity_gpu.impostor_sets()
    );
    report(&mut Measurements {
        pass_times,
        wall_with,
        wall_without,
        tick_cpu,
        upload_cpu,
        upload_bytes,
    });
}
