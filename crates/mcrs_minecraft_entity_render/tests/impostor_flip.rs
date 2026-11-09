//! The impostor test of `docs/spec/client-entity-render.md`, CEREN-VIS-003: each pig and frog
//! variant at a projected height of 11, 8 and 4 pixels and at 16 yaw angles, drawn once as an
//! impostor and once as a full rig on a real adapter. The mean FLIP error of the entity pixels
//! must be at most 0.1.
//!
//! It reads the entity textures from the client jar of the pinned release, or from
//! `MCRS_CLIENT_JAR`:
//!
//! ```sh
//! cargo test --release -p mcrs_minecraft_entity_render --test impostor_flip -- --ignored --nocapture
//! ```
#![expect(
    clippy::unwrap_used,
    clippy::panic,
    clippy::indexing_slicing,
    reason = "a test reports a failure by panicking"
)]
#![expect(
    clippy::disallowed_methods,
    reason = "the test finds the client jar through the environment"
)]

#[expect(dead_code, reason = "the test uses a part of the test support")]
mod support;

use glam::{Mat4, Vec2, Vec3};
use mcrs_minecraft_core::CardinalLight;
use mcrs_minecraft_entity_animation::EntityQuery;
use mcrs_minecraft_entity_render::{
    CULL_VALID, Capacities, CullRecord, EntityFrameCounts, EntityGpu, EntityLibrary, EntityPack,
    FrameUniform, STATE_BABY, STATE_HIDDEN_SHIFT, SlotArena, StateRecord, TextureImage, TickRecord,
    f16_up,
};
use mcrs_minecraft_render::{CameraOrigin, FrameView};
use mcrs_minecraft_render_controller::RenderQuery;
use mcrs_molang::{ScalarVm, Value, string_hash};

const SIZE: u32 = 128;
/// The frame is the middle of a frame of 1440 pixels in height with a field of view of 70°, so an
/// entity of a few pixels is as far as in the game.
const TEST_HEIGHT: f32 = 1440.0;
const HEIGHTS: [f32; 3] = [11.0, 8.0, 4.0];
/// Heights under those of the spec test: the errors are information.
const SMALL_HEIGHTS: [f32; 2] = [3.0, 2.0];
const YAWS: u32 = 16;
const TICK: u32 = 1000;
const LIMIT: f32 = 0.1;
/// The mean brightness of an impostor stays within this ratio of the full rig at each camera
/// view, also between the views of the image sets.
const BRIGHTNESS: std::ops::RangeInclusive<f64> = 0.95..=1.05;
/// Light levels other than full, so a light that the impostor reads from the wrong place shows.
const BLOCK_LIGHT: u8 = 7;
const SKY_LIGHT: u8 = 11;
const BACKGROUND: [u8; 3] = [120, 160, 210];
/// A camera direction relative to the body yaw of the entity at each of the 16 yaw angles.
struct CameraView {
    elevation: f32,
    /// Added to each of the 16 yaw angles of the image sets.
    yaw_offset: f32,
    /// The limit applies; the other views are information.
    judged: bool,
}

/// The test views, the yaws and the elevation of the images, then views between the images,
/// which show the error of the nearest view.
const CAMERA_VIEWS: [CameraView; 8] = [
    CameraView {
        elevation: 0.0,
        yaw_offset: 0.0,
        judged: true,
    },
    CameraView {
        elevation: 30.0,
        yaw_offset: 0.0,
        judged: false,
    },
    CameraView {
        elevation: 0.0,
        yaw_offset: 5.625,
        judged: false,
    },
    CameraView {
        elevation: 0.0,
        yaw_offset: 11.25,
        judged: false,
    },
    CameraView {
        elevation: 2.5,
        yaw_offset: 0.0,
        judged: false,
    },
    CameraView {
        elevation: 7.5,
        yaw_offset: 0.0,
        judged: false,
    },
    CameraView {
        elevation: 12.5,
        yaw_offset: 0.0,
        judged: false,
    },
    CameraView {
        elevation: 25.0,
        yaw_offset: 0.0,
        judged: false,
    },
];
/// A person 0.7 m from a monitor 0.7 m wide with 3840 pixels across, as the scene frames.
const PIXELS_PER_DEGREE: (f32, f32, f32) = (0.7, 3840.0, 0.7);

