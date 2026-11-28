use bevy::math::{Mat4, Vec3};
use bevy::shader::ShaderDefVal;
use bevy::tasks::block_on;
use mcrs_minecraft_mesh::pack;
use mcrs_minecraft_render::shared_bindings::{terrain_draw, terrain_view};
use mcrs_minecraft_render::{CameraUniform, Params, SectionDesc, SpriteEntry};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::*;

use crate::world_shaders::wgsl;

const SIZE: u32 = 256;
const TARGETS: [TextureFormat; 3] = [
    TextureFormat::Rgba8Unorm,
    TextureFormat::Rgba16Float,
    TextureFormat::Rgba16Float,
];

fn buffer(device: &Device, data: &[u32], usage: BufferUsages) -> Buffer {
    device.create_buffer_init(&BufferInitDescriptor {
        label: None,
        contents: bytemuck::cast_slice(data),
        usage,
    })
}

fn model_vertices() -> Vec<u32> {
    let positions = [[1, 2], [1, 1], [2, 1], [2, 2]];
    let uvs = [[0, 0], [0, 1023], [307, 614], [1023, 0]];
    let mut vertices = Vec::new();
    for (position, uv) in positions.into_iter().zip(uvs) {
        let mut words = [0; 3];
        pack::MODEL_X.set(&mut words, (position[0] * 32 + 64) as u64);
        pack::MODEL_Y.set(&mut words, (position[1] * 32 + 64) as u64);
        pack::MODEL_Z.set(&mut words, 64);
        pack::MODEL_U.set(&mut words, uv[0]);
        pack::MODEL_V.set(&mut words, uv[1]);
        pack::MODEL_SHADE.set(&mut words, 255);
        vertices.extend(words);
    }
    vertices
}

fn target(device: &Device, format: TextureFormat) -> Texture {
    device.create_texture(&TextureDescriptor {
        label: None,
        size: Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format,
        usage: TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
        view_formats: &[],
    })
}

#[derive(Clone, Copy)]
enum Fragment {
    Solid,
    Cutout,
    Expanded,
}

fn pipeline(device: &Device, model: bool, fragment: Fragment) -> RenderPipeline {
    let shader = if model { "model" } else { "greedy" };
    let mut source = wgsl(shader, vec![ShaderDefVal::Bool("SOLID".into(), true)]);
    let vertex_output = if model { "ModelOut" } else { "GreedyOut" };
    source.push_str(&format!("\n@fragment fn expanded_only(input: {vertex_output}) -> @location(0) vec4<f32> {{ return vec4<f32>(1.0); }}\n"));
    let module = device.create_shader_module(ShaderModuleDescriptor {
        label: Some(shader),
        source: ShaderSource::Wgsl(source.into()),
    });
    device.create_render_pipeline(&RenderPipelineDescriptor {
        label: Some(shader),
        layout: None,
        vertex: VertexState {
            module: &module,
            entry_point: Some(if model {
                "vertex_model"
            } else {
                "vertex_greedy"
            }),
            compilation_options: Default::default(),
            buffers: &[],
        },
        primitive: Default::default(),
        depth_stencil: None,
        multisample: Default::default(),
        fragment: Some(FragmentState {
            module: &module,
            entry_point: Some(match (model, fragment) {
                (_, Fragment::Expanded) => "expanded_only",
                (true, Fragment::Solid) => "fragment_model_solid",
                (true, Fragment::Cutout) => "fragment_model_cutout",
                (false, Fragment::Solid) => "fragment_greedy_solid",
                (false, Fragment::Cutout) => "fragment_greedy_cutout",
            }),
            compilation_options: Default::default(),
            targets: &TARGETS.map(|format| {
                Some(ColorTargetState {
                    format,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })
            }),
        }),
        multiview_mask: None,
        cache: None,
    })
}

