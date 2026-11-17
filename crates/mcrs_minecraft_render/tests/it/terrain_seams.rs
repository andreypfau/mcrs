use std::borrow::Cow;
use std::path::Path;

use bevy::math::{DMat4, DVec3, DVec4, Mat4, Vec3, proj};
use bevy::prelude::Transform;
use bevy::tasks::block_on;
use mcrs_minecraft_render::CameraUniform;
use wesl::error::ResolveError;
use wesl::syntax::{ModulePath, PathOrigin};
use wesl::{CompileOptions, Resolver};
use wgpu::util::{BufferInitDescriptor, DeviceExt};
use wgpu::{
    BindGroup, BindGroupDescriptor, BindGroupEntry, BufferDescriptor, BufferUsages, Color,
    ColorTargetState, ColorWrites, CompareFunction, DepthStencilState, Device, DeviceDescriptor,
    Extent3d, FragmentState, Instance, InstanceDescriptor, LoadOp, MapMode, Operations,
    PipelineCompilationOptions, PollType, Queue, RenderPassColorAttachment,
    RenderPassDepthStencilAttachment, RenderPassDescriptor, RenderPipeline,
    RenderPipelineDescriptor, RequestAdapterOptions, ShaderModuleDescriptor, ShaderSource, StoreOp,
    TexelCopyBufferInfo, TexelCopyBufferLayout, TextureDescriptor, TextureDimension, TextureFormat,
    TextureUsages, VertexState,
};

const WIDTH: u32 = 384;
const HEIGHT: u32 = 256;
const BACKGROUND: [u8; 4] = [0, 0, 0, 255];

const SHADER: &str = r#"
import mcrs_minecraft_render::shaders::include::quad::{GREEDY_EDGE_OVERLAP, model_quad_uv, expanded_quad_uv};
import mcrs_minecraft_render::shaders::include::quad_coverage::{quad_projection, quad_uv_at, quad_contains};
import mcrs_minecraft_render::shaders::core::greedy::{GreedyOut, greedy_coverage};
import mcrs_minecraft_render::shaders::core::model::{ModelOut, model_coverage};

struct Quad {
    anchor: vec4<f32>,
    u: vec4<f32>,
    v: vec4<f32>,
    color: vec4<f32>,
    size_and_guarded: vec4<f32>,
};
@group(0) @binding(0) var<storage, read> quads: array<Quad>;

struct Vertex {
    @builtin(position) position: vec4<f32>,
    @location(0) @interpolate(flat) projection0: vec3<f32>,
    @location(1) @interpolate(flat) projection1: vec3<f32>,
    @location(2) @interpolate(flat) projection2: vec3<f32>,
    @location(3) @interpolate(flat) color: vec4<f32>,
    @location(4) @interpolate(flat) guarded: f32,
};

@vertex
fn vertex(@builtin(vertex_index) index: u32) -> Vertex {
    let quad = quads[index / 6u];
    let corners = array<vec2<f32>, 6>(
        vec2<f32>(0.0, 1.0), vec2<f32>(1.0, 1.0), vec2<f32>(0.0, 0.0),
        vec2<f32>(0.0, 0.0), vec2<f32>(1.0, 1.0), vec2<f32>(1.0, 0.0),
    );
    let corner = corners[index % 6u];
    let expanded = expanded_quad_uv(
        vec3<f32>(quad.size_and_guarded.x, 0.0, 0.0),
        vec3<f32>(0.0, quad.size_and_guarded.y, 0.0),
        corner,
        GREEDY_EDGE_OVERLAP,
    );
    let projection = quad_projection(quad.anchor, quad.u, quad.v);
    var out: Vertex;
    out.position = quad.anchor + quad.u * expanded.x + quad.v * expanded.y;
    out.projection0 = projection[0];
    out.projection1 = projection[1];
    out.projection2 = projection[2];
    out.color = quad.color;
    out.guarded = quad.size_and_guarded.z;
    return out;
}