struct Case {
    name: &'static str,
    entity: &'static str,
    /// The geometry and the texture of each layer.
    layers: &'static [(&'static str, &'static str)],
    baby: bool,
}

const CASES: [Case; 10] = [
    Case {
        name: "pig temperate",
        entity: "minecraft:pig",
        layers: &[("geometry.pig", "minecraft:entity/pig/pig_temperate")],
        baby: false,
    },
    Case {
        name: "pig warm",
        entity: "minecraft:pig",
        layers: &[("geometry.pig", "minecraft:entity/pig/pig_warm")],
        baby: false,
    },
    Case {
        name: "pig cold",
        entity: "minecraft:pig",
        layers: &[("geometry.pig.cold", "minecraft:entity/pig/pig_cold")],
        baby: false,
    },
    Case {
        name: "pig saddled",
        entity: "minecraft:pig",
        layers: &[
            ("geometry.pig", "minecraft:entity/pig/pig_temperate"),
            (
                "geometry.pig.saddle",
                "minecraft:entity/equipment/pig_saddle/saddle",
            ),
        ],
        baby: false,
    },
    Case {
        name: "pig baby temperate",
        entity: "minecraft:pig",
        layers: &[(
            "geometry.pig.baby",
            "minecraft:entity/pig/pig_temperate_baby",
        )],
        baby: true,
    },
    Case {
        name: "pig baby warm",
        entity: "minecraft:pig",
        layers: &[("geometry.pig.baby", "minecraft:entity/pig/pig_warm_baby")],
        baby: true,
    },
    Case {
        name: "pig baby cold",
        entity: "minecraft:pig",
        layers: &[("geometry.pig.baby", "minecraft:entity/pig/pig_cold_baby")],
        baby: true,
    },
    Case {
        name: "frog temperate",
        entity: "minecraft:frog",
        layers: &[("geometry.frog", "minecraft:entity/frog/frog_temperate")],
        baby: false,
    },
    Case {
        name: "frog warm",
        entity: "minecraft:frog",
        layers: &[("geometry.frog", "minecraft:entity/frog/frog_warm")],
        baby: false,
    },
    Case {
        name: "frog cold",
        entity: "minecraft:frog",
        layers: &[("geometry.frog", "minecraft:entity/frog/frog_cold")],
        baby: false,
    },
];

/// The entity textures of the client jar.
fn jar_textures() -> Option<Vec<TextureImage>> {
    let path = std::env::var_os("MCRS_CLIENT_JAR")
        .map(std::path::PathBuf::from)
        .or_else(|| {
            mcrs_minecraft_client_jar::candidates()
                .into_iter()
                .find(|path| path.exists())
        })?;
    let jar = std::fs::read(&path).ok()?;
    let directory = mcrs_minecraft_client_jar::Directory::of(&jar).ok()?;
    let prefix = "assets/minecraft/textures/";
    let files = directory
        .read(&jar, |name| {
            name.starts_with(prefix)
                && name.ends_with(".png")
                && [
                    "entity/pig/",
                    "entity/frog/",
                    "entity/equipment/pig_saddle/",
                ]
                .iter()
                .any(|folder| name[prefix.len()..].starts_with(folder))
        })
        .ok()?;
    Some(
        files
            .into_iter()
            .map(|(name, bytes)| {
                let image = image::load_from_memory(&bytes).unwrap().to_rgba8();
                let path = &name[prefix.len()..name.len() - ".png".len()];
                TextureImage {
                    identifier: format!("minecraft:{path}"),
                    width: image.width(),
                    height: image.height(),
                    rgba: image.into_raw(),
                }
            })
            .collect(),
    )
}

/// The slot records of one case.
struct Placed {
    state: StateRecord,
    height: f32,
}

