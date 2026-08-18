// Terrain rasterisation by vertex pulling: no vertex buffer, no index buffer, one draw per stream.
//
// Each instance is one quad. `instance_index` reads the compacted list the culling pass produced,
// which yields an index into the geometry arena; `vertex_index % 6` picks the corner. Greedy quads
// unpack from eight bytes, baked model quads from four twelve-byte vertices.

#import bevy_render::view::View

struct Params {
    group_base: u32,
    group_count: u32,
    visible_base: u32,
    args_index: u32,
    min_section_y: i32,
    wireframe: u32,
    // Explicit scalars rather than a vec3: a vec3 would align to 16 and silently grow the struct
    // past the 32 bytes the dynamic uniform offsets are laid out on.
    pad0: u32,
    pad1: u32,
}

@group(0) @binding(0) var<uniform> view: View;
@group(0) @binding(1) var<uniform> params: Params;

@group(1) @binding(0) var<storage, read> quads: array<vec2<u32>>;
@group(1) @binding(1) var<storage, read> vertices: array<u32>;
@group(1) @binding(2) var<storage, read> visible: array<u32>;
@group(1) @binding(3) var atlas: texture_2d_array<f32>;
@group(1) @binding(4) var atlas_sampler: sampler;
@group(1) @binding(5) var tints: texture_2d_array<f32>;
@group(1) @binding(6) var tint_sampler: sampler;

struct VertexOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) @interpolate(flat) layer: u32,
    @location(2) shade: f32,
    @location(3) world_xz: vec2<f32>,
    @location(4) @interpolate(flat) tint_kind: u32,
    /// Corner position within the quad, so the fragment can find the quad's own edges. Greedy
    /// quads cover many blocks, so this is not the same thing as the texture coordinate.
    @location(5) quad_uv: vec2<f32>,
    /// Signed distance to the diagonal the quad is split along, in quad-local units. Which of the
    /// two diagonals that is depends on the winding, so it is resolved here rather than in the
    /// fragment, which would otherwise need the winding flag too.
    @location(6) diagonal: f32,
};

/// Quad corners in the order vanilla winds them, as (u, v).
fn corner_uv(index: u32) -> vec2<f32> {
    switch index {
        case 0u: { return vec2<f32>(0.0, 0.0); }
        case 1u: { return vec2<f32>(0.0, 1.0); }
        case 2u: { return vec2<f32>(1.0, 1.0); }
        default: { return vec2<f32>(1.0, 0.0); }
    }
}

/// Two triangles from four corners. The second winding is used when the ambient occlusion gradient
/// is anisotropic, so the split follows the darker diagonal instead of cutting across it.
fn corner_index(vertex: u32, flip: bool) -> u32 {
    if (flip) {
        switch vertex {
            case 0u: { return 1u; }
            case 1u: { return 2u; }
            case 2u: { return 3u; }
            case 3u: { return 1u; }
            case 4u: { return 3u; }
            default: { return 0u; }
        }
    }
    switch vertex {
        case 0u: { return 0u; }
        case 1u: { return 1u; }
        case 2u: { return 2u; }
        case 3u: { return 0u; }
        case 4u: { return 2u; }
        default: { return 3u; }
    }
}

/// World direction the sprite's `u` runs along for each face, sign folded in.
fn face_u_dir(face: u32) -> vec3<f32> {
    switch face {
        case 0u, 1u, 3u: { return vec3<f32>(1.0, 0.0, 0.0); }
        case 2u: { return vec3<f32>(-1.0, 0.0, 0.0); }
        case 4u: { return vec3<f32>(0.0, 0.0, 1.0); }
        default: { return vec3<f32>(0.0, 0.0, -1.0); }
    }
}

fn face_v_dir(face: u32) -> vec3<f32> {
    switch face {
        case 0u: { return vec3<f32>(0.0, 0.0, -1.0); }
        case 1u: { return vec3<f32>(0.0, 0.0, 1.0); }
        default: { return vec3<f32>(0.0, -1.0, 0.0); }
    }
}

/// Vanilla's directional face shade, from `CardinalLighting`.
fn face_shade(face: u32) -> f32 {
    switch face {
        case 0u: { return 0.5; }
        case 1u: { return 1.0; }
        case 2u, 3u: { return 0.8; }
        default: { return 0.6; }
    }
}

/// Vanilla's light curve at full daylight, with the small ambient floor the client also applies.
fn light_curve(level: f32) -> f32 {
    let f = level / 15.0;
    return mix(0.05, 1.0, f / (4.0 - 3.0 * f));
}

/// Ambient occlusion samples average four neighbours that are each 0.2 when solid and 1.0 when
/// open, so the four reachable values are evenly spaced from 0.4 to 1.0.
fn ao_factor(bits: u32, corner: u32) -> f32 {
    return 0.4 + f32((bits >> (corner * 2u)) & 3u) * 0.2;
}

