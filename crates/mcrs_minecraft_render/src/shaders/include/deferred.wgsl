#define_import_path mcrs_minecraft_client::deferred

#import mcrs_minecraft_client::lighting::lightmap

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

fn octahedral_decode(e: vec2<f32>) -> vec3<f32> {
    let z = 1.0 - abs(e.x) - abs(e.y);
    let xy = select(e, (1.0 - abs(e.yx)) * sign_not_zero(e), z < 0.0);
    return normalize(vec3<f32>(xy, z));
}

/// Block and sky light are in sixteenths of a level, as `lightmap` takes them.
fn gbuffer(albedo: vec3<f32>, ao: f32, normal: vec3<f32>, block: f32, sky: f32) -> GBuffer {
    var out: GBuffer;
    out.albedo_ao = vec4<f32>(albedo, ao);
    out.normal_motion = vec4<f32>(octahedral_encode(normalize(normal)), 0.0, 0.0);
    out.light = vec4<f32>(0.0, block, sky, 0.0);
    return out;
}

fn shade(albedo: vec3<f32>, ao: f32, block: f32, sky: f32) -> vec3<f32> {
    return albedo * (ao * lightmap(block, sky));
}
