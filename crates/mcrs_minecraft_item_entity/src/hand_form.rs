use core::f32::consts::TAU;

use glam::DVec3;
use mcrs_minecraft_entity_key::KeyedStream;
use mcrs_minecraft_numeric::cos_nearest;
use mcrs_minecraft_numeric::radians_from_degrees;
use mcrs_minecraft_numeric::sin_nearest;

use crate::Start;

/// The velocity of the hand form (IEN-SPN-002).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum HandVelocity {
    /// Along the look direction, in degrees.
    Look { yaw: f32, pitch: f32 },
    /// In a random horizontal direction, as at the death of a player.
    Scatter,
}

/// The hand form of an entity at `feet` with the eye height `eye_height`, with the draws of
/// `stream` (IEN-SPN-002).
#[must_use]
pub fn hand_form(
    feet: DVec3,
    eye_height: f32,
    velocity: HandVelocity,
    stream: &KeyedStream,
) -> Start {
    let draw = |index: u64| stream.unit(index) as f32;
    let angle = draw(0) * TAU;
    let velocity = match velocity {
        HandVelocity::Look { yaw, pitch } => {
            let (yaw, pitch) = (radians_from_degrees(yaw), radians_from_degrees(pitch));
            let spread = f64::from(draw(1) * 0.02);
            let lift = 0.1 * (draw(2) - draw(3));
            let angle = f64::from(angle);
            DVec3::new(
                f64::from(-sin_nearest(yaw) * cos_nearest(pitch) * 0.3) + angle.cos() * spread,
                f64::from(-sin_nearest(pitch) * 0.3 + 0.1 + lift),
                f64::from(cos_nearest(yaw) * cos_nearest(pitch) * 0.3) + angle.sin() * spread,
            )
        }
        HandVelocity::Scatter => {
            let reach = draw(1) * 0.5;
            let angle = f64::from(angle);
            DVec3::new(
                f64::from(-sin_nearest(angle) * reach),
                f64::from(0.2_f32),
                f64::from(cos_nearest(angle) * reach),
            )
        }
    };
    Start {
        position: DVec3::new(
            feet.x,
            feet.y + f64::from(eye_height) - f64::from(0.3_f32),
            feet.z,
        ),
        velocity,
    }
}

/// The default form at `point`, with the draws of `stream` (IEN-SPN-008).
#[must_use]
pub fn default_form(point: DVec3, stream: &KeyedStream) -> Start {
    let horizontal = |index: u64| stream.unit(index) * 0.2 - 0.1;
    Start {
        position: point,
        velocity: DVec3::new(horizontal(0), 0.2, horizontal(1)),
    }
}
