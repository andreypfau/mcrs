const EDGE_TOLERANCE_PIXELS: f32 = 1e-4;

fn quad_projection(anchor: vec4<f32>, u: vec4<f32>, v: vec4<f32>) -> mat3x3<f32> {
    return mat3x3<f32>(anchor.xyw, u.xyw, v.xyw);
}

fn quad_uv_at(projection: mat3x3<f32>, pixel: vec2<f32>, viewport: vec2<f32>, origin: vec2<f32>) -> vec2<f32> {
    let ndc = (pixel - origin) / viewport * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0);
    let a = projection[0].xy - projection[0].z * ndc;
    let u = projection[1].xy - projection[1].z * ndc;
    let v = projection[2].xy - projection[2].z * ndc;
    let denominator = u.x * v.y - u.y * v.x;
    return vec2<f32>(v.x * a.y - v.y * a.x, a.x * u.y - a.y * u.x) / denominator;
}

fn quad_edge_contains(distance: f32, inward: vec2<f32>) -> bool {
    let tolerance = length(inward) * EDGE_TOLERANCE_PIXELS;
    let nearly_horizontal = abs(inward.x) <= abs(inward.y) * 1e-5;
    let owns_edge = select(inward.x > 0.0, inward.y > 0.0, nearly_horizontal);
    return distance > tolerance || (distance >= -tolerance && owns_edge);
}

fn quad_contains(projection: mat3x3<f32>, uv: vec2<f32>, pixel: vec2<f32>, viewport: vec2<f32>) -> bool {
    let q = vec3<f32>(pixel / viewport * vec2<f32>(2.0, -2.0) + vec2<f32>(-1.0, 1.0), 1.0);
    let a = projection[0];
    let u = projection[1];
    let v = projection[2];
    let r0 = cross(v, a);
    let r1 = cross(a, u);
    let r2 = cross(u, v);
    let denominator = dot(r2, q);
    let pixel_to_ndc = vec2<f32>(2.0, -2.0) / viewport;
    let du = (r0.xy - uv.x * r2.xy) / denominator * pixel_to_ndc;
    let dv = (r1.xy - uv.y * r2.xy) / denominator * pixel_to_ndc;
    return quad_edge_contains(uv.x, du) && quad_edge_contains(1.0 - uv.x, -du)
        && quad_edge_contains(uv.y, dv) && quad_edge_contains(1.0 - uv.y, -dv);
}
