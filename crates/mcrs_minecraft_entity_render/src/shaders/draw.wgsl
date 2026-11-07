// The draw of the face records of one texture array into the gbuffer.

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(5) var<storage, read> states: array<StateRecord>;
@group(0) @binding(8) var<storage, read> models: array<ModelRecord>;
@group(0) @binding(10) var<storage, read> cuboids: array<CuboidRecord>;
@group(0) @binding(11) var<storage, read> visible: array<vec2<u32>>;
@group(0) @binding(12) var<storage, read> bone_matrices: array<vec4<f32>>;
@group(0) @binding(13) var<storage, read> instances: array<Instance>;
@group(0) @binding(14) var<storage, read> faces: array<vec2<u32>>;
@group(0) @binding(19) var<storage, read> variants: array<Variant>;
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

@vertex
fn vertex(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let record = faces[vertex_index / 6u];
    var corners = TRIANGLE_CORNERS;
    let corner = corners[vertex_index % 6u];
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
    let placed = compose_rows(
        instances[index],
        bone_matrices[bone * 3u],
        bone_matrices[bone * 3u + 1u],
        bone_matrices[bone * 3u + 2u],
    );
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
    let world = transform_point(placed.row0, placed.row1, placed.row2, point);
    var out: VertexOutput;
    out.clip = frame.clip_from_relative * vec4<f32>(world, 1.0);
    out.world = world;
    out.normal = transform_normal(placed.row0, placed.row1, placed.row2, normal);
    out.uv = uv * vec2<f32>(variant.uv_scale_x, variant.uv_scale_y);
    out.layer = variant.array_layer & 0xffffu;
    let instance = instances[index];
    out.overlay = instance.overlay;
    out.light = instance.light.xy;
    return out;
}

struct GBuffer {
    @location(0) albedo_ao: vec4<f32>,
    @location(1) normal_motion: vec4<f32>,
    @location(2) light: vec4<f32>,
}

// The encoding of `deferred.wgsl` of the terrain renderer, which the lighting pass decodes.
fn sign_not_zero(v: vec2<f32>) -> vec2<f32> {
    return select(vec2<f32>(-1.0), vec2<f32>(1.0), v >= vec2<f32>(0.0));
}

fn octahedral_encode(n: vec3<f32>) -> vec2<f32> {
    let p = n.xy / (abs(n.x) + abs(n.y) + abs(n.z));
    return select(p, (1.0 - abs(p.yx)) * sign_not_zero(p), n.z < 0.0);
}

@fragment
fn fragment(in: VertexOutput) -> GBuffer {
    let texel = textureSample(entity_textures, entity_sampler, in.uv, in.layer);
    if (texel.a < CUTOUT_ALPHA) {
        discard;
    }
    let n = normalize(in.normal);
    let m = select(n, -n, dot(n, frame.camera_offset - in.world) < 0.0);
    let light0 = normalize(vec3<f32>(0.2, 1.0, -0.7));
    let nether = (frame.flags & FRAME_NETHER_LIGHT) != 0u;
    let light1 = normalize(vec3<f32>(-0.2, select(1.0, -1.0, nether), 0.7));
    let factor = min(1.0, (max(0.0, dot(m, light0)) + max(0.0, dot(m, light1))) * 0.6 + 0.4);
    var colour = texel.rgb * factor;
    colour = mix(in.overlay.rgb, colour, in.overlay.a);
    var out: GBuffer;
    out.albedo_ao = vec4<f32>(colour, 1.0);
    out.normal_motion = vec4<f32>(octahedral_encode(m), 0.0, 0.0);
    out.light = vec4<f32>(0.0, in.light.x, in.light.y, 0.0);
    return out;
}
