
import mcrs_minecraft_render::shaders::include::fields::{
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
import mcrs_minecraft_render::shaders::include::fog::{fog_mix, fog_value};
@if(IMPROVED) import mcrs_minecraft_render::shaders::include::fog::fog_values;
@if(IMPROVED) import mcrs_minecraft_render::shaders::include::transparency::transparent_fog;
import mcrs_minecraft_render::shaders::include::frame::{camera, params};
import mcrs_minecraft_render::shaders::include::lighting::face_shade;
import mcrs_minecraft_render::shaders::include::quad::{
    GREEDY_EDGE_OVERLAP, corner_index, degenerate, face_normal, face_u_dir, face_v_dir,
    greedy_corner,
};
@if(SOLID) import mcrs_minecraft_render::shaders::include::quad_coverage::{quad_projection, quad_uv_at, quad_contains};
import mcrs_minecraft_render::shaders::include::section::section_origin;
import mcrs_minecraft_render::shaders::include::surface::{Surface, shade_surface};
import mcrs_minecraft_render::shaders::include::terrain_bindings::{
    TerrainVertex, face_field, quad_field, quad_ref, quad_words, section_desc,
};

struct GreedyOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) quad_uv: vec2<f32>,
    @location(1) world: vec3<f32>,
    @location(2) @interpolate(flat) face_base: u32,
    @location(3) @interpolate(flat) face_span: vec2<u32>,
    @location(4) @interpolate(flat) directional: f32,
    @location(5) @interpolate(flat) face: u32,
    @if(SOLID) @location(6) @interpolate(flat) projection_u: vec3<f32>,
    @if(SOLID) @location(7) @interpolate(flat) projection_v: vec3<f32>,
    @if(SOLID) @location(8) @interpolate(flat) projection_w: vec3<f32>,
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
    @if(SOLID) let overlap = GREEDY_EDGE_OVERLAP;
    @else let overlap = 0.0;
    let words = quad_words(quad);
    let origin = section_origin(desc);
    let scale = f32(desc.scale);
    let corner = greedy_corner(words, origin, scale, corner_index(in.index), overlap);
    let world = corner.world;
    let quad_uv = corner.quad_uv;
    @if(SOLID) {
        let size = vec2<f32>(span) * scale;
        let u = face_u_dir(face) * size.x;
        let v = face_v_dir(face) * size.y;
        let anchor = greedy_corner(words, origin, scale, 2u, 0.0).world - u - v;
        let projection = quad_projection(
            camera.clip_from_relative * vec4<f32>(anchor, 1.0),
            camera.clip_from_relative * vec4<f32>(u, 0.0),
            camera.clip_from_relative * vec4<f32>(v, 0.0),
        );
        out.projection_u = projection[0];
        out.projection_v = projection[1];
        out.projection_w = projection[2];
    }

    out.clip_position = camera.clip_from_relative * vec4<f32>(world, 1.0);
    out.quad_uv = quad_uv;
    out.world = world;
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

    let fluid = face_field(face.attr, FACE_FLUID_WORD, FACE_FLUID_SHIFT, FACE_FLUID_BITS) != 0u;
    let scale = select(1.0, 0.5, fluid);

    let orientation = face_field(face.attr, FACE_UV_WORD, FACE_UV_SHIFT, FACE_UV_BITS);
    let uv = oriented_uv(face.uv, face.cell, orientation);

    var s: Surface;
    s.sprite = face_field(face.attr, FACE_SPRITE_WORD, FACE_SPRITE_SHIFT, FACE_SPRITE_BITS);
    s.tint_kind = face_field(face.attr, FACE_TINT_WORD, FACE_TINT_SHIFT, FACE_TINT_BITS);
    s.shade = vec3<f32>(1.0);
    s.uv = select(uv, fract(face.uv) * 0.5, fluid);
    s.world_xz = in.world.xz;
    s.ddx = oriented_gradient(dpdx(face.uv), orientation) * scale;
    s.ddy = oriented_gradient(dpdy(face.uv), orientation) * scale;
    return s;
}