@fragment
fn fragment(in: Vertex) -> @location(0) vec4<f32> {
    if (in.guarded >= 5.0) {
        let origin = select(vec2<f32>(0.0), vec2<f32>(37.0, 53.0), in.guarded >= 7.0);
        if (in.guarded == 5.0 || in.guarded == 7.0) {
            var face: GreedyOut;
            face.clip_position = vec4<f32>(in.position.xy + origin, in.position.zw);
            face.projection_u = in.projection0;
            face.projection_v = in.projection1;
            face.projection_w = in.projection2;
            if (greedy_coverage(face).outside) { discard; }
        } else {
            var face: ModelOut;
            face.clip_position = vec4<f32>(in.position.xy + origin, in.position.zw);
            face.projection_u = in.projection0;
            face.projection_v = in.projection1;
            face.projection_w = in.projection2;
            if (model_coverage(face).outside) { discard; }
        }
        return in.color;
    }
    if (in.guarded >= 3.0) {
        let p = select(vec2<f32>(0.25, 0.75), vec2<f32>(0.75, 0.25), in.guarded > 3.5);
        let uv = model_quad_uv(p, vec2<f32>(0.0, 0.0), vec2<f32>(0.0, 1.0), vec2<f32>(0.3, 0.6), vec2<f32>(1.0, 0.0));
        return vec4<f32>(uv, 0.0, 1.0);
    }
    let projection = mat3x3<f32>(in.projection0, in.projection1, in.projection2);
    let origin = select(vec2<f32>(0.0), vec2<f32>(37.0, 53.0), in.guarded > 1.5);
    let uv = quad_uv_at(projection, in.position.xy + origin, vec2<f32>(384.0, 256.0), origin);
    if (in.guarded > 0.5 && !quad_contains(projection, uv, in.position.xy, vec2<f32>(384.0, 256.0))) {
        discard;
    }
    return in.color;
}
"#;

struct Sources;

impl Resolver for Sources {
    fn resolve_source<'a>(&'a self, path: &ModulePath) -> Result<Cow<'a, str>, ResolveError> {
        if path.origin == PathOrigin::Absolute && path.components == ["terrain_seams"] {
            return Ok(Cow::Borrowed(SHADER));
        }
        let file = Path::new(env!("CARGO_MANIFEST_DIR"))
            .join("src")
            .join(path.components.join("/"))
            .with_extension("wesl");
        std::fs::read_to_string(file)
            .map(Cow::Owned)
            .map_err(|error| ResolveError::ModuleNotFound(path.clone(), error.to_string()))
    }
}

#[derive(Clone, Copy)]
struct Face {
    anchor: DVec3,
    u: DVec3,
    v: DVec3,
    normal: DVec3,
    u_axis: DVec3,
    v_axis: DVec3,
    color: [u8; 4],
}

impl Face {
    fn new(anchor: DVec3, u: DVec3, v: DVec3, color: [u8; 4]) -> Self {
        Self {
            anchor,
            u,
            v,
            normal: u.cross(v),
            u_axis: u / u.length_squared(),
            v_axis: v / v.length_squared(),
            color,
        }
    }

    fn translated(self, offset: DVec3) -> Self {
        Self {
            anchor: self.anchor + offset,
            ..self
        }
    }
}

fn faces() -> Vec<Face> {
    let mut faces = vec![Face::new(
        DVec3::new(-3.0, 1.0, -1.0),
        DVec3::new(6.0, 0.0, 0.0),
        DVec3::new(0.0, 0.0, 2.0),
        [40, 180, 60, 255],
    )];
    for x in -3..3 {
        faces.push(Face::new(
            DVec3::new(f64::from(x), 1.0, 1.0),
            DVec3::X,
            -DVec3::Y,
            [180, 60, 30, 255],
        ));
    }
    faces.push(Face::new(
        DVec3::new(-3.0, 0.0, 1.0),
        DVec3::new(6.0, 0.0, 0.0),
        DVec3::new(0.0, 0.0, 2.0),
        [30, 90, 180, 255],
    ));
    faces.push(Face::new(
        DVec3::new(3.0, 1.0, -1.0),
        DVec3::new(0.0, 0.0, 2.0),
        -DVec3::Y,
        [180, 150, 30, 255],
    ));
    faces
}

