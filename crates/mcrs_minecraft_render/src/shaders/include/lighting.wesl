import mcrs_minecraft_render::shaders::include::lightmap::lightmap_sample;
import mcrs_minecraft_render::shaders::include::terrain_layout::{lightmap_levels, lightmap_sampler};

fn lightmap(block: f32, sky: f32) -> vec3<f32> {
    return lightmap_sample(lightmap_levels, lightmap_sampler, block, sky);
}

fn face_shade(face: u32, table: u32) -> f32 {
    switch face {
        case 0u: { return select(0.5, 0.9, table == 1u); }
        case 1u: { return select(1.0, 0.9, table == 1u); }
        case 2u, 3u: { return 0.8; }
        default: { return 0.6; }
    }
}