fn place(case: &Case, pack: &EntityPack, library: &EntityLibrary) -> Placed {
    let (definition, entity) = pack.entity(case.entity).unwrap();
    let tables = &library.definitions[definition];
    let mut layers = [[0u16; 2]; 4];
    for (layer, (geometry, texture)) in case.layers.iter().enumerate() {
        let short = entity
            .resources
            .geometries
            .iter()
            .position(|(_, id)| id == geometry)
            .unwrap();
        let variant = *library
            .textures
            .get(*texture)
            .unwrap_or_else(|| panic!("the jar has {texture}"));
        layers[layer] = [tables.models[short].unwrap(), variant];
    }
    let choices = entity
        .controllers
        .evaluate(&mut ScalarVm::new(), |query| match query {
            RenderQuery::Entity(EntityQuery::Pose) => Some(Value::String(string_hash("standing"))),
            _ => None,
        });
    let hidden = choices
        .first()
        .map_or(0, |choice| tables.hidden_mask(choice));
    let baby = if case.baby { tables.baby_scale } else { 1.0 };
    Placed {
        state: StateRecord {
            definition: definition as u16,
            layer_mask: (1u8 << case.layers.len()) - 1,
            layers,
            flags: if case.baby { STATE_BABY } else { 0 }
                | (u32::from(hidden) << STATE_HIDDEN_SHIFT),
            hurt_start: -1e9,
            block_light: BLOCK_LIGHT,
            sky_light: SKY_LIGHT,
            scale: f16_up(1.0),
            section: u32::MAX,
            clock_offset: TICK as i32,
            input: u32::MAX,
            ..StateRecord::default()
        },
        height: tables.box_height * baby,
    }
}

/// The pixels of one block at the distance of one block.
fn projection_scale() -> f32 {
    TEST_HEIGHT / (2.0 * 35f32.to_radians().tan())
}

fn field_of_view() -> f32 {
    2.0 * (SIZE as f32 / 2.0 / projection_scale()).atan()
}

/// The camera at `distance` from the middle of the box, `elevation` degrees above it, moved
/// across the view by `phase` pixels, so the edges of the entity fall at varied places inside
/// the pixels.
fn camera(height: f32, distance: f32, elevation: f32, phase: [f32; 2]) -> FrameView {
    let pixel = distance / projection_scale();
    let (sine, cosine) = elevation.to_radians().sin_cos();
    let up = Vec3::new(0.0, cosine, -sine);
    let shift = Vec3::X * phase[0] * pixel + up * phase[1] * pixel;
    let target = Vec3::new(0.5, 64.0 + height / 2.0, 0.5) + shift;
    let eye = target + Vec3::new(0.0, sine, cosine) * distance;
    let section = (eye / 16.0).floor().as_ivec3();
    let origin = section.as_vec3() * 16.0;
    let projection = Mat4::perspective_infinite_reverse_rh(field_of_view(), 1.0, 0.05);
    let view = Mat4::look_at_rh(eye - origin, target - origin, Vec3::Y);
    FrameView::new(
        projection,
        view,
        CameraOrigin {
            section,
            offset: eye - origin,
        },
        Vec2::splat(SIZE as f32),
    )
}

/// The slot of the entity and the targets of its frame.
struct Frame {
    arena: SlotArena,
    albedo: wgpu::Texture,
    light: wgpu::Texture,
    depth: wgpu::Texture,
    targets: Vec<wgpu::TextureView>,
    readback: wgpu::Buffer,
    light_readback: wgpu::Buffer,
    depth_readback: wgpu::Buffer,
}

/// The albedo and the light targets of a frame.
struct Rendered {
    albedo: Vec<u8>,
    /// The `f16` bits of each pixel of the light target.
    light: Vec<[u16; 4]>,
    /// The depth of each pixel, larger nearer to the camera.
    depth: Vec<f32>,
}

