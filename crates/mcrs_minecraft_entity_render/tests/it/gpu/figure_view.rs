use crate::support;
use glam::{Mat4, Vec3};
use mcrs_minecraft_entity_render::{
    CULL_VALID, Capacities, CullRecord, EntityGpu, EntityLibrary, EntityPack, EntityViews,
    FigureFrame, FigureTargets, SlotArena, StateRecord, TextureImage, TickRecord, f16_up,
};

const TICK: u32 = 1000;
const SIDE: u32 = 64;
const SKIN: [u8; 4] = [200, 120, 60, 255];
const CLEAR: [u8; 4] = [0, 0, 0, 0];

const FRONT_FACTOR: f32 = 0.8201;

#[test]
fn figure_view_player_draws_with_figure_face_factor_in_scissor() {
    let Some(gpu) = support::Gpu::new() else {
        eprintln!("skipped: no adapter, the figure view test needs a GPU");
        return;
    };
    let pack = support::pack();
    let steve = TextureImage {
        identifier: "minecraft:entity/player/wide/steve".into(),
        width: SIDE,
        height: SIDE,
        rgba: SKIN.repeat((SIDE * SIDE) as usize),
    };
    let library = EntityLibrary::build(&pack.sources(), &[steve], 2048);
    let mut arena = player_arena(&pack, &library);

    let capacities = Capacities::for_cap(&library, 64);
    let mut entity_gpu = gpu.entity_gpu(&library, capacities, arena.capacity());
    let upload = arena.take_upload();
    let mut encoder = gpu.device.create_command_encoder(&Default::default());
    entity_gpu.upload(&gpu.device, &gpu.queue, &mut encoder, &upload);
    gpu.queue.submit([encoder.finish()]);

    let figure = FigureFrame {
        clip_from_figure: Mat4::from_cols_array(&[
            1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.1, 0.0, 0.0, -1.0, 0.5, 1.0,
        ]),
        lights: [Vec3::new(0.2, 1.0, 1.0), Vec3::new(-0.2, 1.0, 0.0)],
    };
    let lightmap = white_lightmap(&gpu);
    let sampler = gpu.device.create_sampler(&Default::default());
    for color in [
        wgpu::TextureFormat::Rgba8Unorm,
        wgpu::TextureFormat::Rgba8UnormSrgb,
    ] {
        let targets = FigureTargets {
            color,
            depth: wgpu::TextureFormat::Depth32Float,
            depth_compare: wgpu::CompareFunction::GreaterEqual,
            sample_count: 1,
        };
        entity_gpu.prepare_figure(&gpu.device, targets, &lightmap, &sampler);
        entity_gpu.write_figure_frame(&gpu.queue, &figure, TICK, 0.0);
        let mut encoder = gpu.device.create_command_encoder(&Default::default());
        entity_gpu.encode_figure_compute(&mut encoder);

        let target = texture(&gpu, targets.color, wgpu::TextureUsages::COPY_SRC);
        let depth = texture(&gpu, targets.depth, wgpu::TextureUsages::empty());
        let scissor_top = 20;
        draw_figure(&mut encoder, &entity_gpu, &target, &depth, scissor_top);
        let texels = read_texture(&gpu, encoder, &target);
        let texel = |x: u32, y: u32| -> [u8; 4] {
            let at = ((y * SIDE + x) * 4) as usize;
            texels.get(at..at + 4).unwrap().try_into().unwrap()
        };

        let expected = SKIN.map(|channel| (f32::from(channel) * FRONT_FACTOR).round() as i16);
        let drawn = texel(SIDE / 2, 28);
        for (channel, (&got, want)) in drawn.iter().zip(expected).take(3).enumerate() {
            assert!(
                (i16::from(got) - want).abs() <= 1,
                "{color:?}: channel {channel} of the front: {drawn:?}, expected {expected:?}"
            );
        }
        assert_eq!(
            drawn.get(3),
            Some(&255),
            "{color:?}: the figure writes an opaque colour"
        );
        assert_eq!(
            texel(SIDE / 2, 8),
            CLEAR,
            "{color:?}: the head is outside the scissor"
        );
        assert_eq!(
            texel(2, 40),
            CLEAR,
            "{color:?}: no part of the player is at the left edge"
        );
    }
}