fn oriented_uv(uv: vec2<f32>, cell: vec2<u32>, orientation: u32) -> vec2<f32> {
    var inside = clamp(uv - vec2<f32>(cell), vec2<f32>(0.0), vec2<f32>(1.0));
    if ((orientation & 1u) != 0u) {
        inside = inside.yx;
    }
    if ((orientation & 2u) != 0u) {
        inside.x = 1.0 - inside.x;
    }
    if ((orientation & 4u) != 0u) {
        inside.y = 1.0 - inside.y;
    }
    return inside;
}

fn oriented_gradient(gradient: vec2<f32>, orientation: u32) -> vec2<f32> {
    return select(gradient, gradient.yx, (orientation & 1u) != 0u);
}

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
    let f = clamp(face.uv - vec2<f32>(face.cell), vec2<f32>(0.0), vec2<f32>(1.0));
    if (f.y > f.x) {
        return corners[0] + (corners[2] - corners[1]) * f.x + (corners[1] - corners[0]) * f.y;
    }
    return corners[0] + (corners[3] - corners[0]) * f.x + (corners[2] - corners[3]) * f.y;
}

fn greedy_gbuffer(in: GreedyOut, albedo: vec3<f32>) -> mcrs_minecraft_render::shaders::include::gbuffer_encode::GBuffer {
    let light = greedy_light(in);
    return mcrs_minecraft_render::shaders::include::deferred::gbuffer(albedo, light.x, face_normal(in.face), light.y, light.z);
}

struct GreedyCoverage {
    surface: GreedyOut,
    outside: bool,
};

fn greedy_coverage(in: GreedyOut) -> GreedyCoverage {
    var surface = in;
    @if(SOLID) {
        let projection = mat3x3<f32>(in.projection_u, in.projection_v, in.projection_w);
        surface.quad_uv = quad_uv_at(projection, in.clip_position.xy, camera.raster_viewport, vec2<f32>(0.0));
        return GreedyCoverage(surface, !quad_contains(projection, surface.quad_uv, in.clip_position.xy, camera.raster_viewport));
    } @else {
        return GreedyCoverage(surface, false);
    }
}

@fragment
fn fragment_greedy_solid(in: GreedyOut) -> mcrs_minecraft_render::shaders::include::gbuffer_encode::GBuffer {
    let coverage = greedy_coverage(in);
    let wireframe_discards = mcrs_minecraft_render::shaders::include::finish::wireframe_discards(coverage.surface.quad_uv);
    let color = shade_surface(greedy_surface(coverage.surface));
    if (coverage.outside || wireframe_discards) {
        discard;
    }
    return greedy_gbuffer(coverage.surface, color.rgb);
}

@fragment
fn fragment_greedy_cutout(in: GreedyOut) -> mcrs_minecraft_render::shaders::include::gbuffer_encode::GBuffer {
    let coverage = greedy_coverage(in);
    let wireframe_discards = mcrs_minecraft_render::shaders::include::finish::wireframe_discards(coverage.surface.quad_uv);
    let color = shade_surface(greedy_surface(coverage.surface));
    if (coverage.outside || color.a < 0.5 || wireframe_discards) {
        discard;
    }
    return greedy_gbuffer(coverage.surface, color.rgb);
}

@fragment
fn fragment_greedy_translucent(in: GreedyOut) -> @location(0) vec4<f32> {
    let wireframe_discards = mcrs_minecraft_render::shaders::include::finish::wireframe_discards(in.quad_uv);
    let color = shade_surface(greedy_surface(in));
    if (wireframe_discards) {
        discard;
    }
    let light = greedy_light(in);
    let lit = mcrs_minecraft_render::shaders::include::deferred::shade(color.rgb, light.x, light.y, light.z);
    @if(IMPROVED) {
        let values = fog_values(in.world - camera.offset, camera.fog_environmental, camera.fog_render_distance);
        let pixel = vec2<i32>(in.clip_position.xy);
        let fogged = transparent_fog(lit, pixel, in.clip_position.z, values, camera.fog_color);
        return vec4<f32>(fogged, color.a);
    }
    let fog = fog_value(in.world - camera.offset, camera.fog_environmental, camera.fog_render_distance);
    return vec4<f32>(fog_mix(lit, fog, camera.fog_color), color.a);
}
