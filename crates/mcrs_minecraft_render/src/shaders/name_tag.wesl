import mcrs_minecraft_render::shaders::include::fields::SECTION_SIZE;
import mcrs_minecraft_render::shaders::include::finish::from_target;
import mcrs_minecraft_render::shaders::include::lightmap::lightmap_sample;
import mcrs_minecraft_render::shaders::include::world_text_layout::{
    font_page, font_sampler, frame, lightmap_levels, lightmap_sampler,
};

struct Vertex {
    @location(0) block: vec3<i32>,
    @location(1) offset: vec3<f32>,
    @location(2) uv: vec2<f32>,
    @location(3) color: vec4<f32>,
    @location(4) light: vec2<f32>,
};

struct Fragment {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
    @location(2) light: vec2<f32>,
};

@vertex
fn vertex(in: Vertex) -> Fragment {
    var out: Fragment;
    let relative = vec3<f32>(in.block - frame.section * i32(SECTION_SIZE)) + in.offset;
    out.clip_position = frame.clip_from_relative * vec4<f32>(relative, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    out.light = in.light;
    return out;
}

@fragment
fn fragment(in: Fragment) -> @location(0) vec4<f32> {
    let sampled = textureSample(font_page, font_sampler, in.uv);
    let texel = select(sampled, vec4<f32>(1.0), in.uv.x < 0.0);
    if texel.a < 0.5 {
        discard;
    }
    let light = lightmap_sample(lightmap_levels, lightmap_sampler, in.light.x, in.light.y);
    return vec4<f32>(from_target(in.color.rgb * texel.rgb * light), in.color.a);
}