struct Renderer {
    device: Device,
    queue: Queue,
    pipeline: RenderPipeline,
}

#[derive(Clone, Copy)]
enum Coverage {
    OriginalBounds,
    OffsetViewport,
    AffineUv,
    NonAffineUv,
    Greedy,
    Model,
    Expanded,
}

impl Renderer {
    fn new() -> Self {
        let instance = Instance::new(InstanceDescriptor::new_without_display_handle());
        let adapter = block_on(instance.request_adapter(&RequestAdapterOptions::default()))
            .ok()
            .or_else(|| {
                block_on(instance.request_adapter(&RequestAdapterOptions {
                    force_fallback_adapter: true,
                    ..Default::default()
                }))
                .ok()
            });
        let Some(adapter) = adapter else {
            panic!("no adapter");
        };
        let Ok((device, queue)) = block_on(adapter.request_device(&DeviceDescriptor {
            required_limits: adapter.limits(),
            ..Default::default()
        })) else {
            panic!("no GPU device");
        };
        let mut options = CompileOptions {
            imports: true,
            condcomp: true,
            visibility: false,
            ..Default::default()
        };
        options
            .features
            .flags
            .insert("SOLID".to_owned(), true.into());
        let source = wesl::compile(
            &ModulePath {
                origin: PathOrigin::Absolute,
                components: vec!["terrain_seams".to_owned()],
            },
            &options,
            &Sources,
        )
        .unwrap()
        .to_string();
        let module = device.create_shader_module(ShaderModuleDescriptor {
            label: Some("terrain seams"),
            source: ShaderSource::Wgsl(source.into()),
        });
        let pipeline = device.create_render_pipeline(&RenderPipelineDescriptor {
            label: Some("terrain seams"),
            layout: None,
            vertex: VertexState {
                module: &module,
                entry_point: Some("vertex"),
                compilation_options: PipelineCompilationOptions::default(),
                buffers: &[],
            },
            primitive: Default::default(),
            depth_stencil: Some(DepthStencilState {
                format: TextureFormat::Depth32Float,
                depth_write_enabled: Some(true),
                depth_compare: Some(CompareFunction::LessEqual),
                stencil: Default::default(),
                bias: Default::default(),
            }),
            multisample: Default::default(),
            fragment: Some(FragmentState {
                module: &module,
                entry_point: Some("fragment"),
                compilation_options: PipelineCompilationOptions::default(),
                targets: &[Some(ColorTargetState {
                    format: TextureFormat::Rgba8Unorm,
                    blend: None,
                    write_mask: ColorWrites::ALL,
                })],
            }),
            multiview_mask: None,
            cache: None,
        });
        Self {
            device,
            queue,
            pipeline,
        }
    }

    fn target(&self, format: TextureFormat, usage: TextureUsages) -> wgpu::Texture {
        self.device.create_texture(&TextureDescriptor {
            label: None,
            size: Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: TextureDimension::D2,
            format,
            usage,
            view_formats: &[],
        })
    }

