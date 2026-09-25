#define_import_path mcrs_minecraft_client::finish

/// Vanilla shades the bytes its textures are written with and hands the result to the screen
/// unchanged. The view target encodes to sRGB as it is written, so the colour is decoded first
/// and the target writes vanilla's bytes back out.
fn to_target(raw: vec3<f32>) -> vec3<f32> {
    let low = raw / 12.92;
    let high = pow((raw + 0.055) / 1.055, vec3<f32>(2.4));
    return select(high, low, raw <= vec3<f32>(0.04045));
}

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
#ifdef WIREFRAME
    return edge_pixels(quad_uv) > 1.0;
#else
    return false;
#endif
}

fn finish_solid(color: vec4<f32>, quad_uv: vec2<f32>) -> vec4<f32> {
    if (wireframe_discards(quad_uv)) {
        discard;
    }
    return vec4<f32>(to_target(color.rgb), 1.0);
}

fn finish_cutout(color: vec4<f32>, quad_uv: vec2<f32>) -> vec4<f32> {
    // Taken before the test: behind a short-circuit the derivative inside would sit in
    // non-uniform control flow, which WebGPU rejects for the whole module.
    let wireframe_discards = wireframe_discards(quad_uv);
    if (color.a < 0.5 || wireframe_discards) {
        discard;
    }
    return vec4<f32>(to_target(color.rgb), 1.0);
}

fn finish_translucent(color: vec4<f32>, quad_uv: vec2<f32>) -> vec4<f32> {
    if (wireframe_discards(quad_uv)) {
        discard;
    }
    return vec4<f32>(to_target(color.rgb), color.a);
}
