
import package::shaders::include::fields::{
    FACE_AO_WORD, FACE_AO_SHIFT, FACE_AO_BITS, FACE_AO_CORNER_BITS,
    FACE_BLOCK_LIGHT_WORD, FACE_BLOCK_LIGHT_SHIFT, FACE_BLOCK_LIGHT_BITS,
    FACE_LIGHT_CORNER_BITS,
    FACE_FLUID_WORD, FACE_FLUID_SHIFT, FACE_FLUID_BITS,
    FACE_SKY_LIGHT_WORD, FACE_SKY_LIGHT_SHIFT, FACE_SKY_LIGHT_BITS,
    FACE_SPRITE_WORD, FACE_SPRITE_SHIFT, FACE_SPRITE_BITS,
    FACE_TINT_WORD, FACE_TINT_SHIFT, FACE_TINT_BITS,
    FACE_UV_WORD, FACE_UV_SHIFT, FACE_UV_BITS,
    FACE_WORDS,
    QUAD_FACE_WORD, QUAD_FACE_SHIFT, QUAD_FACE_BITS,
    QUAD_FACE_BASE_WORD, QUAD_FACE_BASE_SHIFT, QUAD_FACE_BASE_BITS,
    QUAD_H_WORD, QUAD_H_SHIFT, QUAD_H_BITS,
    QUAD_W_WORD, QUAD_W_SHIFT, QUAD_W_BITS,
    QUAD_WORDS,
};
import package::shaders::include::frame::{camera, params};
import package::shaders::include::lighting::face_shade;
import package::shaders::include::quad::{corner_index, degenerate, face_normal, greedy_corner};
import package::shaders::include::section::section_origin;
import package::shaders::include::surface::{Surface, shade_surface};
import package::shaders::include::terrain_bindings::{
    TerrainVertex, face_field, quad_field, quad_ref, quad_words, section_desc,
};

struct GreedyOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) quad_uv: vec2<f32>,
    @location(1) world_xz: vec2<f32>,
    @location(2) @interpolate(flat) face_base: u32,
    @location(3) @interpolate(flat) face_span: vec2<u32>,
    @location(4) @interpolate(flat) directional: f32,
    @location(5) @interpolate(flat) face: u32,
};

@vertex
fn vertex_greedy(in: TerrainVertex) -> GreedyOut {
    var out: GreedyOut;
    let at = quad_ref(in);
    if (!at.drawn) {
        out.clip_position = degenerate();
        return out;
    }
    let quad = at.quad * QUAD_WORDS;

    let desc = section_desc(at.section);
    let face = quad_field(quad, QUAD_FACE_WORD, QUAD_FACE_SHIFT, QUAD_FACE_BITS);
    let span = vec2<u32>(
        quad_field(quad, QUAD_W_WORD, QUAD_W_SHIFT, QUAD_W_BITS) + 1u,
        quad_field(quad, QUAD_H_WORD, QUAD_H_SHIFT, QUAD_H_BITS) + 1u,
    );
    let corner = greedy_corner(quad_words(quad), section_origin(desc), f32(desc.scale), corner_index(in.index));
    let world = corner.world;
    let quad_uv = corner.quad_uv;

    out.clip_position = camera.clip_from_relative * vec4<f32>(world, 1.0);
    out.quad_uv = quad_uv;
    out.world_xz = world.xz;
    out.face_base = desc.face_base
        + quad_field(quad, QUAD_FACE_BASE_WORD, QUAD_FACE_BASE_SHIFT, QUAD_FACE_BASE_BITS);
    out.face_span = span;
    out.directional = face_shade(face, camera.shade_table);
    out.face = face;
    return out;
}

struct GreedyFace {
    uv: vec2<f32>,
    cell: vec2<u32>,
    attr: u32,
    ao: u32,
    block: u32,
    sky: u32,
};

fn greedy_face(in: GreedyOut) -> GreedyFace {
    var face: GreedyFace;
    face.uv = in.quad_uv * vec2<f32>(in.face_span);
    face.cell = min(vec2<u32>(max(face.uv, vec2<f32>(0.0))), in.face_span - vec2<u32>(1u));
    face.attr = (in.face_base + face.cell.y * in.face_span.x + face.cell.x) * FACE_WORDS;
    face.ao = face_field(face.attr, FACE_AO_WORD, FACE_AO_SHIFT, FACE_AO_BITS);
    face.block = face_field(face.attr, FACE_BLOCK_LIGHT_WORD, FACE_BLOCK_LIGHT_SHIFT, FACE_BLOCK_LIGHT_BITS);
    face.sky = face_field(face.attr, FACE_SKY_LIGHT_WORD, FACE_SKY_LIGHT_SHIFT, FACE_SKY_LIGHT_BITS);
    return face;
}