    fn bindings(&self, faces: &[Face], camera: Mat4, coverage: Coverage) -> BindGroup {
        let mut quads = Vec::new();
        for face in faces {
            quads.extend((camera * face.anchor.as_vec3().extend(1.0)).to_array());
            quads.extend((camera * face.u.as_vec3().extend(0.0)).to_array());
            quads.extend((camera * face.v.as_vec3().extend(0.0)).to_array());
            quads.extend(face.color.map(|byte| f32::from(byte) / 255.0));
            quads.extend([
                face.u.length() as f32,
                face.v.length() as f32,
                match coverage {
                    Coverage::OriginalBounds => 1.0,
                    Coverage::OffsetViewport => 2.0,
                    Coverage::AffineUv => 3.0,
                    Coverage::NonAffineUv => 4.0,
                    Coverage::Greedy => 5.0,
                    Coverage::Model => 6.0,
                    Coverage::Expanded => 0.0,
                },
                0.0,
            ]);
        }
        let input = self.device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents: bytemuck::cast_slice(&quads),
            usage: BufferUsages::STORAGE,
        });
        let camera_scale = if matches!(coverage, Coverage::Greedy | Coverage::Model) {
            0.5
        } else {
            1.0
        };
        let mut camera = CameraUniform::default();
        camera.viewport = [WIDTH as f32 * camera_scale, HEIGHT as f32 * camera_scale];
        camera.raster_viewport = [WIDTH as f32, HEIGHT as f32];
        let uniform = self.device.create_buffer_init(&BufferInitDescriptor {
            label: None,
            contents: bytemuck::bytes_of(&camera),
            usage: BufferUsages::UNIFORM,
        });
        self.device.create_bind_group(&BindGroupDescriptor {
            label: None,
            layout: &self.pipeline.get_bind_group_layout(0),
            entries: &[
                BindGroupEntry {
                    binding: 0,
                    resource: input.as_entire_binding(),
                },
                BindGroupEntry {
                    binding: 1,
                    resource: uniform.as_entire_binding(),
                },
            ],
        })
    }

    fn image(&self, faces: &[Face], camera: Mat4, coverage: Coverage) -> Vec<u8> {
        let group = self.bindings(faces, camera, coverage);
        let color = self.target(
            TextureFormat::Rgba8Unorm,
            TextureUsages::RENDER_ATTACHMENT | TextureUsages::COPY_SRC,
        );
        let depth = self.target(
            TextureFormat::Depth32Float,
            TextureUsages::RENDER_ATTACHMENT,
        );
        let color_view = color.create_view(&Default::default());
        let depth_view = depth.create_view(&Default::default());
        let read = self.device.create_buffer(&BufferDescriptor {
            label: None,
            size: u64::from(WIDTH * HEIGHT * 4),
            usage: BufferUsages::MAP_READ | BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());
        {
            let mut pass = encoder.begin_render_pass(&RenderPassDescriptor {
                color_attachments: &[Some(RenderPassColorAttachment {
                    view: &color_view,
                    depth_slice: None,
                    resolve_target: None,
                    ops: Operations {
                        load: LoadOp::Clear(Color::BLACK),
                        store: StoreOp::Store,
                    },
                })],
                depth_stencil_attachment: Some(RenderPassDepthStencilAttachment {
                    view: &depth_view,
                    depth_ops: Some(Operations {
                        load: LoadOp::Clear(1.0),
                        store: StoreOp::Store,
                    }),
                    stencil_ops: None,
                }),
                ..Default::default()
            });
            pass.set_pipeline(&self.pipeline);
            pass.set_bind_group(0, &group, &[]);
            pass.draw(0..faces.len() as u32 * 6, 0..1);
        }
        encoder.copy_texture_to_buffer(
            color.as_image_copy(),
            TexelCopyBufferInfo {
                buffer: &read,
                layout: TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(WIDTH * 4),
                    rows_per_image: Some(HEIGHT),
                },
            },
            Extent3d {
                width: WIDTH,
                height: HEIGHT,
                depth_or_array_layers: 1,
            },
        );
        self.queue.submit([encoder.finish()]);
        read.slice(..).map_async(MapMode::Read, |_| {});
        self.device.poll(PollType::wait_indefinitely()).unwrap();
        let image = read.slice(..).get_mapped_range().unwrap().to_vec();
        read.unmap();
        image
    }
}

