#define_import_path mcrs_minecraft_client::lighting

#import mcrs_minecraft_client::terrain_bindings::lightmap_levels

fn lightmap(block_level: f32, sky_level: f32) -> vec3<f32> {
    return textureLoad(lightmap_levels, vec2<u32>(u32(block_level), u32(sky_level)), 0).rgb;
}

fn face_shade(face: u32) -> f32 {
    switch face {
        case 0u: { return 0.5; }
        case 1u: { return 1.0; }
        case 2u, 3u: { return 0.8; }
        default: { return 0.6; }
    }
}

fn ao_factor(bits: u32, corner: u32) -> f32 {
    return 0.4 + f32((bits >> (corner * 2u)) & 3u) * 0.2;
}
