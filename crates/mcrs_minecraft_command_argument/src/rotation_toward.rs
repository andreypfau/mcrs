use glam::DVec3;
use mcrs_minecraft_core::Look;
use mcrs_minecraft_core::mth::{atan2, wrap_degrees};

/// The yaw and the pitch that look from `from` to `to`.
#[must_use]
pub fn rotation_toward(from: DVec3, to: DVec3) -> Look {
    let delta = to - from;
    let horizontal = (delta.x * delta.x + delta.z * delta.z).sqrt();
    let pi = f64::from(core::f32::consts::PI);
    Look {
        yaw: wrap_degrees((atan2(delta.z, delta.x) * 180.0 / pi) as f32 - 90.0),
        pitch: wrap_degrees(-(atan2(delta.y, horizontal) * 180.0 / pi) as f32),
    }
}