enum Expected {
    Color([u8; 4]),
    SharedEdge(Vec<[u8; 4]>),
    Silhouette,
}

#[test]
fn production_terrain_coverage_matches_the_reference() {
    let renderer = Renderer::new();
    let faces = faces();
    let camera = view_camera(DVec3::ZERO, Vec3::new(4.35, 3.2, 6.17));
    let expected = renderer.image(&faces, camera, Coverage::OriginalBounds);
    assert_ne!(
        expected,
        renderer.image(&faces, camera, Coverage::Expanded),
        "the scene must expose unguarded overlap"
    );
    for mode in [Coverage::Greedy, Coverage::Model] {
        assert_eq!(
            expected,
            renderer.image(&faces, camera, mode),
            "production coverage must follow the original faces"
        );
    }
}

#[test]
fn model_uv_reconstruction_preserves_both_original_triangles() {
    let renderer = Renderer::new();
    let face = [Face::new(
        DVec3::new(-1.0, -1.0, 0.5),
        DVec3::X * 2.0,
        DVec3::Y * 2.0,
        [255; 4],
    )];
    let pixel = ((HEIGHT / 2 * WIDTH + WIDTH / 2) * 4) as usize;
    let affine = renderer.image(&face, Mat4::IDENTITY, Coverage::AffineUv);
    let warped = renderer.image(&face, Mat4::IDENTITY, Coverage::NonAffineUv);
    assert_eq!(&affine[pixel..pixel + 4], &[19, 166, 0, 255]);
    assert_eq!(&warped[pixel..pixel + 4], &[147, 38, 0, 255]);
}

#[test]
fn a_pixel_on_a_shared_edge_has_the_same_color_in_either_draw_order() {
    let renderer = Renderer::new();
    let mut faces = vec![
        Face::new(
            DVec3::new(-1.0, -1.0, 0.5),
            DVec3::X,
            DVec3::Y * 2.0,
            [255, 0, 0, 255],
        ),
        Face::new(
            DVec3::new(0.0, -1.0, 0.5),
            DVec3::X,
            DVec3::Y * 2.0,
            [0, 0, 255, 255],
        ),
    ];
    let pixel = ((HEIGHT / 2 * WIDTH + WIDTH / 2) * 4) as usize;
    for shift_pixels in [-0.25, -0.003, -0.00002, 0.0, 0.00002, 0.003, 0.25] {
        let camera = Mat4::from_translation(Vec3::new(
            (1.0 + 2.0 * shift_pixels) / WIDTH as f32,
            0.0,
            0.0,
        ));
        let first = renderer.image(&faces, camera, Coverage::OriginalBounds);
        let offset_viewport = renderer.image(&faces, camera, Coverage::OffsetViewport);
        assert_eq!(
            first, offset_viewport,
            "a shifted viewport must shade the same image"
        );
        faces.reverse();
        let second = renderer.image(&faces, camera, Coverage::OriginalBounds);
        faces.reverse();
        assert_eq!(
            &first[pixel..pixel + 4],
            &second[pixel..pixel + 4],
            "edge at {shift_pixels} pixels depends on draw order"
        );
        assert_ne!(
            &first[pixel..pixel + 4],
            &BACKGROUND,
            "edge at {shift_pixels} pixels must remain covered"
        );
        if shift_pixels <= -0.001 {
            assert_eq!(&first[pixel..pixel + 4], &[0, 0, 255, 255]);
        } else if shift_pixels >= 0.001 {
            assert_eq!(&first[pixel..pixel + 4], &[255, 0, 0, 255]);
        }
    }
    for angle in [
        0.0,
        std::f32::consts::FRAC_PI_2,
        std::f32::consts::FRAC_PI_2 - 1e-6,
        std::f32::consts::FRAC_PI_2 + 1e-6,
    ] {
        let camera =
            Mat4::from_translation(Vec3::new(1.0 / WIDTH as f32, -1.0 / HEIGHT as f32, 0.0))
                * Mat4::from_rotation_z(angle);
        let first = renderer.image(&faces, camera, Coverage::OriginalBounds);
        faces.reverse();
        let second = renderer.image(&faces, camera, Coverage::OriginalBounds);
        faces.reverse();
        assert_eq!(
            &first[pixel..pixel + 4],
            &second[pixel..pixel + 4],
            "edge angle {angle} depends on draw order"
        );
        assert_ne!(
            &first[pixel..pixel + 4],
            &BACKGROUND,
            "edge angle {angle} must remain covered"
        );
    }
}