fn draw_figure(
    encoder: &mut wgpu::CommandEncoder,
    entity_gpu: &EntityGpu,
    target: &wgpu::Texture,
    depth: &wgpu::Texture,
    scissor_top: u32,
) {
    let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
        label: Some("figure test"),
        color_attachments: &[Some(wgpu::RenderPassColorAttachment {
            view: &target.create_view(&Default::default()),
            depth_slice: None,
            resolve_target: None,
            ops: wgpu::Operations {
                load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                store: wgpu::StoreOp::Store,
            },
        })],
        depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
            view: &depth.create_view(&Default::default()),
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
    pass.set_scissor_rect(0, scissor_top, SIDE, SIDE - scissor_top);
    entity_gpu.encode_figure_draw(&mut pass);
}

fn player_arena(pack: &EntityPack, library: &EntityLibrary) -> SlotArena {
    let (definition, player) = pack.entity("minecraft:player").unwrap();
    let at = player
        .resources
        .geometries
        .iter()
        .position(|(geometry, _)| geometry.as_str() == "default")
        .unwrap();
    let model = library
        .definitions
        .get(definition)
        .and_then(|tables| tables.models.get(at).copied().flatten())
        .unwrap();
    let variant = *library
        .textures
        .get("minecraft:entity/player/wide/steve")
        .unwrap();

    let mut arena = SlotArena::new();
    let slot = arena.allocate();
    arena.write_state(
        slot,
        StateRecord {
            definition: definition as u16,
            layer_mask: 1,
            layers: [[model, variant], [0; 2], [0; 2], [0; 2]],
            hurt_start: -1e9,
            block_light: 15,
            sky_light: 15,
            scale: f16_up(1.0),
            section: u32::MAX,
            clock_offset: TICK as i32,
            input: u32::MAX,
            ..StateRecord::default()
        },
    );
    arena.write_tick(slot, TICK, TickRecord::default(), true);
    arena.write_cull(
        slot,
        CullRecord {
            block: [0, 0, 0],
            radius: f16_up(2.0),
            flags: CULL_VALID | EntityViews::FIGURE.cull_flags(),
        },
    );
    arena
}

fn white_lightmap(gpu: &support::Gpu) -> wgpu::TextureView {
    let lightmap = gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("white lightmap"),
        size: wgpu::Extent3d {
            width: 16,
            height: 16,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: wgpu::TextureFormat::Rgba8Unorm,
        usage: wgpu::TextureUsages::TEXTURE_BINDING | wgpu::TextureUsages::COPY_DST,
        view_formats: &[],
    });
    gpu.queue.write_texture(
        lightmap.as_image_copy(),
        &[255; 16 * 16 * 4],
        wgpu::TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(16 * 4),
            rows_per_image: None,
        },
        lightmap.size(),
    );
    lightmap.create_view(&Default::default())
}

fn read_texture(
    gpu: &support::Gpu,
    mut encoder: wgpu::CommandEncoder,
    target: &wgpu::Texture,
) -> Vec<u8> {
    let readback = gpu.device.create_buffer(&wgpu::BufferDescriptor {
        label: Some("figure readback"),
        size: u64::from(SIDE * SIDE * 4),
        usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::COPY_SRC,
        mapped_at_creation: false,
    });
    encoder.copy_texture_to_buffer(
        target.as_image_copy(),
        wgpu::TexelCopyBufferInfo {
            buffer: &readback,
            layout: wgpu::TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(SIDE * 4),
                rows_per_image: None,
            },
        },
        target.size(),
    );
    gpu.queue.submit([encoder.finish()]);
    gpu.read(&readback)
}

fn texture(
    gpu: &support::Gpu,
    format: wgpu::TextureFormat,
    usage: wgpu::TextureUsages,
) -> wgpu::Texture {
    gpu.device.create_texture(&wgpu::TextureDescriptor {
        label: Some("figure target"),
        size: wgpu::Extent3d {
            width: SIDE,
            height: SIDE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT | usage,
        view_formats: &[],
    })
}
