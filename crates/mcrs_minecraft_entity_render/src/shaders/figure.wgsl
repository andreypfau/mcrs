struct FigureLight {
    light0: vec4<f32>,
    light1: vec4<f32>,
}

const TARGET_ENCODES: bool = #{TARGET_ENCODES};

@group(2) @binding(0) var<uniform> figure_light: FigureLight;
@group(2) @binding(1) var lightmap_levels: texture_2d<f32>;
@group(2) @binding(2) var lightmap_sampler: sampler;

fn figure_lightmap(block: f32, sky: f32) -> vec3<f32> {
    let uv = clamp(
        vec2<f32>(block, sky) / 256.0 + 0.5 / 16.0,
        vec2<f32>(0.5 / 16.0),
        vec2<f32>(15.5 / 16.0),
    );
    return textureSampleLevel(lightmap_levels, lightmap_sampler, uv, 0.0).rgb;
}

fn figure_colour(in: VertexOutput, texel: vec3<f32>) -> vec4<f32> {
    let n = normalize(in.normal);
    let m = select(n, -n, dot(n, frame.camera_offset - in.world) < 0.0);
    let factor = face_factor(m, figure_light.light0.xyz, figure_light.light1.xyz);
    let colour = mix(in.overlay.rgb, texel * factor, in.overlay.a);
    let lit = colour * figure_lightmap(in.light.x, in.light.y);
    return vec4<f32>(select(lit, to_target(lit), TARGET_ENCODES), 1.0);
}

fn to_target(raw: vec3<f32>) -> vec3<f32> {
    let low = raw / 12.92;
    let high = pow((raw + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, raw <= vec3<f32>(0.04045));
}

@fragment
fn figure_fragment(in: VertexOutput) -> @location(0) vec4<f32> {
    let texel = textureSample(entity_textures, entity_sampler, in.uv, in.layer);
    if (texel.a < CUTOUT_ALPHA) {
        discard;
    }
    return figure_colour(in, texel.rgb);
}

@fragment
fn figure_fragment_opaque(in: VertexOutput) -> @location(0) vec4<f32> {
    return figure_colour(in, textureSample(entity_textures, entity_sampler, in.uv, in.layer).rgb);
}
