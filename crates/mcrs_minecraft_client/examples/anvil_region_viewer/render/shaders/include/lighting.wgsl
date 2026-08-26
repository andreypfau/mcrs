#define_import_path anvil_region_viewer::lighting

#import anvil_region_viewer::frame::sky

fn light_curve(level: f32) -> f32 {
    let f = level / 15.0;
    return f / (4.0 - 3.0 * f);
}

fn lightmap(block_level: f32, sky_level: f32) -> vec3<f32> {
    var color = sky.ambient.rgb;
    color += sky.sky_light.rgb * light_curve(sky_level) * sky.sky_light.a;
    let f = block_level / 15.0;
    let parabolic = (2.0 * f - 1.0) * (2.0 * f - 1.0);
    let tint = mix(sky.block_light.rgb, vec3<f32>(1.0), 0.9 * parabolic);
    color += tint * light_curve(block_level) * sky.block_light.a;
    return clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
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
