// The draw of the face records of one texture array into the gbuffer, and the bake of the
// impostor images from the same face records.

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(5) var<storage, read> states: array<StateRecord>;
@group(0) @binding(8) var<uniform> models: array<ModelRecord, #{MODELS}>;
@group(0) @binding(10) var<storage, read> cuboids: array<CuboidRecord>;
@group(0) @binding(11) var<storage, read> visible: array<vec2<u32>>;
@group(0) @binding(12) var<storage, read> bone_matrices: array<vec4<f32>>;
@group(0) @binding(13) var<storage, read> instances: array<Instance>;
@group(0) @binding(14) var<storage, read> faces: array<vec2<u32>>;
@group(0) @binding(19) var<uniform> variants: array<Variant, #{VARIANTS}>;
@group(1) @binding(0) var entity_textures: texture_2d_array<f32>;
@group(1) @binding(1) var entity_sampler: sampler;

const CUTOUT_ALPHA: f32 = 0.1;

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) world: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) @interpolate(flat) layer: u32,
    @location(4) @interpolate(flat) overlay: vec4<f32>,
    @location(5) @interpolate(flat) light: vec2<f32>,
}

/// The corners of the two triangles of a face, counter-clockwise from its outer side.
const TRIANGLE_CORNERS = array<u32, 6>(0u, 2u, 1u, 1u, 2u, 3u);

/// A corner of a face after the transform of its bone, in model units.
struct FaceCorner {
    point: vec3<f32>,
    normal: vec3<f32>,
    uv: vec2<f32>,
    layer: u32,
    /// The index of the visible entry and of its instance.
    index: u32,
    /// The layer of the state record that draws the face.
    model_layer: u32,
}

/// The relative step of depth toward the camera for each later layer. Two layers can lie on one
/// surface, as the saddle on the inflated fur of a cold pig, and a later layer draws over an
/// earlier one there; the order of the face records in a draw is not the order of the layers.
const LAYER_DEPTH_STEP: f32 = 1.0 / 65536.0;

/// The corners of a face in the order of a triangle strip, which makes the triangles of
/// `TRIANGLE_CORNERS`.
const STRIP_CORNERS = array<u32, 4>(0u, 2u, 1u, 3u);

/// The corner `corner` of the face record `record_index`.
fn face_corner_of(record_index: u32, corner: u32) -> FaceCorner {
    let record = faces[record_index];
    let index = record.x;
    let cuboid_index = record.y & 0xffffu;
    let layer = (record.y >> 16u) & 0xffu;
    let face = record.y >> 24u;
    let entry = visible[index];
    let state = states[entry.x];
    let mask = layer_mask(state);
    var first_bone = entry.y;
    for (var before = 0u; before < layer; before = before + 1u) {
        if ((mask & (1u << before)) != 0u) {
            first_bone = first_bone + models[layer_word(state, before) & 0xffffu].bone_count;
        }
    }
    let word = layer_word(state, layer);
    let model = models[word & 0xffffu];
    let variant = variants[word >> 16u];
    let cuboid = cuboids[model.first_cuboid + cuboid_index];
    let bone = first_bone + cuboid_bone(cuboid);
    let b0 = bone_matrices[bone * 3u];
    let b1 = bone_matrices[bone * 3u + 1u];
    let b2 = bone_matrices[bone * 3u + 2u];
    var point: vec3<f32>;
    var normal: vec3<f32>;
    var uv: vec2<f32>;
    let flags = cuboid_flags(cuboid);
    if ((flags & CUBOID_EXPLICIT) != 0u) {
        let ordinal = countOneBits(flags & 0x3fu & ((1u << face) - 1u));
        let at = model.first_cuboid + model.cuboid_count + 2u * (bitcast<u32>(cuboid.origin_x) + ordinal);
        let a = cuboids[at];
        let b = cuboids[at + 1u];
        let edge_1 = vec3<f32>(a.size_x, a.size_y, a.size_z);
        let edge_2 = vec3<f32>(bitcast<f32>(a.uv), bitcast<f32>(a.inflate_bone_flags), b.origin_x);
        let right = (corner & 1u) != 0u;
        let low = (corner & 2u) != 0u;
        point = vec3<f32>(a.origin_x, a.origin_y, a.origin_z)
            + select(vec3<f32>(0.0), edge_2, right)
            + select(vec3<f32>(0.0), edge_1, low);
        normal = normalize(cross(edge_1, edge_2));
        uv = vec2<f32>(b.origin_y, b.origin_z)
            + select(vec2<f32>(0.0), vec2<f32>(b.size_z, bitcast<f32>(b.uv)), right)
            + select(vec2<f32>(0.0), vec2<f32>(b.size_x, b.size_y), low);
    } else {
        let box = drawn_box(cuboid);
        point = face_corner(face, corner, box.lo, box.hi);
        normal = face_normal(face);
        let texel = box_uv_corner(face, corner, cuboid_uv(cuboid), floor(cuboid_size(cuboid)), (flags & CUBOID_MIRROR) != 0u);
        uv = texel / vec2<f32>(model.texture_width, model.texture_height);
    }
    var out: FaceCorner;
    out.point = transform_point(b0, b1, b2, point);
    out.normal = transform_normal(b0, b1, b2, normal);
    out.uv = uv * vec2<f32>(variant.uv_scale_x, variant.uv_scale_y);
    out.layer = variant.array_layer & 0xffffu;
    out.index = index;
    out.model_layer = layer;
    return out;
}