fn frame_group(device: &Device, pipeline: &RenderPipeline) -> BindGroup {
    let transform = Mat4::from_translation(Vec3::new(-1.5, -1.5, 0.5));
    let mut camera_data = CameraUniform::default();
    camera_data.clip_from_relative = transform.to_cols_array();
    camera_data.viewport = [SIZE as f32 / 2.0; 2];
    camera_data.raster_viewport = [SIZE as f32; 2];
    let params = buffer(
        device,
        bytemuck::cast_slice(&[Params::default()]),
        BufferUsages::UNIFORM,
    );
    let camera = buffer(
        device,
        bytemuck::cast_slice(&[camera_data]),
        BufferUsages::UNIFORM,
    );
    device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(0),
        entries: &[
            BindGroupEntry {
                binding: terrain_view::PARAMS,
                resource: params.as_entire_binding(),
            },
            BindGroupEntry {
                binding: terrain_view::CAMERA,
                resource: camera.as_entire_binding(),
            },
        ],
    })
}

fn atlas(device: &Device, queue: &Queue) -> TextureView {
    let atlas = device.create_texture(&TextureDescriptor {
        label: None,
        size: Extent3d {
            width: 2,
            height: 2,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: TextureDimension::D2,
        format: TextureFormat::Rgba8Unorm,
        usage: TextureUsages::TEXTURE_BINDING | TextureUsages::COPY_DST,
        view_formats: &[],
    });
    queue.write_texture(
        atlas.as_image_copy(),
        &[
            255, 0, 0, 255, 0, 255, 0, 255, 0, 0, 255, 255, 255, 255, 0, 255,
        ],
        TexelCopyBufferLayout {
            offset: 0,
            bytes_per_row: Some(8),
            rows_per_image: Some(2),
        },
        Extent3d {
            width: 2,
            height: 2,
            depth_or_array_layers: 1,
        },
    );
    atlas.create_view(&TextureViewDescriptor {
        dimension: Some(TextureViewDimension::D2Array),
        ..Default::default()
    })
}

fn terrain_group(
    device: &Device,
    queue: &Queue,
    pipeline: &RenderPipeline,
    model: bool,
    fragment: Fragment,
) -> BindGroup {
    let expanded_only = matches!(fragment, Fragment::Expanded);
    let mut quad = [0; 2];
    pack::QUAD_X.set(&mut quad, 1);
    pack::QUAD_Y.set(&mut quad, 2);
    pack::QUAD_FACE.set(&mut quad, 3);
    let model_geometry = model_vertices();
    let geometry = buffer(
        device,
        if model { &model_geometry } else { &quad },
        BufferUsages::STORAGE,
    );
    let visible = buffer(device, &[0, 0], BufferUsages::STORAGE);
    let faces = buffer(device, &[0; 3], BufferUsages::STORAGE);
    let sections = buffer(
        device,
        bytemuck::cast_slice(&[SectionDesc {
            section: [0, 0, 0],
            scale: 1,
            face_base: 0,
        }]),
        BufferUsages::STORAGE,
    );
    let animations = buffer(device, &[0; 4], BufferUsages::STORAGE);
    let sprites = buffer(
        device,
        bytemuck::cast_slice(&[SpriteEntry {
            array_layer: 0,
            animation: u32::MAX,
        }]),
        BufferUsages::STORAGE,
    );
    let view = atlas(device, queue);
    let sampler = device.create_sampler(&SamplerDescriptor::default());
    let mut entries = vec![
        BindGroupEntry {
            binding: if model {
                terrain_draw::VERTICES
            } else {
                terrain_draw::QUADS
            },
            resource: geometry.as_entire_binding(),
        },
        BindGroupEntry {
            binding: terrain_draw::VISIBLE,
            resource: visible.as_entire_binding(),
        },
    ];
    if !expanded_only {
        for binding in [
            terrain_draw::ATLAS0,
            terrain_draw::ATLAS1,
            terrain_draw::ATLAS2,
            terrain_draw::ATLAS3,
        ] {
            entries.push(BindGroupEntry {
                binding,
                resource: BindingResource::TextureView(&view),
            });
        }
        entries.extend([
            BindGroupEntry {
                binding: terrain_draw::ATLAS_SAMPLER,
                resource: BindingResource::Sampler(&sampler),
            },
            BindGroupEntry {
                binding: terrain_draw::TINTS,
                resource: BindingResource::TextureView(&view),
            },
            BindGroupEntry {
                binding: terrain_draw::TINT_SAMPLER,
                resource: BindingResource::Sampler(&sampler),
            },
            BindGroupEntry {
                binding: terrain_draw::ANIMATIONS,
                resource: animations.as_entire_binding(),
            },
            BindGroupEntry {
                binding: terrain_draw::SPRITES,
                resource: sprites.as_entire_binding(),
            },
        ]);
        if !model {
            entries.push(BindGroupEntry {
                binding: terrain_draw::FACES,
                resource: faces.as_entire_binding(),
            });
        }
    }
    entries.push(BindGroupEntry {
        binding: terrain_draw::SECTIONS,
        resource: sections.as_entire_binding(),
    });
    device.create_bind_group(&BindGroupDescriptor {
        label: None,
        layout: &pipeline.get_bind_group_layout(1),
        entries: &entries,
    })
}

fn render(device: &Device, queue: &Queue, model: bool, fragment: Fragment) -> Vec<u8> {
    let pipeline = pipeline(device, model, fragment);
    let group0 = frame_group(device, &pipeline);
    let group1 = terrain_group(device, queue, &pipeline, model, fragment);
    let images = TARGETS.map(|format| target(device, format));
    let views = images
        .each_ref()
        .map(|image| image.create_view(&Default::default()));
    let read = device.create_buffer(&BufferDescriptor {
        label: None,
        size: u64::from(SIZE * SIZE * 4),
        usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
        mapped_at_creation: false,
    });
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let attachments = views.each_ref().map(|view| {
            Some(RenderPassColorAttachment {
                view,
                depth_slice: None,
                resolve_target: None,
                ops: Operations {
                    load: LoadOp::Clear(Color::TRANSPARENT),
                    store: StoreOp::Store,
                },
            })
        });
        let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
            color_attachments: &attachments,
            ..Default::default()
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &group0, &[]);
        pass.set_bind_group(1, &group1, &[]);
        pass.draw(0..6, 0..1);
    }
    encoder.copy_texture_to_buffer(
        images[0].as_image_copy(),
        TexelCopyBufferInfo {
            buffer: &read,
            layout: TexelCopyBufferLayout {
                offset: 0,
                bytes_per_row: Some(SIZE * 4),
                rows_per_image: Some(SIZE),
            },
        },
        Extent3d {
            width: SIZE,
            height: SIZE,
            depth_or_array_layers: 1,
        },
    );
    queue.submit([encoder.finish()]);
    read.slice(..).map_async(MapMode::Read, |_| {});
    device.poll(PollType::wait_indefinitely()).unwrap();
    let result = read.slice(..).get_mapped_range().unwrap().to_vec();
    read.unmap();
    result
}

