use crate::carver::CarveShape;
use crate::carver::mask::CarvingMask;
use crate::carver::tunnel::{SplitSeeding, TunnelShape, walk_tunnel};
use crate::carver::water::WaterMask;
use crate::math::sin as math_helper_sin;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen::carver::CarverConfig;
use mcrs_minecraft_worldgen::value_provider::{FloatProvider, HeightContext};

/// `WorldCarver.getRange()`. It feeds the tunnel length and nothing else; the
/// source loop's own radius is [`SOURCE_RADIUS`].
pub const RANGE: i32 = 4;

/// Hardcoded in `generateCarvers`, and deliberately not `RANGE`.
pub const SOURCE_RADIUS: i32 = 8;

const fn tunnel_length() -> i32 {
    (RANGE * 2 - 1) * 16
}

/// Whether this carver starts anything in the source chunk. One draw, taken
/// before any of the per-cave draws.
pub fn is_start_chunk<R: Random>(config: &CarverConfig, rng: &mut R) -> bool {
    rng.next_f32() <= config.probability()
}

/// `CaveWorldCarver.carve`.
#[allow(clippy::too_many_arguments)]
pub fn carve_caves<R: Random>(
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
    let CarverConfig::Cave {
        y: y_provider,
        count,
        thickness: thickness_provider,
        weird_thickness_bias,
        room_vertical_radius_multiplier,
        horizontal_radius_multiplier,
        vertical_radius_multiplier,
        start_vertical_radius_multiplier,
        floor_level: floor_level_provider,
        ..
    } = *config
    else {
        return;
    };

    let cave_count = count.sample(rng);
    for _ in 0..cave_count {
        let x = (source_x * 16 + rng.next_i32_bound(16)) as f64;
        let y = y_provider.sample(rng, context) as f64;
        let z = (source_z * 16 + rng.next_i32_bound(16)) as f64;
        let horizontal_multiplier = horizontal_radius_multiplier.sample(rng) as f64;
        let vertical_multiplier = vertical_radius_multiplier.sample(rng) as f64;
        let start_vertical_multiplier = start_vertical_radius_multiplier.sample(rng) as f64;
        let shape_kind = CarveShape::Cave {
            floor_level: floor_level_provider.sample(rng) as f64,
        };

        let mut tunnels = 1;
        if rng.next_i32_bound(4) == 0 {
            let y_scale = room_vertical_radius_multiplier.sample(rng) as f64;
            let thickness = 1.0 + rng.next_f32() * 6.0;
            create_room(
                chunk_x, chunk_z, x, y, z, thickness, y_scale, shape_kind, water, mask,
            );
            tunnels += rng.next_i32_bound(4);
        }

        for _ in 0..tunnels {
            let yaw = rng.next_f32() * std::f32::consts::TAU;
            let pitch = (rng.next_f32() - 0.5) / 4.0;
            let thickness = sample_thickness(thickness_provider, weird_thickness_bias, rng);
            let length = tunnel_length();
            let distance = length - rng.next_i32_bound(length / 4);
            let mut tunnel_rng = LegacyRandom::new(rng.next_java_long() as u64);
            walk_tunnel(
                chunk_x,
                chunk_z,
                x,
                y,
                z,
                TunnelShape {
                    thickness,
                    y_scale: start_vertical_multiplier,
                    horizontal_radius_multiplier: horizontal_multiplier,
                    vertical_radius_multiplier: vertical_multiplier,
                },
                yaw,
                pitch,
                0,
                distance,
                false,
                SplitSeeding::FromTunnel,
                shape_kind,
                water,
                mask,
                &mut tunnel_rng,
                rng,
            );
        }
    }
}

/// `CaveWorldCarver.getThickness`. The bias draws only when it fires, so it is
/// part of the draw order, not a post-processing step.
fn sample_thickness<R: Random>(provider: FloatProvider, weird_bias: bool, rng: &mut R) -> f32 {
    let thickness = provider.sample(rng);
    if weird_bias && rng.next_i32_bound(10) == 0 {
        return thickness * (rng.next_f32() * rng.next_f32() * 3.0 + 1.0);
    }
    thickness
}

