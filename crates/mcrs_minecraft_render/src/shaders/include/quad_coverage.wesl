const QUAD_BOUNDS_EPSILON: f32 = 1e-6;

// Coverage follows the original plane and pixel centre, independent of snapped raster vertices.
fn quad_projection(anchor: vec4<f32>, u: vec4<f32>, v: vec4<f32>) -> mat3x3<f32> {
    return mat3x3<f32>(anchor.xyw, u.xyw, v.xyw);
}

fn quad_uv_at(projection: mat3x3<f32>, pixel: vec2<f32>, viewport: vec2<f32>) -> vec2<f32> {
    let ndc = pixel / viewport * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
    let a = projection[0].xy - projection[0].z * ndc;
    let u = projection[1].xy - projection[1].z * ndc;
    let v = projection[2].xy - projection[2].z * ndc;
    let denominator = u.x * v.y - u.y * v.x;
    return vec2<f32>(v.x * a.y - v.y * a.x, a.x * u.y - a.y * u.x) / denominator;
}

fn quad_contains(uv: vec2<f32>) -> bool {
    return all(uv >= vec2<f32>(-QUAD_BOUNDS_EPSILON))
        && all(uv <= vec2<f32>(1.0 + QUAD_BOUNDS_EPSILON));
}
