#import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput
#import mcrs_minecraft_client::deferred::octahedral_decode
#import mcrs_minecraft_client::fields::SECTION_SIZE
#import mcrs_minecraft_client::finish::to_target
#import mcrs_minecraft_client::frame::camera

struct Reconstruction {
    relative_from_clip: mat4x4<f32>,
    viewport: vec2<f32>,
}

@group(2) @binding(0) var albedo_ao: texture_2d<f32>;
@group(2) @binding(1) var normal_motion: texture_2d<f32>;
@group(2) @binding(2) var light: texture_2d<f32>;
@group(2) @binding(3) var depth: texture_depth_2d;
@group(2) @binding(4) var<uniform> reconstruction: Reconstruction;

// Block and sky light are stored in sixteenths of a level.
const FULL_LIGHT: f32 = 240.0;

/// Against the origin of the camera's section, as terrain is drawn.
fn relative_position(pixel: vec2<i32>, depth: f32) -> vec3<f32> {
    let uv = (vec2<f32>(pixel) + 0.5) / reconstruction.viewport;
    let ndc = uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
    let p = reconstruction.relative_from_clip * vec4<f32>(ndc, depth, 1.0);
    return p.xyz / p.w;
}

fn along_face(v: vec3<f32>, normal: vec3<f32>) -> vec2<f32> {
    let n = abs(normal);
    if (n.x >= n.y && n.x >= n.z) {
        return v.yz;
    }
    if (n.y >= n.z) {
        return v.xz;
    }
    return v.xy;
}

@fragment
fn show(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(in.position.xy);
    let d = textureLoad(depth, pixel, 0);
    if (d == 0.0) {
        discard;
    }
#ifdef SHOW_ALBEDO
    let colour = textureLoad(albedo_ao, pixel, 0).rgb;
#else ifdef SHOW_AO
    let colour = vec3<f32>(textureLoad(albedo_ao, pixel, 0).a);
#else ifdef SHOW_NORMAL
    let colour = octahedral_decode(textureLoad(normal_motion, pixel, 0).xy) * 0.5 + 0.5;
#else ifdef SHOW_BLOCK_LIGHT
    let colour = vec3<f32>(textureLoad(light, pixel, 0).g / FULL_LIGHT);
#else ifdef SHOW_SKY_LIGHT
    let colour = vec3<f32>(textureLoad(light, pixel, 0).b / FULL_LIGHT);
#else ifdef SHOW_GRID
    let rel = relative_position(pixel, d);
    let normal = octahedral_decode(textureLoad(normal_motion, pixel, 0).xy);
    // Half a block into the surface, so a pixel on a face never floors into the air before it.
    let cell = floor(rel - normal * 0.5);
    let voxel = camera.section * i32(SECTION_SIZE) + vec3<i32>(cell);
    let parity = f32((voxel.x + voxel.y + voxel.z) & 1);
    let ramp = along_face(fract(rel), normal);
    let colour = vec3<f32>(0.2 + 0.4 * parity + 0.4 * ramp.x * ramp.y);
#endif
    return vec4<f32>(to_target(colour), 1.0);
}