/// `CaveWorldCarver.createRoom`: one ellipsoid at the sine table's quarter
/// turn, offset a block east of the cave's origin.
#[allow(clippy::too_many_arguments)]
fn create_room(
    chunk_x: i32,
    chunk_z: i32,
    x: f64,
    y: f64,
    z: f64,
    thickness: f32,
    y_scale: f64,
    shape_kind: CarveShape<'_>,
    water: &WaterMask,
    mask: &mut CarvingMask,
) {
    let horizontal_radius = 1.5 + (math_helper_sin(std::f32::consts::FRAC_PI_2) * thickness) as f64;
    crate::carver::carve_ellipsoid(
        chunk_x,
        chunk_z,
        x + 1.0,
        y,
        z,
        horizontal_radius,
        horizontal_radius * y_scale,
        shape_kind,
        water,
        mask,
    );
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_worldgen::value_provider::{HeightProvider, IntProvider, VerticalAnchor};

    fn overworld() -> HeightContext {
        HeightContext {
            min_y: -64,
            depth: 384,
            sea_level: 63,
        }
    }

    /// Every provider constant, so the only draws left are the fixed ones the
    /// reference takes, and a replay can reproduce them exactly.
    fn constant_cave(count: i32, weird_thickness_bias: bool) -> CarverConfig {
        CarverConfig::Cave {
            probability: 0.15,
            y: HeightProviderFixture::at(40),
            count: IntProvider::Constant(count),
            thickness: FloatProvider::Constant(2.0),
            weird_thickness_bias,
            room_vertical_radius_multiplier: FloatProvider::Constant(0.5),
            horizontal_radius_multiplier: FloatProvider::Constant(1.0),
            vertical_radius_multiplier: FloatProvider::Constant(1.0),
            start_vertical_radius_multiplier: FloatProvider::Constant(1.0),
            floor_level: FloatProvider::Constant(-0.7),
        }
    }

    struct HeightProviderFixture;

    impl HeightProviderFixture {
        fn at(y: i32) -> HeightProvider {
            HeightProvider::Constant(VerticalAnchor::Absolute(y))
        }
    }

    fn empty_mask() -> CarvingMask {
        CarvingMask::new(16, -63, 312)
    }

    #[test]
    fn the_start_draw_is_one_float_against_the_probability() {
        let config = constant_cave(1, false);
        let mut rng = LegacyRandom::new(9);
        let mut replay = LegacyRandom::new(9);
        let started = is_start_chunk(&config, &mut rng);
        assert_eq!(started, replay.next_f32() <= 0.15);
        assert_eq!(rng.next_java_long(), replay.next_java_long());
    }

    #[test]
    fn a_zero_count_draws_nothing_and_marks_nothing() {
        let config = constant_cave(0, false);
        let mut mask = empty_mask();
        let mut rng = LegacyRandom::new(3);
        let mut replay = LegacyRandom::new(3);
        carve_caves(
            &config,
            overworld(),
            0,
            0,
            0,
            0,
            &WaterMask::default(),
            &mut mask,
            &mut rng,
        );
        assert!(mask.is_empty());
        assert_eq!(rng.next_java_long(), replay.next_java_long());
    }

    /// The entry's draw order, written out the way `CaveWorldCarver.carve`
    /// takes it. If the carver reordered a draw, or took one the reference does
    /// not, the two generators would part company here.
    #[test]
    fn the_entry_draws_in_the_reference_order() {
        let config = constant_cave(1, false);
        let mut carved = empty_mask();
        let mut rng = LegacyRandom::new(2024);
        carve_caves(
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

        let mut replay = LegacyRandom::new(2024);
        let mut replayed = empty_mask();
        let mut unused = LegacyRandom::new(1);
        let shape_kind = CarveShape::Cave { floor_level: -0.7 };
        let x = replay.next_i32_bound(16) as f64;
        let y = 40.0;
        let z = replay.next_i32_bound(16) as f64;
        let mut tunnels = 1;
        if replay.next_i32_bound(4) == 0 {
            let thickness = 1.0 + replay.next_f32() * 6.0;
            create_room(
                0,
                0,
                x,
                y,
                z,
                thickness,
                0.5,
                shape_kind,
                &WaterMask::default(),
                &mut replayed,
            );
            tunnels += replay.next_i32_bound(4);
        }
        for _ in 0..tunnels {
            let yaw = replay.next_f32() * std::f32::consts::TAU;
            let pitch = (replay.next_f32() - 0.5) / 4.0;
            let thickness = 2.0;
            let length = tunnel_length();
            let distance = length - replay.next_i32_bound(length / 4);
            let mut tunnel_rng = LegacyRandom::new(replay.next_java_long() as u64);
            walk_tunnel(
                0,
                0,
                x,
                y,
                z,
                TunnelShape {
                    thickness,
                    y_scale: 1.0,
                    horizontal_radius_multiplier: 1.0,
                    vertical_radius_multiplier: 1.0,
                },
                yaw,
                pitch,
                0,
                distance,
                false,
                SplitSeeding::FromTunnel,
                shape_kind,
                &WaterMask::default(),
                &mut replayed,
                &mut tunnel_rng,
                &mut unused,
            );
        }

        assert_eq!(
            rng.next_java_long(),
            replay.next_java_long(),
            "the carver and the replay consumed different draws"
        );
        assert_eq!(runs(&carved), runs(&replayed));
    }

    /// The bias fires on its own draw and multiplies by two more, so a config
    /// that enables it consumes strictly more of the stream.
    #[test]
    fn the_weird_thickness_bias_costs_its_own_draws() {
        let mut plain = LegacyRandom::new(77);
        let mut biased = LegacyRandom::new(77);
        let a = sample_thickness(FloatProvider::Constant(2.0), false, &mut plain);
        let b = sample_thickness(FloatProvider::Constant(2.0), true, &mut biased);
        assert_eq!(a, 2.0);
        assert_ne!(
            plain.next_java_long(),
            biased.next_java_long(),
            "the bias must take at least its gate draw"
        );
        assert!(b >= 2.0, "the bias only ever thickens: {b}");
    }

    #[test]
    fn the_room_sits_a_block_east_of_the_cave_origin() {
        let mut offset = empty_mask();
        create_room(
            0,
            0,
            8.0,
            40.0,
            8.0,
            3.0,
            1.0,
            CarveShape::Cave { floor_level: -0.7 },
            &WaterMask::default(),
            &mut offset,
        );
        let mut centred = empty_mask();
        crate::carver::carve_ellipsoid(
            0,
            0,
            8.0,
            40.0,
            8.0,
            1.5 + (math_helper_sin(std::f32::consts::FRAC_PI_2) * 3.0) as f64,
            1.5 + (math_helper_sin(std::f32::consts::FRAC_PI_2) * 3.0) as f64,
            CarveShape::Cave { floor_level: -0.7 },
            &WaterMask::default(),
            &mut centred,
        );
        assert!(!offset.is_empty());
        assert_ne!(runs(&offset), runs(&centred));
    }

    #[test]
    fn a_cave_carver_marks_cells_inside_the_target_chunk() {
        let config = constant_cave(4, true);
        let mut mask = empty_mask();
        let mut rng = LegacyRandom::new(555);
        for source_x in -1..=1 {
            for source_z in -1..=1 {
                carve_caves(
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
        assert!(!mask.is_empty(), "nine sources carved nothing");
        mask.visit(|x, z, bottom, top| {
            assert!((0..16).contains(&x), "x {x} outside the chunk");
            assert!((0..16).contains(&z), "z {z} outside the chunk");
            assert!(
                bottom >= -63 && top <= 312,
                "Y {bottom}..={top} out of range"
            );
        });
    }

    fn runs(mask: &CarvingMask) -> Vec<(i32, i32, i32, i32)> {
        let mut out = Vec::new();
        mask.visit(|x, z, bottom, top| out.push((x, z, bottom, top)));
        out
    }
}
