use crate::carver::carve_ellipsoid;
use crate::carver::mask::CarvingMask;
use crate::carver::water::WaterMask;
use crate::math::{cos as math_helper_cos, sin as math_helper_sin};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;

/// How thick a tunnel is and how its radii are scaled. Beta leaves every
/// multiplier at one; the modern carvers sample each from a provider.
#[derive(Clone, Copy)]
pub struct TunnelShape {
    pub thickness: f32,
    pub y_scale: f64,
    pub horizontal_radius_multiplier: f64,
    pub vertical_radius_multiplier: f64,
    pub floor_level: f64,
}

/// Which generator seeds a split tunnel. The two draw from different streams —
/// Beta from the source's own generator, the modern carvers from the tunnel's —
/// and that is the one place their trajectories genuinely part.
#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SplitSeeding {
    FromTunnel,
    FromParent,
}

/// `WorldCarver.canReach`: give up once the target chunk is further away than
/// the steps left could possibly carry the tunnel.
fn can_reach(
    chunk_x: i32,
    chunk_z: i32,
    x: f64,
    z: f64,
    step: i32,
    total_steps: i32,
    thickness: f32,
) -> bool {
    let dx = x - (chunk_x as f64 * 16.0 + 8.0);
    let dz = z - (chunk_z as f64 * 16.0 + 8.0);
    let remaining = (total_steps - step) as f64;
    let reach = thickness as f64 + 2.0 + 16.0;
    dx * dx + dz * dz - remaining * remaining <= reach * reach
}

/// Walk one tunnel, marking an ellipsoid at each step it does not skip.
///
/// `room` is Beta's single-step start: it takes the step draw out of the loop
/// and stops after the first ellipsoid that actually rasterises, which is why
/// the water abort has to be answered here rather than in the substance pass.
#[allow(clippy::too_many_arguments)]
pub fn walk_tunnel<R: Random>(
    chunk_x: i32,
    chunk_z: i32,
    mut x: f64,
    mut y: f64,
    mut z: f64,
    shape: TunnelShape,
    mut yaw: f32,
    mut pitch: f32,
    step: i32,
    total_steps: i32,
    room: bool,
    split_seeding: SplitSeeding,
    water: &WaterMask,
    mask: &mut CarvingMask,
    rng: &mut LegacyRandom,
    parent_rng: &mut R,
) {
    let split_at = rng.next_i32_bound(total_steps / 2) + total_steps / 4;
    let steep = rng.next_i32_bound(6) == 0;
    let mut yaw_velocity = 0.0f32;
    let mut pitch_velocity = 0.0f32;

    let mut step = step;
    while step < total_steps {
        let horizontal_radius = 1.5
            + (math_helper_sin(step as f32 * std::f32::consts::PI / total_steps as f32)
                * shape.thickness) as f64;
        let vertical_radius = horizontal_radius * shape.y_scale;

        let cos_pitch = math_helper_cos(pitch);
        x += (math_helper_cos(yaw) * cos_pitch) as f64;
        y += math_helper_sin(pitch) as f64;
        z += (math_helper_sin(yaw) * cos_pitch) as f64;

        pitch *= if steep { 0.92 } else { 0.7 };
        pitch += pitch_velocity * 0.1;
        yaw += yaw_velocity * 0.1;
        pitch_velocity *= 0.9;
        yaw_velocity *= 0.75;
        pitch_velocity += (rng.next_f32() - rng.next_f32()) * rng.next_f32() * 2.0;
        yaw_velocity += (rng.next_f32() - rng.next_f32()) * rng.next_f32() * 4.0;

        if !room && step == split_at && shape.thickness > 1.0 {
            for side in [-std::f32::consts::FRAC_PI_2, std::f32::consts::FRAC_PI_2] {
                let seed = match split_seeding {
                    SplitSeeding::FromTunnel => rng.next_java_long(),
                    SplitSeeding::FromParent => parent_rng.next_java_long(),
                };
                let thickness = rng.next_f32() * 0.5 + 0.5;
                let mut split_rng = LegacyRandom::new(seed as u64);
                walk_tunnel(
                    chunk_x,
                    chunk_z,
                    x,
                    y,
                    z,
                    TunnelShape {
                        thickness,
                        y_scale: 1.0,
                        ..shape
                    },
                    yaw + side,
                    pitch / 3.0,
                    step,
                    total_steps,
                    false,
                    split_seeding,
                    water,
                    mask,
                    &mut split_rng,
                    parent_rng,
                );
            }
            return;
        }

        if room || rng.next_i32_bound(4) != 0 {
            if !can_reach(chunk_x, chunk_z, x, z, step, total_steps, shape.thickness) {
                return;
            }
            let carved = carve_ellipsoid(
                chunk_x,
                chunk_z,
                x,
                y,
                z,
                horizontal_radius * shape.horizontal_radius_multiplier,
                vertical_radius * shape.vertical_radius_multiplier,
                shape.floor_level,
                water,
                mask,
            );
            if room && carved {
                break;
            }
        }

        step += 1;
    }
}
