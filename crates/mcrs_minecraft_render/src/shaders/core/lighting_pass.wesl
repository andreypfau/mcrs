import bevy_core_pipeline::fullscreen_vertex_shader::FullscreenVertexOutput;
import mcrs_minecraft_render::shaders::include::deferred::shade;
import mcrs_minecraft_render::shaders::include::depth::depth_at;
import mcrs_minecraft_render::shaders::include::fog::{fog_mix, fog_value};
@if(IMPROVED) import mcrs_minecraft_render::shaders::include::fog::fog_values;
@if(IMPROVED) import mcrs_minecraft_render::shaders::include::transparency::{sky_with_clouds, transparent_fog, with_clouds};
import mcrs_minecraft_render::shaders::include::frame::camera;
import mcrs_minecraft_render::shaders::include::gbuffer_encode::octahedral_decode;
import mcrs_minecraft_render::shaders::include::gbuffer_layout::{
    albedo_ao, depth, light, nearest, normal_motion, reconstruction,
};
@if(LIGHT_TINT) import mcrs_minecraft_light_render::shaders::volume::{shade_tinted, volume_tint};

fn relative_position(pixel: vec2<i32>, depth: f32) -> vec3<f32> {
    let uv = (vec2<f32>(pixel) + 0.5) / reconstruction.viewport;
    let ndc = uv * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
    let p = reconstruction.relative_from_clip * vec4<f32>(ndc, depth, 1.0);
    return p.xyz / p.w;
}

@fragment
fn lighting(in: FullscreenVertexOutput) -> @location(0) vec4<f32> {
    let pixel = vec2<i32>(in.position.xy);
    let d = depth_at(depth, nearest, pixel);
    if (d == 0.0) {
        @if(IMPROVED)
        return vec4<f32>(sky_with_clouds(pixel), 1.0);
        @else
        discard;
    }
    let a = textureLoad(albedo_ao, pixel, 0);
    let l = textureLoad(light, pixel, 0);
    @if(LIGHTING_TERM) {
        let albedo = vec3<f32>(1.0);
    } @else {
        let albedo = a.rgb;
    }
    let relative = relative_position(pixel, d);
    @if(LIGHT_TINT) {
        let normal = octahedral_decode(textureLoad(normal_motion, pixel, 0).xy);
        let tint = volume_tint(relative, normal);
        let lit = shade_tinted(albedo, a.a, l.g, l.b, tint);
    } @else {
        let lit = shade(albedo, a.a, l.g, l.b);
    }
    @if(LIGHTING_TERM) {
        return vec4<f32>(lit, 1.0);
    } @else {
        @if(IMPROVED) {
            if (l.a > 0.5) {
                let values = fog_values(relative - camera.offset, camera.fog_environmental, camera.fog_render_distance);
                let terrain = transparent_fog(lit, pixel, d, values, camera.fog_color);
                return vec4<f32>(with_clouds(terrain, pixel, d), 1.0);
            }
        }
        let fog = fog_value(relative - camera.offset, camera.fog_environmental, camera.fog_render_distance);
        let m = fog_mix(lit, fog, camera.fog_color);
        @if(IMPROVED)
        return vec4<f32>(with_clouds(m, pixel, d), 1.0);
        @else
        return vec4<f32>(m, 1.0);
    }
}
