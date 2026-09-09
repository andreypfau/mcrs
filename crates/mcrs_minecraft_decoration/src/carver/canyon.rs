use crate::carver::mask::CarvingMask;
use crate::carver::tunnel::can_reach;
use crate::carver::water::WaterMask;
use crate::carver::{CarveShape, carve_ellipsoid};
use crate::math::{cos as math_helper_cos, sin as math_helper_sin};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen::carver::{CanyonShape, CarverConfig};
use mcrs_minecraft_worldgen::value_provider::HeightContext;

use crate::carver::modern::RANGE;

/// `CanyonWorldCarver.carve`.
///
/// A second trajectory kind rather than another set of knobs on the cave
/// integrator: it damps its rotations differently, redraws its radii every
/// step, never splits, and its cross-section is modulated by a per-height
/// width table rather than being a plain ellipsoid.
#[allow(clippy::too_many_arguments)]
pub fn carve_canyon<R: Random>(
    config: &CarverConfig,
    context: HeightContext,
    chunk_x: i32,
    chunk_z: i32,
    source_x: i32,
    source_z: i32,
    water: &WaterMask,
    mask: &mut CarvingMask,
    rng: &mut R,
) {
    let CarverConfig::Canyon {
        y: y_provider,
        vertical_rotation,
        ref shape,
        ..
    } = *config
    else {
        return;
    };

    let max_distance = (RANGE * 2 - 1) * 16;
    let x = (source_x * 16 + rng.next_i32_bound(16)) as f64;
    let y = y_provider.sample(rng, context) as f64;
    let z = (source_z * 16 + rng.next_i32_bound(16)) as f64;
    let yaw = rng.next_f32() * std::f32::consts::TAU;
    let pitch = vertical_rotation.sample(rng);
    let y_scale = shape.y_scale.sample(rng) as f64;
    let thickness = shape.thickness.sample(rng);
    let distance = (max_distance as f32 * shape.distance_factor.sample(rng)) as i32;
    let mut tunnel_rng = LegacyRandom::new(rng.next_java_long() as u64);

    walk_canyon(
        context,
        chunk_x,
        chunk_z,
        x,
        y,
        z,
        shape,
        thickness,
        yaw,
        pitch,
        distance,
        y_scale,
        water,
        mask,
        &mut tunnel_rng,
    );
}

#[allow(clippy::too_many_arguments)]
fn walk_canyon(
    context: HeightContext,
    chunk_x: i32,
    chunk_z: i32,
    mut x: f64,
    mut y: f64,
    mut z: f64,
    shape: &CanyonShape,
    thickness: f32,
    mut yaw: f32,
    mut pitch: f32,
    distance: i32,
    y_scale: f64,
    water: &WaterMask,
    mask: &mut CarvingMask,
    rng: &mut LegacyRandom,
) {
    let width_factors = init_width_factors(context, shape, rng);
    let shape_kind = CarveShape::Canyon {
        width_factors: &width_factors,
        min_y: context.min_y,
    };

    let mut yaw_velocity = 0.0f32;
    let mut pitch_velocity = 0.0f32;

    for step in 0..distance {
        let mut horizontal_radius = 1.5
            + (math_helper_sin(step as f32 * std::f32::consts::PI / distance as f32) * thickness)
                as f64;
        let mut vertical_radius = horizontal_radius * y_scale;
        horizontal_radius *= shape.horizontal_radius_factor.sample(rng) as f64;
        vertical_radius = update_vertical_radius(rng, shape, vertical_radius, distance, step);

        let cos_pitch = math_helper_cos(pitch);
        x += (math_helper_cos(yaw) * cos_pitch) as f64;
        y += math_helper_sin(pitch) as f64;
        z += (math_helper_sin(yaw) * cos_pitch) as f64;

        pitch *= 0.7;
        pitch += pitch_velocity * 0.05;
        yaw += yaw_velocity * 0.05;
        pitch_velocity *= 0.8;
        yaw_velocity *= 0.5;
        pitch_velocity += (rng.next_f32() - rng.next_f32()) * rng.next_f32() * 2.0;
        yaw_velocity += (rng.next_f32() - rng.next_f32()) * rng.next_f32() * 4.0;

        if rng.next_i32_bound(4) != 0 {
            if !can_reach(chunk_x, chunk_z, x, z, step, distance, thickness) {
                return;
            }
            carve_ellipsoid(
                chunk_x,
                chunk_z,
                x,
                y,
                z,
                horizontal_radius,
                vertical_radius,
                shape_kind,
                water,
                mask,
            );
        }
    }
}

