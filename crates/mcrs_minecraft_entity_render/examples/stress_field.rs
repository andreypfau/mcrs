//! The stress scene S1 of `docs/entity-render-budget.md`, section 5.1: 100,000 pigs on a field
//! of 316 × 316 blocks, one pig on each block, 10% with a saddle, half of them walking. The
//! camera stands at a corner at the eye height 1.62 and looks along the diagonal, at 2560 × 1440.
//!
//! ```sh
//! cargo run --release -p mcrs_minecraft_entity_render --example stress_field
//! ```
//!
//! It times each compute pass of the entity passes with pass timestamps, and the gbuffer draw as
//! the difference of the wall time of frames with and without the draw, since the timestamps of
//! a render pass on Metal do not bracket its work (section 4.14). It also times the CPU writes of
//! the slot records in a tick and their upload.
#![expect(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    reason = "a developer tool reports a failure by panicking"
)]

#[path = "../tests/support/mod.rs"]
#[expect(dead_code, reason = "the example uses a part of the test support")]
mod support;

use std::time::Instant;

use glam::{Mat4, Vec3};
use mcrs_minecraft_entity_render::{
    CULL_VALID, Capacities, ComputePass, CullRecord, EntityGpu, EntityLibrary, EntityPack,
    FULL_RIG_CAP, FrameUniform, FrameView, PassQueries, SlotArena, StateRecord, TextureImage,
    TickRecord, f16_up,
};

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

fn turn(degrees: f32) -> u16 {
    ((degrees.rem_euclid(360.0) / 360.0 * 65536.0).round() as u32 % 65536) as u16
}

fn record(pig: &Pig) -> TickRecord {
    let block = pig.position.floor();
    let fraction = ((pig.position - block) * 65536.0)
        .as_uvec3()
        .min(glam::UVec3::splat(65535));
    TickRecord {
        block: block.as_ivec3().to_array(),
        fraction: [fraction.x as u16, fraction.y as u16, fraction.z as u16],
        body_yaw: turn(pig.heading),
        head_yaw: turn(pig.heading + 20.0),
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
    FrameView {
        clip_from_relative: projection * view,
        camera_section: section,
        camera_offset: offset,
        viewport: [WIDTH as f32, HEIGHT as f32],
        focal_y: projection.y_axis.y,
    }
}

/// The gbuffer pass of a frame, cleared, with the entity draw when `draw` is set.
fn draw_gbuffer(
    encoder: &mut wgpu::CommandEncoder,
    targets: &[wgpu::TextureView],
    entity_gpu: &EntityGpu,
    draw: bool,
) {
    let colors: Vec<Option<wgpu::RenderPassColorAttachment<'_>>> = targets[..3]
        .iter()
        .map(|view| {
            Some(wgpu::RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })
        })
        .collect();
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("gbuffer"),
        color_attachments: &colors,
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &targets[3],
            depth_ops: Some(wgpu::Operations {
                load: wgpu::LoadOp::Clear(0.0),
                store: wgpu::StoreOp::Store,
            }),
            stencil_ops: None,
        }),
        timestamp_writes: None,
        occlusion_query_set: None,
        multiview_mask: None,
    });
    if draw {
        entity_gpu.encode_draw(&mut pass);
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
        entity_gpu.upload(&gpu.device, &gpu.queue, &mut encoder, &upload);
        if measuring && !upload.ticks.is_empty() {
            upload_cpu.push(started.elapsed().as_secs_f64() * 1e3);
            upload_bytes.push(upload.bytes() as f64);
        }
        let uniform = FrameUniform::new(&frame_view, tick, partial, false);
        entity_gpu.write_frame(&gpu.queue, &uniform, upload.slot_count);
        entity_gpu.encode_compute(
            &mut encoder,
            Some(PassQueries {
                set: &queries,
                first: 0,
            }),
        );
        draw_gbuffer(&mut encoder, &targets, &entity_gpu, draw);
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
    let counters: Vec<u32> = bytemuck::cast_slice(&gpu.read(entity_gpu.counters())).to_vec();
    println!(
        "slots {} visible {} (under 12 px: {}, which draw a full rig until impostors exist) faces {} over the cap {}",
        pigs.len(),
        counters[1],
        counters[26],
        counters[17],
        counters[25]
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