fn pixel(image: &[u8], x: u32, y: u32) -> &[u8] {
    &image[((y * SIZE + x) * 4) as usize..][..4]
}

#[test]
fn production_solid_shaders_clip_expanded_faces_and_sample_uvs() {
    let instance = Instance::new(InstanceDescriptor::new_without_display_handle());
    let adapter = block_on(instance.request_adapter(&RequestAdapterOptions::default())).unwrap();
    let (device, queue) = block_on(adapter.request_device(&DeviceDescriptor {
        required_limits: adapter.limits(),
        ..Default::default()
    }))
    .unwrap();
    for model in [false, true] {
        let expanded = render(&device, &queue, model, Fragment::Expanded);
        assert_ne!(
            pixel(&expanded, 63, 128),
            &[0, 0, 0, 0],
            "the production vertex shader must expand the face"
        );
        for fragment in [Fragment::Solid, Fragment::Cutout] {
            let image = render(&device, &queue, model, fragment);
            assert_eq!(
                pixel(&image, 63, 128),
                &[0, 0, 0, 0],
                "the production fragment shader must discard outside its face"
            );
            assert_ne!(
                pixel(&image, 64, 128),
                &[0, 0, 0, 0],
                "the original face must remain covered"
            );
            if model {
                assert_eq!(
                    pixel(&image, 96, 160),
                    &[0, 0, 255, 255],
                    "the first triangle preserves the non-affine UV"
                );
                assert_eq!(
                    pixel(&image, 160, 96),
                    &[0, 255, 0, 255],
                    "the second triangle preserves the non-affine UV"
                );
            }
        }
    }
}
