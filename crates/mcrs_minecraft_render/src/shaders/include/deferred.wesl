import package::shaders::include::gbuffer_encode::{GBuffer, octahedral_encode, sign_not_zero};
import package::shaders::include::lighting::lightmap;

fn octahedral_decode(e: vec2<f32>) -> vec3<f32> {
    let z = 1.0 - abs(e.x) - abs(e.y);
    let xy = select(e, (1.0 - abs(e.yx)) * sign_not_zero(e), z < 0.0);
    return normalize(vec3<f32>(xy, z));
}

/// The alpha of the light target on a terrain pixel. Every other opaque draw writes 0 there.
const TERRAIN_MARK: f32 = 1.0;

/// Block and sky light are in sixteenths of a level, as `lightmap` takes them.
fn gbuffer(albedo: vec3<f32>, ao: f32, normal: vec3<f32>, block: f32, sky: f32) -> GBuffer {
    var out: GBuffer;
    out.albedo_ao = vec4<f32>(albedo, ao);
    out.normal_motion = vec4<f32>(octahedral_encode(normalize(normal)), 0.0, 0.0);
    out.light = vec4<f32>(0.0, block, sky, TERRAIN_MARK);
    return out;
}

fn shade(albedo: vec3<f32>, ao: f32, block: f32, sky: f32) -> vec3<f32> {
    return albedo * (ao * lightmap(block, sky));
}
