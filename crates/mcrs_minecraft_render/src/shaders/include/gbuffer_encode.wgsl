#define_import_path mcrs_minecraft_client::gbuffer_encode

struct GBuffer {
    @location(0) albedo_ao: vec4<f32>,
    @location(1) normal_motion: vec4<f32>,
    @location(2) light: vec4<f32>,
};

// `sign` maps 0 to 0, which would fold an axis normal onto the wrong half of the octahedron.
fn sign_not_zero(v: vec2<f32>) -> vec2<f32> {
    return select(vec2<f32>(-1.0), vec2<f32>(1.0), v >= vec2<f32>(0.0));
}

fn octahedral_encode(n: vec3<f32>) -> vec2<f32> {
    let p = n.xy / (abs(n.x) + abs(n.y) + abs(n.z));
    return select(p, (1.0 - abs(p.yx)) * sign_not_zero(p), n.z < 0.0);
}
