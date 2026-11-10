// The draw of the impostors into the gbuffer: one quad for each draw record of the prepare
// pass. The quad turns about the vertical axis of the entity position to face the camera and
// shows the baked view of the nearest yaw and elevation.

@group(0) @binding(0) var<uniform> frame: Frame;
@group(0) @binding(24) var<storage, read> impostor_draws: array<ImpostorDraw>;
@group(1) @binding(0) var impostor_colours: texture_2d_array<f32>;
@group(1) @binding(1) var impostor_normals: texture_2d_array<f32>;

const TILE: u32 = #{TILE}u;
const NO_ENTRY: u32 = 0xffffu;

/// The corners of the two triangles of the quad: across the view, then up.
const QUAD_CORNERS = array<vec2<f32>, 6>(
    vec2<f32>(0.0, 0.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(0.0, 1.0),
    vec2<f32>(1.0, 0.0),
    vec2<f32>(1.0, 1.0),
);

struct VertexOutput {
    @builtin(position) clip: vec4<f32>,
    /// The point of the quad across the view and above the entity position, in blocks at the
    /// scale 1.
    @location(0) quad: vec2<f32>,
    /// The index of the draw record.
    @location(1) @interpolate(flat) index: u32,
}

/// Six vertices for each draw record. A draw of one instance; an instanced strip of four
/// vertices costs more on the reference machine.
@vertex
fn vertex(@builtin(vertex_index) vertex_index: u32) -> VertexOutput {
    let index = vertex_index / 6u;
    var corners = QUAD_CORNERS;
    let corner = corners[vertex_index % 6u];
    let draw = impostor_draws[index];
    let bounds = draw.bounds;
    let quad = vec2<f32>(mix(bounds.x, bounds.y, corner.x), mix(bounds.z, bounds.w, corner.y));
    let across = vec3<f32>(draw.axes.x, 0.0, draw.axes.y);
    let world = draw.origin_yaw.xyz + across * quad.x + vec3<f32>(0.0, quad.y * draw.axes.z, 0.0);
    var out: VertexOutput;
    out.clip = frame.clip_from_relative * vec4<f32>(world, 1.0);
    out.quad = quad;
    out.index = index;
    return out;
}

/// The covered pixel of the image of one layer that is nearest to the camera so far.
struct Nearest {
    depth: f32,
    colour: vec3<f32>,
    normal: vec3<f32>,
}

/// Takes the pixel of the image set `entry` with the view rectangle `rect` at the quad point
/// `quad` when it is covered and not farther than `nearest`. A later layer wins a tie, as in the
/// draw of the full rig.
fn take_layer(nearest: ptr<function, Nearest>, entry: u32, rect: vec4<f32>, quad: vec2<f32>, tile: vec2<u32>) {
    if (entry == NO_ENTRY) {
        return;
    }
    let uv = vec2<f32>((quad.x - rect.x) / (rect.y - rect.x), (rect.w - quad.y) / (rect.w - rect.z));
    if (any(uv < vec2<f32>(0.0)) || any(uv >= vec2<f32>(1.0))) {
        return;
    }
    let texel = vec2<i32>(tile + min(vec2<u32>(uv * f32(TILE)), vec2<u32>(TILE - 1u)));
    let sample = textureLoad(impostor_colours, texel, entry, 0);
    if (sample.a < 0.5) {
        return;
    }
    let normal_depth = textureLoad(impostor_normals, texel, entry, 0);
    if (normal_depth.a >= (*nearest).depth) {
        (*nearest).depth = normal_depth.a;
        (*nearest).colour = sample.rgb;
        (*nearest).normal = normal_depth.xyz * 2.0 - 1.0;
    }
}

@fragment
fn fragment(in: VertexOutput) -> GBuffer {
    let draw = impostor_draws[in.index];
    let tile_bits = bitcast<u32>(draw.axes.w);
    let tile = vec2<u32>(tile_bits & 0xffffu, tile_bits >> 16u);
    var nearest = Nearest(-1.0, vec3<f32>(0.0), vec3<f32>(0.0, 1.0, 0.0));
    take_layer(&nearest, draw.words.x & 0xffffu, draw.rects[0], in.quad, tile);
    take_layer(&nearest, draw.words.x >> 16u, draw.rects[1], in.quad, tile);
    take_layer(&nearest, draw.words.y & 0xffffu, draw.rects[2], in.quad, tile);
    take_layer(&nearest, draw.words.y >> 16u, draw.rects[3], in.quad, tile);
    if (nearest.depth < 0.0) {
        discard;
    }
    return shade_impostor(draw, nearest);
}

/// The fragment of a small impostor: a pixel that no layer covers takes the pixel at the middle
/// of the quad, the centroid of the image, so the draw needs no discard.
@fragment
fn fragment_small(in: VertexOutput) -> GBuffer {
    let draw = impostor_draws[in.index];
    let tile_bits = bitcast<u32>(draw.axes.w);
    let tile = vec2<u32>(tile_bits & 0xffffu, tile_bits >> 16u);
    var nearest = Nearest(-1.0, vec3<f32>(0.0), vec3<f32>(0.0, 1.0, 0.0));
    take_layer(&nearest, draw.words.x & 0xffffu, draw.rects[0], in.quad, tile);
    take_layer(&nearest, draw.words.x >> 16u, draw.rects[1], in.quad, tile);
    take_layer(&nearest, draw.words.y & 0xffffu, draw.rects[2], in.quad, tile);
    take_layer(&nearest, draw.words.y >> 16u, draw.rects[3], in.quad, tile);
    if (nearest.depth < 0.0) {
        let middle = vec2<f32>(draw.bounds.x + draw.bounds.y, draw.bounds.z + draw.bounds.w) * 0.5;
        take_layer(&nearest, draw.words.x & 0xffffu, draw.rects[0], middle, tile);
    }
    return shade_impostor(draw, nearest);
}

fn shade_impostor(draw: ImpostorDraw, nearest: Nearest) -> GBuffer {
    let m = rotation_y(180.0 - draw.origin_yaw.w) * normalize(nearest.normal);
    let overlay = select(vec4<f32>(0.0, 0.0, 0.0, 1.0), vec4<f32>(1.0, 0.0, 0.0, RED_OVERLAY_ALPHA), draw.words.w != 0u);
    let light = vec2<f32>(f32(draw.words.z & 0xffffu), f32(draw.words.z >> 16u));
    return shade(nearest.colour, m, overlay, light);
}
