/// 0 at `ramp.x` or nearer, else 1 at `ramp.y` or farther, else linear: a step at `ramp.x` when
/// `ramp.x >= ramp.y`.
fn fog_ramp(distance: f32, ramp: vec2<f32>) -> f32 {
    if (distance <= ramp.x) {
        return 0.0;
    }
    if (distance >= ramp.y) {
        return 1.0;
    }
    return (distance - ramp.x) / (ramp.y - ramp.x);
}

/// The two fog values of the point at `offset` from the camera: the spherical distance on the
/// environmental ramp, then the cylindrical distance on the render-distance ramp.
fn fog_values(offset: vec3<f32>, environmental: vec2<f32>, render_distance: vec2<f32>) -> vec2<f32> {
    let spherical = length(offset);
    let cylindrical = max(length(offset.xz), abs(offset.y));
    return vec2<f32>(fog_ramp(spherical, environmental), fog_ramp(cylindrical, render_distance));
}

fn fog_value(offset: vec3<f32>, environmental: vec2<f32>, render_distance: vec2<f32>) -> f32 {
    let values = fog_values(offset, environmental, render_distance);
    return max(values.x, values.y);
}

/// In image values, before the colour is encoded for the target.
fn fog_mix(rgb: vec3<f32>, value: f32, color: vec3<f32>) -> vec3<f32> {
    return mix(rgb, color, value);
}

/// The fog of a face with Improved Transparency, in image values: `values` from `fog_values`,
/// `sky` the sky behind the face, `cloud` the colour and the alpha of the cloud image, and
/// `cloud_behind` whether the cloud is as far as the face or farther.
fn improved_fog_mix(
    rgb: vec3<f32>,
    values: vec2<f32>,
    color: vec3<f32>,
    sky: vec3<f32>,
    cloud: vec4<f32>,
    cloud_behind: bool,
) -> vec3<f32> {
    let v = max(values.x, values.y);
    let b = max(smoothstep(0.9, 1.0, values.x), values.y);
    let q = select(cloud.rgb, cloud.rgb / cloud.a, cloud.a > 0.0);
    let background = mix(sky, q, select(0.0, cloud.a, cloud_behind));
    return mix(rgb, mix(color, background, b), v);
}

/// The cloud image over `rgb`, in image values: `cloud` holds `cl` and `ca`, and the reverse-Z
/// depths hide the cloud behind a nearer face. A pixel with no opaque face has the depth 0.
fn cloud_over(rgb: vec3<f32>, cloud: vec4<f32>, cloud_depth: f32, depth: f32) -> vec3<f32> {
    return select(rgb, (1.0 - cloud.a) * rgb + cloud.rgb, cloud_depth >= depth);
}
