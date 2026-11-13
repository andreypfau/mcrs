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

/// The fog value of the point at `offset` from the camera: the spherical distance on the
/// environmental ramp, the cylindrical distance on the render-distance ramp.
fn fog_value(offset: vec3<f32>, environmental: vec2<f32>, render_distance: vec2<f32>) -> f32 {
    let spherical = length(offset);
    let cylindrical = max(length(offset.xz), abs(offset.y));
    return max(fog_ramp(spherical, environmental), fog_ramp(cylindrical, render_distance));
}

/// In image values, before the colour is encoded for the target.
fn fog_mix(rgb: vec3<f32>, value: f32, color: vec3<f32>) -> vec3<f32> {
    return mix(rgb, color, value);
}
