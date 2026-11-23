use crate::CarveShape;
use crate::target::CarveTarget;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum TrigIndex {
    Beta,
    Modern,
}

impl TrigIndex {
    #[inline]
    fn sin(self, x: f32) -> f32 {
        match self {
            TrigIndex::Beta => mcrs_minecraft_numeric::sin_beta(x),
            TrigIndex::Modern => mcrs_minecraft_numeric::sin_truncated(f64::from(x)),
        }
    }

    #[inline]
    fn cos(self, x: f32) -> f32 {
        match self {
            TrigIndex::Beta => mcrs_minecraft_numeric::cos_beta(x),
            TrigIndex::Modern => mcrs_minecraft_numeric::cos_truncated(f64::from(x)),
        }
    }
}

#[derive(Clone, Copy)]
pub struct TunnelShape {
    pub thickness: f32,
    pub y_scale: f64,
    pub horizontal_radius_multiplier: f64,
    pub vertical_radius_multiplier: f64,
    pub trig: TrigIndex,
}

#[derive(Clone, Copy, PartialEq, Eq)]
pub enum SplitSeeding {
    FromTunnel,
    FromParent,
}

pub fn can_reach(
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

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::too_many_arguments,
        reason = "the position, the angles and the step change as the walk recurses into branches, and each branch passes its own values"
    )
)]
pub fn walk_tunnel_into<T: CarveTarget, R: Random>(
    target: &mut T,
    mut live: T::Live,
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
    shape_kind: CarveShape<'_>,
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
            + (shape
                .trig
                .sin(step as f32 * std::f32::consts::PI / total_steps as f32)
                * shape.thickness) as f64;
        let vertical_radius = horizontal_radius * shape.y_scale;

        let cos_pitch = shape.trig.cos(pitch);
        x += (shape.trig.cos(yaw) * cos_pitch) as f64;
        y += shape.trig.sin(pitch) as f64;
        z += (shape.trig.sin(yaw) * cos_pitch) as f64;

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
                walk_tunnel_into(
                    target,
                    live,
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
                    shape_kind,
                    &mut split_rng,
                    parent_rng,
                );
            }
            return;
        }

        if room || rng.next_i32_bound(4) != 0 {
            let Some(reached) = target.reach(live, x, z, step, total_steps, shape.thickness) else {
                return;
            };
            live = reached;
            let carved = target.carve(
                live,
                x,
                y,
                z,
                horizontal_radius * shape.horizontal_radius_multiplier,
                vertical_radius * shape.vertical_radius_multiplier,
                shape_kind,
            );
            if room && carved {
                break;
            }
        }

        step += 1;
    }
}
