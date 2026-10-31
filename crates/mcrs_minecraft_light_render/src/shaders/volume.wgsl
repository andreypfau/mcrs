#define_import_path mcrs_minecraft_client::volume

#import mcrs_minecraft_client::frame::camera
#import mcrs_minecraft_client::lighting::lightmap

struct VolumeTerms {
    ambient: vec4<f32>,
    sky_light: vec4<f32>,
    block_light: vec4<f32>,
    brightness: f32,
    radius: i32,
    min_section_y: i32,
}

@group(3) @binding(0) var page_table: texture_3d<u32>;
@group(3) @binding(1) var atlas: texture_3d<f32>;
@group(3) @binding(2) var atlas_sampler: sampler;
@group(3) @binding(3) var<uniform> terms: VolumeTerms;

const BRICK_SIDE: u32 = 18u;
const SECTION_SIDE: f32 = 16.0;
const NEUTRAL: vec4<f32> = vec4<f32>(0.0, 0.0, 0.0, 1.0);

/// RGB is the coloured types' share of the light, A the default type's. Outside the coloured
/// radius, outside the dimension and where colour is not ready it is `NEUTRAL`.
fn volume_tint(relative: vec3<f32>, normal: vec3<f32>) -> vec4<f32> {
    let p = relative + 0.5 * normal;
    let step = vec3<i32>(floor(p / SECTION_SIDE));
    let section = camera.section + step;
    let local = p - vec3<f32>(step) * SECTION_SIDE;
    let pages = vec3<i32>(textureDimensions(page_table));
    let row = section.y - terms.min_section_y;
    let page = vec3<i32>(
        section.x & (pages.x - 1),
        clamp(row, 0, pages.y - 1),
        section.z & (pages.z - 1),
    );
    let entry = textureLoad(page_table, page, 0).r;
    let slot = entry & 0xffffu;
    let tier = (entry >> 16u) & 0xfu;
    let size = textureDimensions(atlas);
    let per_axis = size / BRICK_SIDE;
    let origin = BRICK_SIDE * vec3<u32>(
        slot % per_axis.x,
        slot / per_axis.x % per_axis.y,
        slot / (per_axis.x * per_axis.y),
    );
    let texel = (vec3<f32>(origin) + 1.0 + local) / vec3<f32>(size);
    let sampled = textureSampleLevel(atlas, atlas_sampler, texel, 0.0);
    let reach = max(abs(step.x), abs(step.z));
    let neutral = terms.radius == 0 || reach > terms.radius || row < 0 || row >= pages.y
        || tier == 0u;
    return select(sampled, NEUTRAL, neutral);
}

fn light_curve(level: f32) -> f32 {
    let f = level / 15.0;
    return f / (4.0 - 3.0 * f);
}

fn finish(color: vec3<f32>) -> vec3<f32> {
    let clamped = clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
    let top = max(clamped.x, max(clamped.y, clamped.z));
    let rest = 1.0 - top;
    let lifted = clamped * ((1.0 - rest * rest * rest * rest) / top);
    return mix(clamped, select(clamped, lifted, top > 0.0), terms.brightness);
}

/// Levels are in sixteenths. Before its clamp vanilla's lightmap texel is affine in the block
/// tint, so the tint moves it by `(t - d) * k`; at the default tint that is exactly zero.
fn tinted_lightmap(block: f32, sky: f32, tint: vec4<f32>) -> vec3<f32> {
    let b = block / 16.0;
    let s = sky / 16.0;
    let d = terms.block_light.rgb;
    let t = tint.rgb + tint.a * d;
    let f = b / 15.0;
    let parabolic = (2.0 * f - 1.0) * (2.0 * f - 1.0);
    let block_brightness = light_curve(b) * terms.block_light.w;
    let sky_brightness = light_curve(s) * terms.sky_light.w;
    let base = max(terms.ambient.rgb, vec3<f32>(0.0)) + terms.sky_light.rgb * sky_brightness
        + mix(d, vec3<f32>(1.0), 0.9 * parabolic) * block_brightness;
    let k = (1.0 - 0.9 * parabolic) * block_brightness;
    let delta = finish(base + (t - d) * k) - finish(base);
    return clamp(lightmap(block, sky) + delta, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn shade_tinted(albedo: vec3<f32>, ao: f32, block: f32, sky: f32, tint: vec4<f32>) -> vec3<f32> {
    return albedo * (ao * tinted_lightmap(block, sky, tint));
}