impl Frame {
    fn new(device: &wgpu::Device) -> Self {
        let texture = |format| {
            device.create_texture(&wgpu::TextureDescriptor {
                label: None,
                size: wgpu::Extent3d {
                    width: SIZE,
                    height: SIZE,
                    depth_or_array_layers: 1,
                },
                mip_level_count: 1,
                sample_count: 1,
                dimension: wgpu::TextureDimension::D2,
                format,
                usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
                view_formats: &[],
            })
        };
        let albedo = texture(wgpu::TextureFormat::Rgba8Unorm);
        let normal = texture(wgpu::TextureFormat::Rgba16Float);
        let light = texture(wgpu::TextureFormat::Rgba16Float);
        let depth = texture(wgpu::TextureFormat::Depth32Float);
        let targets = [&albedo, &normal, &light, &depth]
            .map(|texture| texture.create_view(&Default::default()))
            .into();
        let buffer = |bytes_per_pixel: u32| {
            device.create_buffer(&wgpu::BufferDescriptor {
                label: None,
                size: u64::from(SIZE * SIZE * bytes_per_pixel),
                usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
                mapped_at_creation: false,
            })
        };
        let readback = buffer(4);
        let light_readback = buffer(8);
        let depth_readback = buffer(4);
        let mut arena = SlotArena::new();
        arena.allocate();
        Self {
            arena,
            albedo,
            light,
            depth,
            targets,
            readback,
            light_readback,
            depth_readback,
        }
    }
}

/// The targets of one frame of `state` at the body yaw `yaw`.
fn render(
    gpu: &support::Gpu,
    entity_gpu: &mut EntityGpu,
    frame: &mut Frame,
    state: &StateRecord,
    yaw: f32,
    view: &FrameView,
    threshold: f32,
) -> Rendered {
    let slot = 0;
    frame.arena.write_state(slot, *state);
    frame.arena.write_tick(
        slot,
        TICK,
        TickRecord {
            block: [0, 64, 0],
            fraction: [32768, 0, 32768],
            body_yaw: TickRecord::turn_from_degrees(yaw),
            head_yaw: TickRecord::turn_from_degrees(yaw),
            ..TickRecord::default()
        },
        true,
    );
    frame.arena.write_cull(
        slot,
        CullRecord {
            block: [0, 64, 0],
            radius: f16_up(2.0),
            flags: CULL_VALID,
        },
    );
    let upload = frame.arena.take_upload();
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    entity_gpu.upload(&gpu.device, &gpu.queue, &mut encoder, &upload);
    let uniform = FrameUniform {
        height_threshold: threshold,
        ..FrameUniform::new(view, TICK, 0.0, CardinalLight::Default)
    };
    entity_gpu.write_frame(&gpu.queue, &uniform, upload.slot_count.max(1));
    entity_gpu.encode_compute(&mut encoder, None);
    {
        let colors: Vec<Option<wgpu::RenderPassColorAttachment<'_>>> = frame.targets[..3]
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
            label: None,
            color_attachments: &colors,
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &frame.targets[3],
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
        entity_gpu.encode_draw(&mut pass);
    }
    for (texture, buffer, bytes_per_pixel) in [
        (&frame.albedo, &frame.readback, 4),
        (&frame.light, &frame.light_readback, 8),
        (&frame.depth, &frame.depth_readback, 4),
    ] {
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(SIZE * bytes_per_pixel),
                    rows_per_image: None,
                },
            },
            texture.size(),
        );
    }
    gpu.queue.submit([encoder.finish()]);
    Rendered {
        albedo: gpu.read(&frame.readback),
        light: bytemuck::cast_slice(&gpu.read(&frame.light_readback)).to_vec(),
        depth: bytemuck::cast_slice(&gpu.read(&frame.depth_readback)).to_vec(),
    }
}

/// The mean of the colour channels of the pixels that a frame covers.
fn brightness(albedo: &[u8]) -> f64 {
    let (sum, count) = albedo
        .as_chunks::<4>()
        .0
        .iter()
        .filter(|pixel| pixel[3] != 0)
        .fold((0.0, 0.0), |(sum, count), pixel| {
            let channels = f64::from(pixel[0]) + f64::from(pixel[1]) + f64::from(pixel[2]);
            (sum + channels, count + 1.0)
        });
    sum / f64::max(count, 1.0)
}

