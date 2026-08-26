
#import anvil_region_viewer::fields::{
    FACE_AO_SHIFT, FACE_AO_BITS,
    FACE_ARRAY_SHIFT, FACE_ARRAY_BITS,
    FACE_BLOCK_LIGHT_SHIFT, FACE_BLOCK_LIGHT_BITS,
    FACE_FLUID_SHIFT, FACE_FLUID_BITS,
    FACE_LAYER_SHIFT, FACE_LAYER_BITS,
    FACE_SKY_LIGHT_SHIFT, FACE_SKY_LIGHT_BITS,
    FACE_TINT_SHIFT, FACE_TINT_BITS,
    FLUID_INSET,
    MODEL_STEPS,
    QUAD_DROP_WORD, QUAD_DROP_SHIFT, QUAD_DROP_BITS,
    QUAD_FACE_WORD, QUAD_FACE_SHIFT, QUAD_FACE_BITS,
    QUAD_FACE_BASE_WORD, QUAD_FACE_BASE_SHIFT, QUAD_FACE_BASE_BITS,
    QUAD_FLUID_WORD, QUAD_FLUID_SHIFT, QUAD_FLUID_BITS,
    QUAD_H_WORD, QUAD_H_SHIFT, QUAD_H_BITS,
    QUAD_SECTION_WORD, QUAD_SECTION_SHIFT, QUAD_SECTION_BITS,
    QUAD_W_WORD, QUAD_W_SHIFT, QUAD_W_BITS,
    QUAD_WORDS,
    QUAD_X_WORD, QUAD_X_SHIFT, QUAD_X_BITS,
    QUAD_Y_WORD, QUAD_Y_SHIFT, QUAD_Y_BITS,
    QUAD_Z_WORD, QUAD_Z_SHIFT, QUAD_Z_BITS,
}
#import anvil_region_viewer::finish::{finish_cutout, finish_solid, finish_translucent}
#import anvil_region_viewer::frame::{params, region_origin, view}
#import anvil_region_viewer::lighting::{ao_factor, face_shade, lightmap}
#import anvil_region_viewer::quad::{
    corner_index, corner_uv, face_normal, face_u_dir, face_v_dir,
}
#import anvil_region_viewer::region::{CULLED, degenerate, section_origin}
#import anvil_region_viewer::surface::{Surface, shade_surface}
#import anvil_region_viewer::terrain_bindings::{faces, quad_field, visible}

struct GreedyOut {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) quad_uv: vec2<f32>,
    @location(1) world_xz: vec2<f32>,
    @location(2) @interpolate(flat) face_base: u32,
    @location(3) @interpolate(flat) face_span: vec2<u32>,
    @location(4) @interpolate(flat) directional: f32,
};

