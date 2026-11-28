
import mcrs_minecraft_render::shaders::include::fields::{
    MODEL_BLOCK_LIGHT_WORD, MODEL_BLOCK_LIGHT_SHIFT, MODEL_BLOCK_LIGHT_BITS,
    MODEL_SHADE_WORD, MODEL_SHADE_SHIFT, MODEL_SHADE_BITS,
    MODEL_SKY_LIGHT_WORD, MODEL_SKY_LIGHT_SHIFT, MODEL_SKY_LIGHT_BITS,
    MODEL_SPRITE_WORD, MODEL_SPRITE_SHIFT, MODEL_SPRITE_BITS,
    MODEL_TINT_WORD, MODEL_TINT_SHIFT, MODEL_TINT_BITS,
    MODEL_TINT_HIGH_WORD, MODEL_TINT_HIGH_SHIFT, MODEL_TINT_HIGH_BITS,
    MODEL_U_WORD, MODEL_U_SHIFT, MODEL_U_BITS,
    MODEL_V_WORD, MODEL_V_SHIFT, MODEL_V_BITS,
    MODEL_X_WORD, MODEL_Y_WORD, MODEL_Z_WORD,
};
import mcrs_minecraft_render::shaders::include::fog::{fog_mix, fog_value};
@if(IMPROVED) import mcrs_minecraft_render::shaders::include::fog::fog_values;
@if(IMPROVED) import mcrs_minecraft_render::shaders::include::transparency::transparent_fog;
import mcrs_minecraft_render::shaders::include::frame::{camera, params};
import mcrs_minecraft_render::shaders::include::quad::{
    CORNERS_PER_QUAD, GREEDY_EDGE_OVERLAP, MODEL_WORDS_PER_VERTEX, corner_index, corner_uv,
    degenerate, expanded_quad_uv, model_position, model_quad_uv,
};
@if(SOLID) import mcrs_minecraft_render::shaders::include::quad_coverage::{quad_projection, quad_uv_at, quad_contains};
import mcrs_minecraft_render::shaders::include::section::section_origin;
import mcrs_minecraft_render::shaders::include::surface::{Surface, shade_surface};
import mcrs_minecraft_render::shaders::include::terrain_bindings::{
    TerrainVertex, model_field, model_word, quad_ref, section_desc,
};


struct ModelOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) world: vec3<f32>,
    @location(2) block_light: f32,
    @location(3) @interpolate(flat) sprite: u32,
    @location(4) @interpolate(flat) tint_kind: u32,
    @location(5) quad_uv: vec2<f32>,
    @location(6) sky_light: f32,
    @location(7) shade: f32,
    @location(8) @interpolate(flat) normal: vec3<f32>,
    @if(SOLID) @location(9) @interpolate(flat) projection_u: vec3<f32>,
    @if(SOLID) @location(10) @interpolate(flat) projection_v: vec3<f32>,
    @if(SOLID) @location(11) @interpolate(flat) projection_w: vec3<f32>,
    @if(SOLID) @location(12) @interpolate(flat) uv_origin_u: vec4<f32>,
    @if(SOLID) @location(13) @interpolate(flat) uv_v: vec2<f32>,
    @if(SOLID) @location(14) @interpolate(flat) uv_cross: vec2<f32>,
};

fn model_local(base: u32) -> vec3<f32> {
    return model_position(
        model_word(base + MODEL_X_WORD),
        model_word(base + MODEL_Y_WORD),
        model_word(base + MODEL_Z_WORD),
    );
}

fn model_corner_uv(quad: u32, corner: u32) -> vec2<f32> {
    let base = (quad * CORNERS_PER_QUAD + corner) * MODEL_WORDS_PER_VERTEX;
    let uv_scale = f32((1u << MODEL_U_BITS) - 1u);
    return vec2<f32>(
        f32(model_field(base, MODEL_U_WORD, MODEL_U_SHIFT, MODEL_U_BITS)) / uv_scale,
        f32(model_field(base, MODEL_V_WORD, MODEL_V_SHIFT, MODEL_V_BITS)) / uv_scale,
    );
}

