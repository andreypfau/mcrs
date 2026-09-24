#define_import_path mcrs_minecraft_client::lighting

#import mcrs_minecraft_client::terrain_bindings::{lightmap_levels, lightmap_sampler}

/// Vanilla samples its lightmap bilinearly at sixteenths of a level, clamped to the texel centres.
fn lightmap(block: f32, sky: f32) -> vec3<f32> {
    let uv = clamp(
        vec2<f32>(block, sky) / 256.0 + 0.5 / 16.0,
        vec2<f32>(0.5 / 16.0),
        vec2<f32>(15.5 / 16.0),
    );
    return textureSampleLevel(lightmap_levels, lightmap_sampler, uv, 0.0).rgb;
}

fn face_shade(face: u32) -> f32 {
    switch face {
        case 0u: { return 0.5; }
        case 1u: { return 1.0; }
        case 2u, 3u: { return 0.8; }
        default: { return 0.6; }
    }
}