struct RayHit {
    color: [u8; 4],
    boundary: bool,
}

fn ray_hit(faces: &[Face], inverse: DMat4, x: f64, y: f64) -> RayHit {
    let ndc = DVec4::new(
        x * 2.0 / f64::from(WIDTH) - 1.0,
        1.0 - y * 2.0 / f64::from(HEIGHT),
        0.0,
        1.0,
    );
    let near = inverse * ndc;
    let far = inverse * DVec4::new(ndc.x, ndc.y, 1.0, 1.0);
    let origin = near.truncate() / near.w;
    let direction = far.truncate() / far.w - origin;
    let mut nearest = f64::INFINITY;
    let mut color = BACKGROUND;
    let mut boundary = false;
    for face in faces {
        let denominator = face.normal.dot(direction);
        if denominator.abs() < 1e-12 {
            continue;
        }
        let distance = face.normal.dot(face.anchor - origin) / denominator;
        if distance < 0.0 {
            continue;
        }
        let point = origin + direction * distance - face.anchor;
        let u = point.dot(face.u_axis);
        let v = point.dot(face.v_axis);
        let tolerance = 2e-5;
        if u >= -tolerance && u <= 1.0 + tolerance && v >= -tolerance && v <= 1.0 + tolerance {
            boundary |= u
                .abs()
                .min((1.0 - u).abs())
                .min(v.abs())
                .min((1.0 - v).abs())
                <= tolerance;
        }
        if (0.0..=1.0).contains(&u) && (0.0..=1.0).contains(&v) && distance < nearest {
            nearest = distance;
            color = face.color;
        }
    }
    RayHit { color, boundary }
}

fn expected(faces: &[Face], inverse: DMat4, x: u32, y: u32) -> Expected {
    let x = f64::from(x) + 0.5;
    let y = f64::from(y) + 0.5;
    let hit = ray_hit(faces, inverse, x, y);
    if !hit.boundary {
        return Expected::Color(hit.color);
    }
    let mut colors = Vec::new();
    for dx in [-0.01, 0.0, 0.01] {
        for dy in [-0.01, 0.0, 0.01] {
            if dx == 0.0 && dy == 0.0 {
                continue;
            }
            let color = ray_hit(faces, inverse, x + dx, y + dy).color;
            if color == BACKGROUND {
                return Expected::Silhouette;
            }
            if !colors.contains(&color) {
                colors.push(color);
            }
        }
    }
    Expected::SharedEdge(colors)
}

const VIEWS: [(DVec3, Vec3); 10] = [
    (DVec3::ZERO, Vec3::new(4.35, 3.2, 6.17)),
    (DVec3::ZERO, Vec3::new(0.37, 2.7, 4.03)),
    (DVec3::ZERO, Vec3::new(-4.21, 2.13, 5.71)),
    (DVec3::ZERO, Vec3::new(4.3513, 3.2007, 6.1703)),
    (DVec3::ZERO, Vec3::new(1.13, 1.03, 4.7)),
    (DVec3::ZERO, Vec3::new(0.37, 1.1, 1.15)),
    (DVec3::ZERO, Vec3::new(0.0, 3.0, 6.0)),
    (DVec3::ZERO, Vec3::new(0.0, 2.123, 5.57)),
    (
        DVec3::new(192.0, -16.0, -192.0),
        Vec3::new(196.35, -12.8, -185.83),
    ),
    (
        DVec3::new(192.0, -16.0, -192.0),
        Vec3::new(195.7, -14.65, -132.0),
    ),
];