@vertex
fn vertex_model(in: TerrainVertex) -> ModelOut {
    var out: ModelOut;
    let at = quad_ref(in);
    if (!at.drawn) {
        out.clip_position = degenerate();
        return out;
    }
    let corner = corner_index(in.index);
    let base = (at.quad * CORNERS_PER_QUAD + corner) * MODEL_WORDS_PER_VERTEX;

    let desc = section_desc(at.section);
    let origin = section_origin(desc);
    let scale = f32(desc.scale);
    let p0 = model_corner(at.quad, 0u);
    let p1 = model_corner(at.quad, 1u);
    let p2 = model_corner(at.quad, 2u);
    let p3 = model_corner(at.quad, 3u);
    let u_vec = p3 - p0;
    let v_vec = p1 - p0;
    let is_parallelogram = all(abs(p0 + u_vec + v_vec - p2) < vec3<f32>(1e-4))
        && dot(u_vec, u_vec) > 1e-8
        && dot(v_vec, v_vec) > 1e-8;
    let uv0 = model_corner_uv(at.quad, 0u);
    let uv1 = model_corner_uv(at.quad, 1u);
    let uv2 = model_corner_uv(at.quad, 2u);
    let uv3 = model_corner_uv(at.quad, 3u);
    var quad_uv = corner_uv(corner);
    var world: vec3<f32>;
    var uv: vec2<f32>;
    @if(SOLID) {
        if (is_parallelogram) {
            quad_uv = expanded_quad_uv(u_vec, v_vec, quad_uv, GREEDY_EDGE_OVERLAP);
            world = origin + (p0 + u_vec * quad_uv.x + v_vec * quad_uv.y) * scale;
            let uv_u = uv3 - uv0;
            let uv_v = uv1 - uv0;
            uv = uv0 + uv_u * quad_uv.x + uv_v * quad_uv.y;
            let projection = quad_projection(
                camera.clip_from_relative * vec4<f32>(origin + p0 * scale, 1.0),
                camera.clip_from_relative * vec4<f32>(u_vec * scale, 0.0),
                camera.clip_from_relative * vec4<f32>(v_vec * scale, 0.0),
            );
            out.projection_u = projection[0];
            out.projection_v = projection[1];
            out.projection_w = projection[2];
            out.uv_origin_u = vec4<f32>(uv0, uv_u);
            out.uv_v = uv_v;
            out.uv_cross = uv0 + uv2 - uv1 - uv3;
        } else {
            world = origin + model_local(base) * scale;
            uv = model_corner_uv(at.quad, corner);
            out.projection_u = vec3<f32>(0.0);
            out.projection_v = vec3<f32>(0.0);
            out.projection_w = vec3<f32>(0.0);
            out.uv_origin_u = vec4<f32>(0.0);
            out.uv_v = vec2<f32>(0.0);
            out.uv_cross = vec2<f32>(0.0);
        }
    } @else {
        world = origin + model_local(base) * scale;
        uv = model_corner_uv(at.quad, corner);
    }

    let block_light =
        f32(model_field(base, MODEL_BLOCK_LIGHT_WORD, MODEL_BLOCK_LIGHT_SHIFT, MODEL_BLOCK_LIGHT_BITS));
    let sky_light =
        f32(model_field(base, MODEL_SKY_LIGHT_WORD, MODEL_SKY_LIGHT_SHIFT, MODEL_SKY_LIGHT_BITS));
    let shade = f32(model_field(base, MODEL_SHADE_WORD, MODEL_SHADE_SHIFT, MODEL_SHADE_BITS)) / 255.0;

    out.clip_position = camera.clip_from_relative * vec4<f32>(world, 1.0);
    out.uv = uv;
    out.sprite = model_field(base, MODEL_SPRITE_WORD, MODEL_SPRITE_SHIFT, MODEL_SPRITE_BITS);
    out.block_light = block_light;
    out.sky_light = sky_light;
    out.shade = shade;
    let tri0 = cross(p2 - p1, p0 - p1);
    let tri1 = cross(v_vec, u_vec);
    let raw_normal = select(tri1, tri0, dot(tri0, tri0) > 0.0);
    out.normal = normalize(select(vec3<f32>(0.0, 1.0, 0.0), raw_normal, dot(raw_normal, raw_normal) > 0.0));
    out.world = world;
    out.tint_kind = model_field(base, MODEL_TINT_WORD, MODEL_TINT_SHIFT, MODEL_TINT_BITS)
        | (model_field(base, MODEL_TINT_HIGH_WORD, MODEL_TINT_HIGH_SHIFT, MODEL_TINT_HIGH_BITS)
            << MODEL_TINT_BITS);
    out.quad_uv = quad_uv;
    return out;
}