/// Checks that each covered pixel of `frame` holds the light of the state record, in sixteenths
/// of a level, as the full rig writes it.
fn assert_light(frame: &Rendered, name: &str) {
    let expected = [
        f16_up(f32::from(BLOCK_LIGHT) * 16.0),
        f16_up(f32::from(SKY_LIGHT) * 16.0),
    ];
    for (pixel, light) in frame.albedo.as_chunks::<4>().0.iter().zip(&frame.light) {
        if pixel[3] != 0 {
            assert_eq!(
                [light[1], light[2]],
                expected,
                "{name}: the block and sky light of a pixel"
            );
        }
    }
}

/// The FLIP error of each pixel that either frame covers.
fn entity_errors(rig: &[u8], impostor: &[u8]) -> Vec<f32> {
    let rgb = |rgba: &[u8]| -> Vec<u8> {
        rgba.as_chunks::<4>()
            .0
            .iter()
            .flat_map(|pixel| {
                if pixel[3] == 0 {
                    BACKGROUND
                } else {
                    [pixel[0], pixel[1], pixel[2]]
                }
            })
            .collect()
    };
    let (distance, pixels, width) = PIXELS_PER_DEGREE;
    let error = nv_flip::flip(
        nv_flip::FlipImageRgb8::with_data(SIZE, SIZE, &rgb(rig)),
        nv_flip::FlipImageRgb8::with_data(SIZE, SIZE, &rgb(impostor)),
        nv_flip::pixels_per_degree(distance, pixels, width),
    );
    error
        .to_vec()
        .into_iter()
        .zip(
            rig.as_chunks::<4>()
                .0
                .iter()
                .zip(impostor.as_chunks::<4>().0),
        )
        .filter(|(_, (a, b))| a[3] != 0 || b[3] != 0)
        .map(|(value, _)| value)
        .collect()
}

fn mean(values: &[f32]) -> f32 {
    values.iter().map(|value| f64::from(*value)).sum::<f64>() as f32 / values.len().max(1) as f32
}

#[test]
#[ignore = "needs a GPU and the client jar"]
fn an_impostor_looks_like_the_full_rig_below_twelve_pixels() {
    let Some(gpu) = support::Gpu::new() else {
        panic!("no adapter");
    };
    let textures = jar_textures().expect("the client jar of the pinned release");
    let pack = support::pack();
    let library = EntityLibrary::build(&pack.sources(), &textures, 2048);
    let mut entity_gpu = gpu.entity_gpu(&library, Capacities::for_cap(&library, 64), 64);
    let mut frame = Frame::new(&gpu.device);
    let focal = projection_scale();
    println!("{}", gpu.name);
    println!(
        "case                 elevation  yaw offset  FLIP and brightness at 11, 8, 4, then 3 and 2 px"
    );
    let mut worst = 0.0f32;
    for case in &CASES {
        let placed = place(case, &pack, &library);
        for camera_view in CAMERA_VIEWS {
            let mut row = format!(
                "{:<20} {:>9} {:>11}",
                case.name, camera_view.elevation, camera_view.yaw_offset
            );
            for pixels in HEIGHTS.into_iter().chain(SMALL_HEIGHTS) {
                let judged_height = HEIGHTS.contains(&pixels);
                let distance = placed.height * focal / pixels;
                let mut errors = Vec::new();
                let mut brightness_sum = [0.0f64; 2];
                for step in 0..YAWS {
                    let yaw = step as f32 * 360.0 / YAWS as f32 + camera_view.yaw_offset;
                    let phase = [
                        (step as f32 * 0.618).fract(),
                        (step as f32 * 0.382 + 0.25).fract(),
                    ];
                    let view = camera(placed.height, distance, camera_view.elevation, phase);
                    let mut draw = |threshold| {
                        render(
                            &gpu,
                            &mut entity_gpu,
                            &mut frame,
                            &placed.state,
                            yaw,
                            &view,
                            threshold,
                        )
                    };
                    let rig = draw(0.0);
                    let impostor = draw(f32::MAX);
                    let counts = EntityFrameCounts::of(bytemuck::cast_slice(
                        &gpu.read(entity_gpu.counters()),
                    ));
                    assert_eq!(
                        counts.impostors, 1,
                        "{}: the slot is an impostor",
                        case.name
                    );
                    assert_light(&rig, case.name);
                    assert_light(&impostor, case.name);
                    brightness_sum[0] += brightness(&rig.albedo);
                    brightness_sum[1] += brightness(&impostor.albedo);
                    errors.extend(entity_errors(&rig.albedo, &impostor.albedo));
                }
                let error = mean(&errors);
                let ratio = brightness_sum[1] / brightness_sum[0];
                assert!(
                    !judged_height || BRIGHTNESS.contains(&ratio),
                    "{}: an impostor at {pixels} px is {ratio:.3} times as bright as the full \
                     rig at the elevation {} and the yaw offset {}",
                    case.name,
                    camera_view.elevation,
                    camera_view.yaw_offset
                );
                if camera_view.judged && judged_height {
                    worst = worst.max(error);
                }
                row += &format!(" {error:>7.4} {ratio:>5.3}");
            }
            println!("{row}");
        }
    }
    println!("worst mean on the judged views: {worst:.4}");
    assert!(
        worst <= LIMIT,
        "the mean FLIP error of an impostor is at most {LIMIT}"
    );
}