fn greedy_surface(in: GreedyOut) -> Surface {
    let face = greedy_face(in);

    // A fluid sprite is drawn at half scale and repeats, so its gradients halve with it.
    let fluid = face_field(face.attr, FACE_FLUID_WORD, FACE_FLUID_SHIFT, FACE_FLUID_BITS) != 0u;
    let scale = select(1.0, 0.5, fluid);

    let orientation = face_field(face.attr, FACE_UV_WORD, FACE_UV_SHIFT, FACE_UV_BITS);
    let uv = oriented_uv(face.uv, face.cell, orientation);

    var s: Surface;
    s.sprite = face_field(face.attr, FACE_SPRITE_WORD, FACE_SPRITE_SHIFT, FACE_SPRITE_BITS);
    s.tint_kind = face_field(face.attr, FACE_TINT_WORD, FACE_TINT_SHIFT, FACE_TINT_BITS);
    s.shade = vec3<f32>(1.0);
    s.uv = select(uv, fract(face.uv) * 0.5, fluid);
    s.world_xz = in.world_xz;
    s.ddx = oriented_gradient(dpdx(face.uv), orientation) * scale;
    s.ddy = oriented_gradient(dpdy(face.uv), orientation) * scale;
    return s;
}

/// The sprite coordinates at `uv` of a face whose cell `cell` lays its sprite with the
/// orientation bits of `UvOrientation`: swap, then mirror u, then mirror v.
fn oriented_uv(uv: vec2<f32>, cell: vec2<u32>, orientation: u32) -> vec2<f32> {
    var inside = uv - vec2<f32>(cell);
    if ((orientation & 1u) != 0u) {
        inside = inside.yx;
    }
    if ((orientation & 2u) != 0u) {
        inside.x = 1.0 - inside.x;
    }
    if ((orientation & 4u) != 0u) {
        inside.y = 1.0 - inside.y;
    }
    return vec2<f32>(cell) + inside;
}

fn oriented_gradient(gradient: vec2<f32>, orientation: u32) -> vec2<f32> {
    return select(gradient, gradient.yx, (orientation & 1u) != 0u);
}

/// A corner's vertex colour as vanilla computes it, before the lightmap: the occlusion byte
/// scaled by the face shade, over 255, and the light in sixteenths. Light is stored in quarter
/// levels.
fn corner_light(ao: u32, block: u32, sky: u32, corner: u32, directional: f32) -> vec3<f32> {
    let ao_mask = (1u << FACE_AO_CORNER_BITS) - 1u;
    let light_mask = (1u << FACE_LIGHT_CORNER_BITS) - 1u;
    let code = (ao >> (corner * FACE_AO_CORNER_BITS)) & ao_mask;
    let byte = floor(f32(255u - 51u * code) * directional);
    let block_units = f32(((block >> (corner * FACE_LIGHT_CORNER_BITS)) & light_mask) * 4u);
    let sky_units = f32(((sky >> (corner * FACE_LIGHT_CORNER_BITS)) & light_mask) * 4u);
    return vec3<f32>(byte / 255.0, block_units, sky_units);
}

fn greedy_light(in: GreedyOut) -> vec3<f32> {
    let face = greedy_face(in);
    var corners: array<vec3<f32>, 4>;
    for (var k = 0u; k < 4u; k++) {
        corners[k] = corner_light(face.ao, face.block, face.sky, k, in.directional);
    }
    let f = face.uv - vec2<f32>(face.cell);
    // Vanilla draws each block face as two triangles split from corner 0 to corner 2, and the
    // colour is interpolated across each triangle on its own, not bilinearly across the face.
    if (f.y > f.x) {
        return corners[0] + (corners[2] - corners[1]) * f.x + (corners[1] - corners[0]) * f.y;
    }
    return corners[0] + (corners[3] - corners[0]) * f.x + (corners[2] - corners[3]) * f.y;
}

fn greedy_gbuffer(in: GreedyOut, albedo: vec3<f32>) -> package::shaders::include::gbuffer_encode::GBuffer {
    let light = greedy_light(in);
    return package::shaders::include::deferred::gbuffer(albedo, light.x, face_normal(in.face), light.y, light.z);
}

@fragment
fn fragment_greedy_solid(in: GreedyOut) -> package::shaders::include::gbuffer_encode::GBuffer {
    let wireframe_discards = package::shaders::include::finish::wireframe_discards(in.quad_uv);
    let color = shade_surface(greedy_surface(in));
    if (wireframe_discards) {
        discard;
    }
    return greedy_gbuffer(in, color.rgb);
}

@fragment
fn fragment_greedy_cutout(in: GreedyOut) -> package::shaders::include::gbuffer_encode::GBuffer {
    // Taken before the test: behind a short-circuit the derivative inside would sit in
    // non-uniform control flow, which WebGPU rejects for the whole module.
    let wireframe_discards = package::shaders::include::finish::wireframe_discards(in.quad_uv);
    let color = shade_surface(greedy_surface(in));
    if (color.a < 0.5 || wireframe_discards) {
        discard;
    }
    return greedy_gbuffer(in, color.rgb);
}

@fragment
fn fragment_greedy_translucent(in: GreedyOut) -> @location(0) vec4<f32> {
    let wireframe_discards = package::shaders::include::finish::wireframe_discards(in.quad_uv);
    let color = shade_surface(greedy_surface(in));
    if (wireframe_discards) {
        discard;
    }
    let light = greedy_light(in);
    let lit = package::shaders::include::deferred::shade(color.rgb, light.x, light.y, light.z);
    return vec4<f32>(package::shaders::include::finish::to_target(lit), color.a);
}