@vertex
fn vertex_greedy(
    @builtin(vertex_index) vertex: u32,
    @builtin(instance_index) instance: u32,
) -> GreedyOut {
    let culled = visible[params.visible_base + instance];
    var out: GreedyOut;
    if (culled == CULLED) {
        out.clip_position = degenerate();
        return out;
    }
    let quad = culled * QUAD_WORDS;

    let section = quad_field(quad, QUAD_SECTION_WORD, QUAD_SECTION_SHIFT, QUAD_SECTION_BITS);
    let anchor = section_origin(section, region_origin())
        + vec3<f32>(
            f32(quad_field(quad, QUAD_X_WORD, QUAD_X_SHIFT, QUAD_X_BITS)),
            f32(quad_field(quad, QUAD_Y_WORD, QUAD_Y_SHIFT, QUAD_Y_BITS)),
            f32(quad_field(quad, QUAD_Z_WORD, QUAD_Z_SHIFT, QUAD_Z_BITS)),
        );
    let face = quad_field(quad, QUAD_FACE_WORD, QUAD_FACE_SHIFT, QUAD_FACE_BITS);
    let span = vec2<u32>(
        quad_field(quad, QUAD_W_WORD, QUAD_W_SHIFT, QUAD_W_BITS) + 1u,
        quad_field(quad, QUAD_H_WORD, QUAD_H_SHIFT, QUAD_H_BITS) + 1u,
    );
    let size = vec2<f32>(span);

    // Fluids sit below the top of their block, so the surface drops and the sides shorten.
    let drop = f32(quad_field(quad, QUAD_DROP_WORD, QUAD_DROP_SHIFT, QUAD_DROP_BITS)) / MODEL_STEPS;
    var quad_uv = corner_uv(corner_index(vertex));
    if (face >= 2u) {
        quad_uv.y = max(quad_uv.y, drop / size.y);
    }
    let c = quad_uv * size;
    let u_dir = face_u_dir(face);
    let v_dir = face_v_dir(face);
    var world = anchor + u_dir * c.x + v_dir * c.y;
    if (face == 1u) {
        world.y -= drop;
    }
    // A fluid face shares a plane with the block face behind it, so it is pulled in slightly.
    let fluid = quad_field(quad, QUAD_FLUID_WORD, QUAD_FLUID_SHIFT, QUAD_FLUID_BITS);
    world -= face_normal(u_dir, v_dir) * (FLUID_INSET * f32(fluid));

    out.clip_position = view.clip_from_world * vec4<f32>(world, 1.0);
    out.quad_uv = quad_uv;
    out.world_xz = world.xz;
    out.face_base = params.face_origin
        + faces[params.face_origin + section]
        + quad_field(quad, QUAD_FACE_BASE_WORD, QUAD_FACE_BASE_SHIFT, QUAD_FACE_BASE_BITS);
    out.face_span = span;
    out.directional = face_shade(face);
    return out;
}

fn greedy_surface(in: GreedyOut) -> Surface {
    let uv = in.quad_uv * vec2<f32>(in.face_span);
    let cell = min(vec2<u32>(max(uv, vec2<f32>(0.0))), in.face_span - vec2<u32>(1u));
    let attr = faces[in.face_base + cell.y * in.face_span.x + cell.x];
    let block_light = f32(extractBits(attr, FACE_BLOCK_LIGHT_SHIFT, FACE_BLOCK_LIGHT_BITS));
    let sky_light = f32(extractBits(attr, FACE_SKY_LIGHT_SHIFT, FACE_SKY_LIGHT_BITS));
    let ao_bits = extractBits(attr, FACE_AO_SHIFT, FACE_AO_BITS);
    let f = uv - vec2<f32>(cell);
    let ao = mix(
        mix(ao_factor(ao_bits, 0u), ao_factor(ao_bits, 3u), f.x),
        mix(ao_factor(ao_bits, 1u), ao_factor(ao_bits, 2u), f.x),
        f.y,
    );

    // A fluid sprite is drawn at half scale and repeats, so its gradients halve with it.
    let fluid = extractBits(attr, FACE_FLUID_SHIFT, FACE_FLUID_BITS) != 0u;
    let scale = select(1.0, 0.5, fluid);

    var s: Surface;
    s.layer = extractBits(attr, FACE_LAYER_SHIFT, FACE_LAYER_BITS);
    s.array = extractBits(attr, FACE_ARRAY_SHIFT, FACE_ARRAY_BITS);
    s.tint_kind = extractBits(attr, FACE_TINT_SHIFT, FACE_TINT_BITS);
    s.shade = lightmap(block_light, sky_light) * (in.directional * ao);
    s.uv = select(uv, fract(uv) * 0.5, fluid);
    s.world_xz = in.world_xz;
    s.ddx = dpdx(uv) * scale;
    s.ddy = dpdy(uv) * scale;
    return s;
}

@fragment
fn fragment_greedy_solid(in: GreedyOut) -> @location(0) vec4<f32> {
    return finish_solid(shade_surface(greedy_surface(in)));
}

@fragment
fn fragment_greedy_cutout(in: GreedyOut) -> @location(0) vec4<f32> {
    return finish_cutout(shade_surface(greedy_surface(in)), in.quad_uv);
}

@fragment
fn fragment_greedy_translucent(in: GreedyOut) -> @location(0) vec4<f32> {
    return finish_translucent(shade_surface(greedy_surface(in)), in.quad_uv);
}
