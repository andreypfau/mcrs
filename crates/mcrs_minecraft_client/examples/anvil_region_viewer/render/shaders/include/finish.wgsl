#define_import_path anvil_region_viewer::finish

#import anvil_region_viewer::frame::params

fn edge_pixels(quad_uv: vec2<f32>) -> f32 {
    let width = max(fwidth(quad_uv), vec2<f32>(1e-6));
    let border = min(
        min(quad_uv.x, 1.0 - quad_uv.x) / width.x,
        min(quad_uv.y, 1.0 - quad_uv.y) / width.y,
    );
    let split = quad_uv.x - quad_uv.y;
    let diagonal = abs(split) / max(fwidth(split), 1e-6);
    return min(border, diagonal);
}

fn wireframe_discards(quad_uv: vec2<f32>) -> bool {
    return params.wireframe != 0u && edge_pixels(quad_uv) > 1.0;
}

fn finish_solid(color: vec4<f32>) -> vec4<f32> {
    return vec4<f32>(color.rgb, 1.0);
}

fn finish_cutout(color: vec4<f32>, quad_uv: vec2<f32>) -> vec4<f32> {
    if (color.a < 0.5 || wireframe_discards(quad_uv)) {
        discard;
    }
    return vec4<f32>(color.rgb, 1.0);
}

fn finish_translucent(color: vec4<f32>, quad_uv: vec2<f32>) -> vec4<f32> {
    if (wireframe_discards(quad_uv)) {
        discard;
    }
    return color;
}