@vertex
fn vertex_simple(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> VertexOut {
    let quad = quads[visible[params.visible_base + instance]];
    let lo = quad.x;
    let hi = quad.y;

    let anchor = vec3<f32>(
        f32(lo & 0x3ffu),
        f32((lo >> 10u) & 0x1ffu) - 64.0,
        f32((lo >> 19u) & 0x3ffu),
    );
    let face = (lo >> 29u) & 7u;
    let size = vec2<f32>(f32((hi & 0xfu) + 1u), f32(((hi >> 4u) & 0xfu) + 1u));
    let ao_bits = (hi >> 17u) & 0xffu;
    let light = f32((hi >> 25u) & 0xfu);
    let flip = ((hi >> 29u) & 1u) == 1u;

    let corner = corner_index(vertex, flip);
    let quad_uv = corner_uv(corner);
    let c = quad_uv * size;
    let world = anchor + face_u_dir(face) * c.x + face_v_dir(face) * c.y;

    var out: VertexOut;
    out.clip_position = view.clip_from_world * vec4<f32>(world, 1.0);
    out.uv = c;
    out.layer = (hi >> 8u) & 0x1ffu;
    out.shade = face_shade(face) * light_curve(light) * ao_factor(ao_bits, corner);
    out.world_xz = world.xz;
    out.tint_kind = (hi >> 30u) & 3u;
    out.quad_uv = quad_uv;
    out.diagonal = select(
        quad_uv.x - quad_uv.y,
        quad_uv.x + quad_uv.y - 1.0,
        flip,
    );
    return out;
}

@vertex
fn vertex_complex(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> VertexOut {
    let quad = visible[params.visible_base + instance];
    let corner = corner_index(vertex, false);
    let base = (quad * 4u + corner) * 3u;
    let w0 = vertices[base];
    let w1 = vertices[base + 1u];
    let w2 = vertices[base + 2u];

    // Fixed point: 1/32 of a block, biased two blocks; y also carries the region's Y bias.
    let world = vec3<f32>(
        f32(w0 & 0xffffu) / 32.0 - 2.0,
        f32((w0 >> 16u) & 0xffffu) / 32.0 - 2.0 - 64.0,
        f32(w1 & 0xffffu) / 32.0 - 2.0,
    );
    let u = f32((w1 >> 16u) & 0x3ffu) / 1023.0;
    let v = f32((w2 >> 16u) & 0x3ffu) / 1023.0;
    let light = f32((w2 >> 26u) & 0xfu);
    let shade_bucket = (w2 >> 30u) & 3u;
    var shade = 1.0;
    switch shade_bucket {
        case 0u: { shade = 0.5; }
        case 1u: { shade = 0.8; }
        case 2u: { shade = 0.6; }
        default: { shade = 1.0; }
    }

    var out: VertexOut;
    out.clip_position = view.clip_from_world * vec4<f32>(world, 1.0);
    out.uv = vec2<f32>(u, v);
    out.layer = w2 & 0x1ffu;
    out.shade = shade * light_curve(light);
    out.world_xz = world.xz;
    out.tint_kind = (w1 >> 26u) & 3u;
    let quad_uv = corner_uv(corner);
    out.quad_uv = quad_uv;
    out.diagonal = quad_uv.x - quad_uv.y;
    return out;
}

/// Distance from the fragment to the nearest edge of its own triangle, measured in pixels: the
/// screen-space derivative of a quad-local coordinate is how much of the quad one pixel spans, so
/// dividing by it keeps the outline a pixel wide whether the quad is one block across or a chunk
/// across. The split diagonal counts as an edge, so what is drawn is the triangle count.
fn edge_pixels(in: VertexOut) -> f32 {
    let width = max(fwidth(in.quad_uv), vec2<f32>(1e-6));
    let border = min(
        min(in.quad_uv.x, 1.0 - in.quad_uv.x) / width.x,
        min(in.quad_uv.y, 1.0 - in.quad_uv.y) / width.y,
    );
    let diagonal = abs(in.diagonal) / max(fwidth(in.diagonal), 1e-6);
    return min(border, diagonal);
}

fn shade_sample(in: VertexOut) -> vec4<f32> {
    let color = textureSample(atlas, atlas_sampler, in.uv, in.layer);
    // Both samples are unconditional: `tint_kind` varies between instances inside one draw, so a
    // branch around a `textureSample` would not be uniform control flow and WGSL rejects it.
    let tint = textureSample(
        tints,
        tint_sampler,
        in.world_xz / 512.0,
        max(in.tint_kind, 1u) - 1u,
    );
    let factor = select(vec3<f32>(1.0), tint.rgb, in.tint_kind != 0u);
    return vec4<f32>(color.rgb * factor * in.shade, color.a);
}

/// Keeping only the outline leaves the quad's interior unwritten, so the depth buffer stays open
/// there and the geometry behind shows through, all the way to the sky.
fn wireframe_discards(in: VertexOut) -> bool {
    // Taken before any `discard`: that demotes the invocation to a helper, and the neighbours a
    // derivative reads may be exactly the ones that were discarded.
    return params.wireframe != 0u && edge_pixels(in) > 1.0;
}

@fragment
fn fragment_opaque(in: VertexOut) -> @location(0) vec4<f32> {
    let color = shade_sample(in);
    let hidden = wireframe_discards(in);
    if (color.a < 0.5 || hidden) {
        discard;
    }
    return vec4<f32>(color.rgb, 1.0);
}

@fragment
fn fragment_blend(in: VertexOut) -> @location(0) vec4<f32> {
    let color = shade_sample(in);
    if (wireframe_discards(in)) {
        discard;
    }
    return color;
}