/// One width factor per Y of the dimension, resampled whenever the smoothness
/// draw fires, so the canyon's walls step in and out rather than being a tube.
fn init_width_factors(
    context: HeightContext,
    shape: &CanyonShape,
    rng: &mut LegacyRandom,
) -> Vec<f32> {
    let mut factors = vec![0.0f32; context.depth.max(0) as usize];
    let mut factor = 1.0f32;
    for (index, slot) in factors.iter_mut().enumerate() {
        if index == 0 || rng.next_i32_bound(shape.width_smoothness) == 0 {
            factor = 1.0 + rng.next_f32() * rng.next_f32();
        }
        *slot = factor * factor;
    }
    factors
}

fn update_vertical_radius(
    rng: &mut LegacyRandom,
    shape: &CanyonShape,
    vertical_radius: f64,
    distance: i32,
    step: i32,
) -> f64 {
    let along = 1.0f32 - (0.5f32 - step as f32 / distance as f32).abs() * 2.0;
    let factor = shape.vertical_radius_default_factor + shape.vertical_radius_center_factor * along;
    let jitter = rng.next_f32() * (1.0 - 0.75) + 0.75;
    factor as f64 * vertical_radius * jitter as f64
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_worldgen::value_provider::{FloatProvider, HeightProvider, VerticalAnchor};

    fn overworld() -> HeightContext {
        HeightContext {
            min_y: -64,
            depth: 384,
            sea_level: 63,
        }
    }

    fn shipped_shape() -> CanyonShape {
        CanyonShape {
            distance_factor: FloatProvider::Constant(0.875),
            thickness: FloatProvider::Constant(3.0),
            width_smoothness: 3,
            horizontal_radius_factor: FloatProvider::Constant(0.875),
            vertical_radius_default_factor: 1.0,
            vertical_radius_center_factor: 0.0,
            y_scale: FloatProvider::Constant(3.0),
        }
    }

    fn canyon_at(y: i32) -> CarverConfig {
        CarverConfig::Canyon {
            probability: 0.01,
            y: HeightProvider::Constant(VerticalAnchor::Absolute(y)),
            vertical_rotation: FloatProvider::Constant(0.0),
            shape: shipped_shape(),
        }
    }

    fn empty_mask() -> CarvingMask {
        CarvingMask::new(16, -63, 312)
    }

    fn runs(mask: &CarvingMask) -> Vec<(i32, i32, i32, i32)> {
        let mut out = Vec::new();
        mask.visit(|x, z, bottom, top| out.push((x, z, bottom, top)));
        out
    }

    /// The entry's draw order, written out the way `CanyonWorldCarver.carve`
    /// takes it: origin, rotations, then the shape's own samples, then the
    /// tunnel seed. One canyon per source, never a count.
    #[test]
    fn the_entry_draws_in_the_reference_order() {
        let config = canyon_at(40);
        let mut carved = empty_mask();
        let mut rng = LegacyRandom::new(31337);
        carve_canyon(
            &config,
            overworld(),
            0,
            0,
            0,
            0,
            &WaterMask::default(),
            &mut carved,
            &mut rng,
        );

        let shape = shipped_shape();
        let mut replay = LegacyRandom::new(31337);
        let mut replayed = empty_mask();
        let x = replay.next_i32_bound(16) as f64;
        let y = 40.0;
        let z = replay.next_i32_bound(16) as f64;
        let yaw = replay.next_f32() * std::f32::consts::TAU;
        let pitch = 0.0;
        let y_scale = 3.0;
        let thickness = 3.0;
        let distance = (112.0f32 * 0.875) as i32;
        let mut tunnel_rng = LegacyRandom::new(replay.next_java_long() as u64);
        walk_canyon(
            overworld(),
            0,
            0,
            x,
            y,
            z,
            &shape,
            thickness,
            yaw,
            pitch,
            distance,
            y_scale,
            &WaterMask::default(),
            &mut replayed,
            &mut tunnel_rng,
        );

        assert_eq!(
            rng.next_java_long(),
            replay.next_java_long(),
            "the carver and the replay consumed different draws"
        );
        assert_eq!(runs(&carved), runs(&replayed));
    }

    #[test]
    fn a_canyon_marks_cells_inside_the_target_chunk() {
        let mut mask = empty_mask();
        let mut rng = LegacyRandom::new(4242);
        let config = canyon_at(40);
        for source_x in -2..=2 {
            for source_z in -2..=2 {
                carve_canyon(
                    &config,
                    overworld(),
                    0,
                    0,
                    source_x,
                    source_z,
                    &WaterMask::default(),
                    &mut mask,
                    &mut rng,
                );
            }
        }
        assert!(!mask.is_empty(), "no source reached the chunk");
        mask.visit(|x, z, bottom, top| {
            assert!((0..16).contains(&x), "x {x} outside the chunk");
            assert!((0..16).contains(&z), "z {z} outside the chunk");
            assert!(
                bottom >= -63 && top <= 312,
                "Y {bottom}..={top} out of range"
            );
        });
    }

    #[test]
    fn the_width_table_covers_the_dimension_and_holds_squares() {
        let mut rng = LegacyRandom::new(11);
        let factors = init_width_factors(overworld(), &shipped_shape(), &mut rng);
        assert_eq!(factors.len(), 384);
        for factor in &factors {
            // Every entry is `w * w` for some `w` in 1..2.
            assert!(
                (1.0..=4.0).contains(factor),
                "width factor {factor} out of range"
            );
        }
        // Smoothness 3 means roughly a third of the heights redraw, so the
        // table must neither be constant nor change at every step.
        let changes = factors.windows(2).filter(|w| w[0] != w[1]).count();
        assert!(changes > 40, "only {changes} steps of 383");
        assert!(changes < 250, "{changes} steps of 383 is not smoothing");
    }

    /// The centre factor is what makes a canyon bulge in the middle. With the
    /// shipped zero it is flat, so assert against a config that uses it.
    #[test]
    fn the_centre_factor_widens_the_middle_of_the_run() {
        let shape = CanyonShape {
            vertical_radius_center_factor: 1.0,
            ..shipped_shape()
        };
        let mut rng = LegacyRandom::new(5);
        let middle = update_vertical_radius(&mut rng, &shape, 1.0, 100, 50);
        let mut rng = LegacyRandom::new(5);
        let end = update_vertical_radius(&mut rng, &shape, 1.0, 100, 0);
        assert!(middle > end, "middle {middle} should exceed the end {end}");
        // default 1.0 + centre 1.0 * 1.0 at the midpoint, times the 0.75..1.0 jitter.
        assert!((0.75 * 2.0..=2.0).contains(&middle), "{middle}");
        assert!((0.75..=1.0).contains(&end), "{end}");
    }

    /// The canyon's cross-section is not the cave's ellipsoid: the `yd * yd / 6`
    /// term stretches it vertically, so it reaches Y the cave shape would skip.
    #[test]
    fn the_cross_section_stretches_further_than_an_ellipsoid() {
        let mut canyon = empty_mask();
        let mut rng = LegacyRandom::new(8);
        walk_canyon(
            overworld(),
            0,
            0,
            8.0,
            40.0,
            8.0,
            &shipped_shape(),
            3.0,
            0.0,
            0.0,
            1,
            1.0,
            &WaterMask::default(),
            &mut canyon,
            &mut rng,
        );
        let mut cave = empty_mask();
        crate::carver::carve_ellipsoid(
            0,
            0,
            8.0,
            40.0,
            8.0,
            1.5,
            1.5,
            CarveShape::Cave { floor_level: -0.7 },
            &WaterMask::default(),
            &mut cave,
        );
        let span = |mask: &CarvingMask| {
            let mut lowest = i32::MAX;
            let mut highest = i32::MIN;
            mask.visit(|_, _, bottom, top| {
                lowest = lowest.min(bottom);
                highest = highest.max(top);
            });
            highest - lowest
        };
        assert!(!canyon.is_empty(), "the canyon marked nothing");
        assert!(
            span(&canyon) > span(&cave),
            "canyon span {} should exceed the cave's {}",
            span(&canyon),
            span(&cave)
        );
    }
}
