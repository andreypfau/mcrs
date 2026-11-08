// The colour of an entity pixel in the gbuffer, for the full rig and for the impostor.

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

/// The gbuffer of a texel colour `texel` with the world normal `m`: the two-light factor, then
/// the overlay.
fn shade(texel: vec3<f32>, m: vec3<f32>, overlay: vec4<f32>, light: vec2<f32>) -> GBuffer {
    let light0 = normalize(vec3<f32>(0.2, 1.0, -0.7));
    let nether = (frame.flags & FRAME_NETHER_LIGHT) != 0u;
    let light1 = normalize(vec3<f32>(-0.2, select(1.0, -1.0, nether), 0.7));
    let factor = min(1.0, (max(0.0, dot(m, light0)) + max(0.0, dot(m, light1))) * 0.6 + 0.4);
    var colour = texel * factor;
    colour = mix(overlay.rgb, colour, overlay.a);
    var out: GBuffer;
    out.albedo_ao = vec4<f32>(colour, 1.0);
    out.normal_motion = vec4<f32>(octahedral_encode(m), 0.0, 0.0);
    out.light = vec4<f32>(0.0, light.x, light.y, 0.0);
    return out;
}