fn view_camera(offset: DVec3, eye: Vec3) -> Mat4 {
    let target = offset.as_vec3() + Vec3::new(0.0, 0.5, 0.5);
    let mut camera = proj::perspective(1.05, WIDTH as f32 / HEIGHT as f32, 0.1, 400.0)
        * Transform::from_translation(eye)
            .looking_at(target, Vec3::Y)
            .to_matrix()
            .inverse();
    if (eye.x - offset.x as f32).abs() < 1e-5 {
        let edge = camera * (offset.as_vec3() + Vec3::new(0.0, 1.0, 1.0)).extend(1.0);
        let shift = Vec3::new(
            1.0 / WIDTH as f32 - edge.x / edge.w,
            -1.0 / HEIGHT as f32 - edge.y / edge.w,
            0.0,
        );
        camera = Mat4::from_translation(shift) * camera;
    }
    camera
}

#[test]
fn expanded_terrain_covers_shared_edges_without_leaking_a_neighbours_color() {
    let renderer = Renderer::new();
    let base_faces = faces();
    let mut unguarded_errors = 0;
    let mut shared_edge_samples = 0;
    for (offset, eye) in VIEWS {
        let mut faces: Vec<Face> = base_faces
            .iter()
            .map(|face| face.translated(offset))
            .collect();
        let camera = view_camera(offset, eye);
        let inverse = camera.as_dmat4().inverse();
        let oracle: Vec<Expected> = (0..HEIGHT)
            .flat_map(|y| (0..WIDTH).map(move |x| (x, y)))
            .map(|(x, y)| expected(&faces, inverse, x, y))
            .collect();
        let mut first_image = None;
        for order in 0..2 {
            let image = renderer.image(&faces, camera, Coverage::OriginalBounds);
            let unguarded = renderer.image(&faces, camera, Coverage::Expanded);
            let mut checked = 0;
            for y in 0..HEIGHT {
                for x in 0..WIDTH {
                    let index = (y * WIDTH + x) as usize;
                    let offset = index * 4;
                    let actual: [u8; 4] = image[offset..offset + 4].try_into().unwrap();
                    let expanded: [u8; 4] = unguarded[offset..offset + 4].try_into().unwrap();
                    match &oracle[index] {
                        Expected::Color(color) => {
                            unguarded_errors += usize::from(&expanded != color);
                            assert_eq!(
                                &actual, color,
                                "eye {eye}, order {order}, pixel ({x}, {y})"
                            );
                        }
                        Expected::SharedEdge(colors) => {
                            assert!(
                                colors.contains(&actual),
                                "shared opaque edge has a hole or foreign color: eye {eye}, order {order}, pixel ({x}, {y}), actual {actual:?}, allowed {colors:?}"
                            );
                            unguarded_errors += usize::from(!colors.contains(&expanded));
                            shared_edge_samples += 1;
                        }
                        Expected::Silhouette => continue,
                    }
                    checked += 1;
                }
            }
            assert!(
                checked > WIDTH * HEIGHT * 98 / 100,
                "the oracle must cover almost every pixel"
            );
            if let Some(first) = &first_image {
                assert!(
                    first == &image,
                    "shared-edge ownership depends on draw order: {eye}"
                );
            } else {
                first_image = Some(image.clone());
            }
            faces.reverse();
        }
    }
    assert!(
        unguarded_errors > 0,
        "the scene must detect the color spill of unguarded expansion"
    );
    assert!(
        shared_edge_samples > 0,
        "the scene must check coverage at shared opaque edges"
    );
}
