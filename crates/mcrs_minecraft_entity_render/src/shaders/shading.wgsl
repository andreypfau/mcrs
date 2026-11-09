// The colour of an entity pixel in the gbuffer, for the full rig and for the impostor.

/// The gbuffer of a texel colour `texel` with the world normal `m`: the two-light factor, then
/// the overlay.
fn shade(texel: vec3<f32>, m: vec3<f32>, overlay: vec4<f32>, light: vec2<f32>) -> GBuffer {
    let light0 = normalize(vec3<f32>(0.2, 1.0, -0.7));
    let nether = frame.shade_table == SHADE_TABLE_NETHER;
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
