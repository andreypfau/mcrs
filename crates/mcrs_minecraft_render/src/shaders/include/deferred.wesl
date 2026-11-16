import mcrs_minecraft_render::shaders::include::gbuffer_encode::{GBuffer, octahedral_encode};
import mcrs_minecraft_render::shaders::include::lighting::lightmap;

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
