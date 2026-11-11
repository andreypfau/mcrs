import package::shaders::include::terrain_bindings::{lightmap_levels, lightmap_sampler};

/// Vanilla samples its lightmap bilinearly at sixteenths of a level, clamped to the texel centres.
fn lightmap(block: f32, sky: f32) -> vec3<f32> {
    let uv = clamp(
        vec2<f32>(block, sky) / 256.0 + 0.5 / 16.0,
        vec2<f32>(0.5 / 16.0),
        vec2<f32>(15.5 / 16.0),
    );
    return textureSampleLevel(lightmap_levels, lightmap_sampler, uv, 0.0).rgb;
}

/// The directional shade of a face direction in the table of the dimension: `0` is the
/// `default` table and `1` the `nether` one.
fn face_shade(face: u32, table: u32) -> f32 {
    switch face {
        case 0u: { return select(0.5, 0.9, table == 1u); }
        case 1u: { return select(1.0, 0.9, table == 1u); }
        case 2u, 3u: { return 0.8; }
        default: { return 0.6; }
    }
}
