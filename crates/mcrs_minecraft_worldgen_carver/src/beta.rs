use crate::CarveShape;
use crate::mask::CarvingMask;
use crate::modern::SOURCE_RADIUS;
use crate::tunnel::{SplitSeeding, TrigIndex, TunnelShape, walk_tunnel};
use crate::water::WaterMask;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;

/// `MapGenCaves`: what one source chunk carves into the target chunk.
pub fn carve_beta_caves<R: Random>(
    chunk_x: i32,
    chunk_z: i32,
    source_x: i32,
    source_z: i32,
    water: &WaterMask,
    mask: &mut CarvingMask,
    rng: &mut R,
) {
    let shape_kind = CarveShape::Cave {
        floor_level: BETA_FLOOR_LEVEL,
    };
    let cave_count = {
        let a = rng.next_i32_bound(40) + 1;
        let b = rng.next_i32_bound(a) + 1;
        rng.next_i32_bound(b)
    };
    let cave_count = if rng.next_i32_bound(15) != 0 {
        0
    } else {
        cave_count
    };

    for _ in 0..cave_count {
        let x = (source_x * 16 + rng.next_i32_bound(16)) as f64;
        let y_bound = rng.next_i32_bound(120) + 8;
        let y = rng.next_i32_bound(y_bound) as f64;
        let z = (source_z * 16 + rng.next_i32_bound(16)) as f64;

        let mut tunnels = 1;
        if rng.next_i32_bound(4) == 0 {
            let thickness = 1.0 + rng.next_f32() * 6.0;
            let mut room_rng = LegacyRandom::new(rng.next_java_long() as u64);
            let total_steps = tunnel_length(&mut room_rng);
            walk_tunnel(
                chunk_x,
                chunk_z,
                x,
                y,
                z,
                beta_shape(thickness, 0.5),
                0.0,
                0.0,
                total_steps / 2,
                total_steps,
                true,
                SplitSeeding::FromParent,
                shape_kind,
                water,
                mask,
                &mut room_rng,
                rng,
            );
            tunnels += rng.next_i32_bound(4);
        }

        for _ in 0..tunnels {
            let yaw = rng.next_f32() * std::f32::consts::TAU;
            let pitch = (rng.next_f32() - 0.5) * 2.0 / 8.0;
            let thickness = rng.next_f32() * 2.0 + rng.next_f32();
            let mut tunnel_rng = LegacyRandom::new(rng.next_java_long() as u64);
            let total_steps = tunnel_length(&mut tunnel_rng);
            walk_tunnel(
                chunk_x,
                chunk_z,
                x,
                y,
                z,
                beta_shape(thickness, 1.0),
                yaw,
                pitch,
                0,
                total_steps,
                false,
                SplitSeeding::FromParent,
                shape_kind,
                water,
                mask,
                &mut tunnel_rng,
                rng,
            );
        }
    }
}

/// Beta sizes a tunnel off the same field its source loop walks, so the radius
/// the loop shares with the modern carvers is the one the length derives from.
const TUNNEL_LENGTH: i32 = SOURCE_RADIUS * 16 - 16;

/// Beta draws the length from the tunnel's own generator, before the split
/// point and the steepness; the modern carvers draw it from the source's.
fn tunnel_length(rng: &mut LegacyRandom) -> i32 {
    TUNNEL_LENGTH - rng.next_i32_bound(TUNNEL_LENGTH / 4)
}

const BETA_FLOOR_LEVEL: f64 = -0.7;

fn beta_shape(thickness: f32, y_scale: f64) -> TunnelShape {
    TunnelShape {
        thickness,
        y_scale,
        horizontal_radius_multiplier: 1.0,
        vertical_radius_multiplier: 1.0,
        trig: TrigIndex::Beta,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::carve_ellipsoid;

    #[test]
    fn next_i32_bound_one_returns_zero() {
        let mut rng = LegacyRandom::new(12345);
        assert_eq!(rng.next_i32_bound(1), 0);
    }

    /// The mask marks the Y the ellipsoid test accepts, which sits within the
    /// vertical radius and never below the floor level.
    #[test]
    fn the_ellipsoid_marks_only_its_own_interior() {
        let mut mask = CarvingMask::new(16, 1, 120);
        assert!(carve_ellipsoid(
            0,
            0,
            8.0,
            50.0,
            8.0,
            3.0,
            2.0,
            CarveShape::Cave {
                floor_level: BETA_FLOOR_LEVEL,
            },
            &WaterMask::default(),
            &mut mask,
        ));

        let mut columns = 0;
        mask.visit(|x, z, bottom, top| {
            columns += 1;
            let xd = (x as f64 + 0.5 - 8.0) / 3.0;
            let zd = (z as f64 + 0.5 - 8.0) / 3.0;
            assert!(xd * xd + zd * zd < 1.0, "column ({x}, {z}) is outside");
            assert!((bottom as f64 - 0.5 - 50.0) / 2.0 > -0.7);
            assert!((top as f64 - 0.5 - 50.0) / 2.0 < 1.0);
        });
        assert!(columns > 0);
    }

    #[test]
    fn a_water_abort_marks_nothing_and_reports_it() {
        let mut water = WaterMask::default();
        water.insert(4, 50, 8);
        let mut mask = CarvingMask::new(16, 1, 120);
        assert!(!carve_ellipsoid(
            0,
            0,
            8.0,
            50.0,
            8.0,
            3.0,
            2.0,
            CarveShape::Cave {
                floor_level: BETA_FLOOR_LEVEL,
            },
            &water,
            &mut mask,
        ));
        assert!(mask.is_empty());
    }

    #[test]
    fn an_ellipsoid_far_from_the_chunk_marks_nothing() {
        let mut mask = CarvingMask::new(16, 1, 120);
        assert!(!carve_ellipsoid(
            0,
            0,
            200.0,
            50.0,
            8.0,
            3.0,
            2.0,
            CarveShape::Cave {
                floor_level: BETA_FLOOR_LEVEL,
            },
            &WaterMask::default(),
            &mut mask,
        ));
        assert!(mask.is_empty());
    }
}