/// One instance for each face, four vertices of a strip. The first vertex of the draw holds the
/// first face record of the texture array, four vertices for each record.
@vertex
fn vertex(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOutput {
    var strip = STRIP_CORNERS;
    let corner = face_corner_of(vertex_index / 4u + instance_index, strip[vertex_index % 4u]);
    let instance = instances[corner.index];
    let world = transform_point(instance.row0, instance.row1, instance.row2, corner.point);
    var out: VertexOutput;
    out.clip = frame.clip_from_relative * vec4<f32>(world, 1.0);
    out.clip.z = out.clip.z * (1.0 + f32(corner.model_layer) * LAYER_DEPTH_STEP);
    out.world = world;
    out.normal = transform_normal(instance.row0, instance.row1, instance.row2, corner.normal);
    out.uv = corner.uv;
    out.layer = corner.layer;
    out.overlay = instance.overlay;
    out.light = instance.light.xy;
    return out;
}

@fragment
fn fragment(in: VertexOutput) -> GBuffer {
    let texel = textureSample(entity_textures, entity_sampler, in.uv, in.layer);
    if (texel.a < CUTOUT_ALPHA) {
        discard;
    }
    return shade_face(in, texel.rgb);
}

/// The fragment of a face whose texture rectangle has no cutout texel.
@fragment
fn fragment_opaque(in: VertexOutput) -> GBuffer {
    return shade_face(in, textureSample(entity_textures, entity_sampler, in.uv, in.layer).rgb);
}

fn shade_face(in: VertexOutput, texel: vec3<f32>) -> GBuffer {
    let n = normalize(in.normal);
    let m = select(n, -n, dot(n, frame.camera_offset - in.world) < 0.0);
    return shade(texel, m, in.overlay, in.light);
}

struct BakeOutput {
    @builtin(position) clip: vec4<f32>,
    @location(0) normal: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) @interpolate(flat) layer: u32,
    /// The direction toward the camera of the view, in model space.
    @location(3) @interpolate(flat) toward: vec3<f32>,
}

/// A corner of the bake of an impostor image: the rows of the instance map model space to the
/// clip space of the image.
@vertex
fn bake_vertex(@builtin(vertex_index) vertex_index: u32) -> BakeOutput {
    var corners = TRIANGLE_CORNERS;
    let corner = face_corner_of(vertex_index / 6u, corners[vertex_index % 6u]);
    let instance = instances[corner.index];
    var out: BakeOutput;
    out.clip = vec4<f32>(transform_point(instance.row0, instance.row1, instance.row2, corner.point), 1.0);
    out.normal = corner.normal;
    out.uv = corner.uv;
    out.layer = corner.layer;
    out.toward = instance.row2.xyz;
    return out;
}

struct BakeTargets {
    @location(0) colour: vec4<f32>,
    /// The normal in model space as `n · 0.5 + 0.5`, and the depth toward the camera.
    @location(1) normal_depth: vec4<f32>,
}

@fragment
fn bake_fragment(in: BakeOutput) -> BakeTargets {
    let texel = textureSample(entity_textures, entity_sampler, in.uv, in.layer);
    if (texel.a < CUTOUT_ALPHA) {
        discard;
    }
    let n = normalize(in.normal);
    let m = select(n, -n, dot(n, in.toward) < 0.0);
    var out: BakeTargets;
    out.colour = vec4<f32>(texel.rgb, 1.0);
    out.normal_depth = vec4<f32>(m * 0.5 + 0.5, in.clip.z);
    return out;
}
