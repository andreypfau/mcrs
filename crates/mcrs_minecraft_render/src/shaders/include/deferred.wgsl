#define_import_path mcrs_minecraft_client::deferred

#import mcrs_minecraft_client::lighting::lightmap

fn shade(albedo: vec3<f32>, ao: f32, block: f32, sky: f32) -> vec3<f32> {
    return albedo * (ao * lightmap(block, sky));
}
