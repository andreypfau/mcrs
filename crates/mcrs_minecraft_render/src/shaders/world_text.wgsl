#import bevy_render::view::View

@group(0) @binding(0) var<uniform> view: View;

@group(1) @binding(0) var font_page: texture_2d<f32>;
@group(1) @binding(1) var font_sampler: sampler;

struct Vertex {
    @location(0) position: vec3<f32>,
    @location(1) uv: vec2<f32>,
    @location(2) color: vec4<f32>,
};

struct Fragment {
    @builtin(position) clip_position: vec4<f32>,
    @location(0) uv: vec2<f32>,
    @location(1) color: vec4<f32>,
};

@vertex
fn vertex(in: Vertex) -> Fragment {
    var out: Fragment;
    out.clip_position = view.clip_from_world * vec4<f32>(in.position, 1.0);
    out.uv = in.uv;
    out.color = in.color;
    return out;
}

@fragment
fn fragment(in: Fragment) -> @location(0) vec4<f32> {
    let texel = textureSample(font_page, font_sampler, in.uv);
    if texel.a < 0.5 {
        discard;
    }
    return vec4<f32>(in.color.rgb * texel.rgb, 1.0);
}