/// The saddle of a cold pig lies on the outer cube of its body: both grow by the same inflate.
/// The layers draw in order, so the saddle must win there whatever order the face records of a
/// draw take: on the surface of the body, the saddle is nearer by a small step, never at the same
/// depth.
#[test]
#[ignore = "needs a GPU and the client jar"]
fn a_later_layer_wins_on_the_surface_of_an_earlier_one() {
    let Some(gpu) = support::Gpu::new() else {
        panic!("no adapter");
    };
    let textures = jar_textures().expect("the client jar of the pinned release");
    let pack = support::pack();
    let library = EntityLibrary::build(&pack.sources(), &textures, 2048);
    let mut entity_gpu = gpu.entity_gpu(&library, Capacities::for_cap(&library, 64), 64);
    let mut frame = Frame::new(&gpu.device);
    let case = Case {
        name: "pig cold saddled",
        entity: "minecraft:pig",
        layers: &[
            ("geometry.pig.cold", "minecraft:entity/pig/pig_cold"),
            (
                "geometry.pig.saddle",
                "minecraft:entity/equipment/pig_saddle/saddle",
            ),
        ],
        baby: false,
    };
    let placed = place(&case, &pack, &library);
    let alone = |mask: u8| StateRecord {
        layer_mask: mask,
        ..placed.state
    };
    let distance = placed.height * projection_scale() / 60.0;
    let mut on_surface = 0usize;
    let mut tied = 0usize;
    for step in 0..YAWS {
        let yaw = step as f32 * 360.0 / YAWS as f32 + 7.0;
        let view = camera(placed.height, distance, 30.0, [0.0, 0.0]);
        let mut draw =
            |state: &StateRecord| render(&gpu, &mut entity_gpu, &mut frame, state, yaw, &view, 0.0);
        let body = draw(&alone(0b01));
        let saddle = draw(&alone(0b10));
        for (index, (near, far)) in saddle.depth.iter().zip(&body.depth).enumerate() {
            if saddle.albedo[index * 4 + 3] == 0 || body.albedo[index * 4 + 3] == 0 {
                continue;
            }
            on_surface += usize::from((near - far).abs() <= far * 1e-4);
            tied += usize::from(near == far);
        }
    }
    println!(
        "{}: {on_surface} saddle pixels on the surface of the body, {tied} at its depth",
        gpu.name
    );
    assert!(
        on_surface > 100,
        "the saddle lies on the surface of the body"
    );
    assert_eq!(tied, 0, "no saddle pixel has the depth of the body");
}
