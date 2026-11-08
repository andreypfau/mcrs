use glam::DVec3;

use crate::PlayerRotation;
use crate::PlayerState;
use crate::SineTable;

/// Degrees to radians in f64, as the teleport computes them.
const RADIANS_PER_DEGREE: f64 = 0.017453292519943295;

/// A `player_position` packet: the changes and the flags that make each one relative.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Teleport {
    pub position: DVec3,
    pub velocity: DVec3,
    pub yaw: f32,
    pub pitch: f32,
    pub relative: Relative,
}

bitflags::bitflags! {
    /// The values of a teleport that add to the current value.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct Relative: u16 {
        const X = 1 << 0;
        const Y = 1 << 1;
        const Z = 1 << 2;
        const YAW = 1 << 3;
        const PITCH = 1 << 4;
        const VELOCITY_X = 1 << 5;
        const VELOCITY_Y = 1 << 6;
        const VELOCITY_Z = 1 << 7;
        /// Rotate the velocity by the change of the rotation before the velocity change.
        const ROTATE_VELOCITY = 1 << 8;
    }
}

/// Applies a teleport to the state and the rotation of a player. The flags, the supporting block
/// and the fall distance stay.
pub fn apply_teleport(
    sine: &SineTable,
    state: &mut PlayerState,
    rotation: &mut PlayerRotation,
    packet: &Teleport,
) {
    let relative = packet.relative;
    let pick = |flag: Relative, current: f64, change: f64| {
        if relative.contains(flag) {
            current + change
        } else {
            change
        }
    };
    let position = DVec3::new(
        pick(Relative::X, state.position.x, packet.position.x),
        pick(Relative::Y, state.position.y, packet.position.y),
        pick(Relative::Z, state.position.z, packet.position.z),
    );
    let yaw = if relative.contains(Relative::YAW) {
        rotation.yaw + packet.yaw
    } else {
        packet.yaw
    };
    let pitch = if relative.contains(Relative::PITCH) {
        rotation.pitch + packet.pitch
    } else {
        packet.pitch
    }
    .clamp(-90.0, 90.0);
    let mut velocity = state.velocity;
    if relative.contains(Relative::ROTATE_VELOCITY) {
        let about_x = radians(rotation.pitch - pitch);
        let (cos, sin) = (f64::from(sine.cos(about_x)), f64::from(sine.sin(about_x)));
        velocity = DVec3::new(
            velocity.x,
            velocity.y * cos + velocity.z * sin,
            velocity.z * cos - velocity.y * sin,
        );
        let about_y = radians(rotation.yaw - yaw);
        let (cos, sin) = (f64::from(sine.cos(about_y)), f64::from(sine.sin(about_y)));
        velocity = DVec3::new(
            velocity.x * cos + velocity.z * sin,
            velocity.y,
            velocity.z * cos - velocity.x * sin,
        );
    }
    state.velocity = DVec3::new(
        pick(Relative::VELOCITY_X, velocity.x, packet.velocity.x),
        pick(Relative::VELOCITY_Y, velocity.y, packet.velocity.y),
        pick(Relative::VELOCITY_Z, velocity.z, packet.velocity.z),
    );
    state.position = position;
    rotation.yaw = yaw;
    rotation.pitch = pitch;
}

/// `f32(α × 0.017453292519943295)` of an f32 angle in degrees, widened.
fn radians(degrees: f32) -> f64 {
    f64::from((f64::from(degrees) * RADIANS_PER_DEGREE) as f32)
}