fn model_corner(quad: u32, corner: u32) -> vec3<f32> {
    let base = (quad * CORNERS_PER_QUAD + corner) * MODEL_WORDS_PER_VERTEX;
    return model_local(base);
}

fn model_surface(in: ModelOut) -> Surface {
    var s: Surface;
    s.sprite = in.sprite;
    s.tint_kind = in.tint_kind;
    s.shade = vec3<f32>(1.0);
    s.world_xz = in.world.xz;
    @if(SOLID) {
        if (any(in.projection_w != vec3<f32>(0.0))) {
            let clamped = clamp(in.quad_uv, vec2<f32>(0.0), vec2<f32>(1.0));
            let uv0 = in.uv_origin_u.xy;
            let uv1 = uv0 + in.uv_v;
            let uv3 = uv0 + in.uv_origin_u.zw;
            let uv2 = in.uv_cross + uv1 + uv3 - uv0;
            s.uv = model_quad_uv(clamped, uv0, uv1, uv2, uv3);
        } else {
            s.uv = in.uv;
        }
    } @else {
        s.uv = in.uv;
    }
    s.ddx = dpdx(s.uv);
    s.ddy = dpdy(s.uv);
    return s;
}

fn model_gbuffer(in: ModelOut, albedo: vec3<f32>) -> mcrs_minecraft_render::shaders::include::gbuffer_encode::GBuffer {
    return mcrs_minecraft_render::shaders::include::deferred::gbuffer(albedo, in.shade, in.normal, in.block_light, in.sky_light);
}

struct ModelCoverage {
    surface: ModelOut,
    outside: bool,
};

fn model_coverage(in: ModelOut) -> ModelCoverage {
    var surface = in;
    @if(SOLID) {
        if (any(in.projection_w != vec3<f32>(0.0))) {
            let projection = mat3x3<f32>(in.projection_u, in.projection_v, in.projection_w);
            surface.quad_uv = quad_uv_at(projection, in.clip_position.xy, camera.raster_viewport, vec2<f32>(0.0));
            return ModelCoverage(surface, !quad_contains(projection, surface.quad_uv, in.clip_position.xy, camera.raster_viewport));
        }
    }
    return ModelCoverage(surface, false);
}

@fragment
fn fragment_model_solid(in: ModelOut) -> mcrs_minecraft_render::shaders::include::gbuffer_encode::GBuffer {
    let coverage = model_coverage(in);
    let wireframe_discards = mcrs_minecraft_render::shaders::include::finish::wireframe_discards(coverage.surface.quad_uv);
    let color = shade_surface(model_surface(coverage.surface));
    if (coverage.outside || wireframe_discards) {
        discard;
    }
    return model_gbuffer(coverage.surface, color.rgb);
}

@fragment
fn fragment_model_cutout(in: ModelOut) -> mcrs_minecraft_render::shaders::include::gbuffer_encode::GBuffer {
    let coverage = model_coverage(in);
    let wireframe_discards = mcrs_minecraft_render::shaders::include::finish::wireframe_discards(coverage.surface.quad_uv);
    let color = shade_surface(model_surface(coverage.surface));
    if (coverage.outside || color.a < 0.5 || wireframe_discards) {
        discard;
    }
    return model_gbuffer(coverage.surface, color.rgb);
}

@fragment
fn fragment_model_translucent(in: ModelOut) -> @location(0) vec4<f32> {
    let wireframe_discards = mcrs_minecraft_render::shaders::include::finish::wireframe_discards(in.quad_uv);
    let color = shade_surface(model_surface(in));
    if (wireframe_discards) {
        discard;
    }
    let lit = mcrs_minecraft_render::shaders::include::deferred::shade(color.rgb, in.shade, in.block_light, in.sky_light);
    @if(IMPROVED) {
        let values = fog_values(in.world - camera.offset, camera.fog_environmental, camera.fog_render_distance);
        let pixel = vec2<i32>(in.clip_position.xy);
        let fogged = transparent_fog(lit, pixel, in.clip_position.z, values, camera.fog_color);
        return vec4<f32>(fogged, color.a);
    }
    let fog = fog_value(in.world - camera.offset, camera.fog_environmental, camera.fog_render_distance);
    return vec4<f32>(fog_mix(lit, fog, camera.fog_color), color.a);
}
