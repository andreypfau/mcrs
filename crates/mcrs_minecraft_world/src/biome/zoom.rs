use sha2::{Digest, Sha256};

/// The seed enters the digest as eight little-endian bytes and the first eight
/// digest bytes are read back the same way; both orderings are part of the value.
pub fn obfuscate_seed(seed: i64) -> i64 {
    let digest = Sha256::digest(seed.to_le_bytes());
    i64::from_le_bytes(digest[..8].try_into().unwrap())
}

pub fn quart_cell(zoom_seed: i64, x: i32, y: i32, z: i32) -> (i32, i32, i32) {
    let (abs_x, abs_y, abs_z) = (x - 2, y - 2, z - 2);
    let (parent_x, parent_y, parent_z) = (abs_x >> 2, abs_y >> 2, abs_z >> 2);
    let fract_x = f64::from(abs_x & 3) / 4.0;
    let fract_y = f64::from(abs_y & 3) / 4.0;
    let fract_z = f64::from(abs_z & 3) / 4.0;

    let mut best = 0;
    let mut best_distance = f64::INFINITY;
    for i in 0..8 {
        let (corner_x, distance_x) = if i & 4 == 0 {
            (parent_x, fract_x)
        } else {
            (parent_x + 1, fract_x - 1.0)
        };
        let (corner_y, distance_y) = if i & 2 == 0 {
            (parent_y, fract_y)
        } else {
            (parent_y + 1, fract_y - 1.0)
        };
        let (corner_z, distance_z) = if i & 1 == 0 {
            (parent_z, fract_z)
        } else {
            (parent_z + 1, fract_z - 1.0)
        };
        let next = fiddled_distance(
            zoom_seed, corner_x, corner_y, corner_z, distance_x, distance_y, distance_z,
        );
        if best_distance > next {
            best = i;
            best_distance = next;
        }
    }

    (
        if best & 4 == 0 { parent_x } else { parent_x + 1 },
        if best & 2 == 0 { parent_y } else { parent_y + 1 },
        if best & 1 == 0 { parent_z } else { parent_z + 1 },
    )
}

fn fiddled_distance(
    seed: i64,
    x_random: i32,
    y_random: i32,
    z_random: i32,
    distance_x: f64,
    distance_y: f64,
    distance_z: f64,
) -> f64 {
    let (x, y, z) = (i64::from(x_random), i64::from(y_random), i64::from(z_random));
    let mut rval = lcg_next(seed, x);
    rval = lcg_next(rval, y);
    rval = lcg_next(rval, z);
    rval = lcg_next(rval, x);
    rval = lcg_next(rval, y);
    rval = lcg_next(rval, z);
    let fiddle_x = fiddle(rval);
    rval = lcg_next(rval, seed);
    let fiddle_y = fiddle(rval);
    rval = lcg_next(rval, seed);
    let fiddle_z = fiddle(rval);

    let (dz, dy, dx) = (
        distance_z + fiddle_z,
        distance_y + fiddle_y,
        distance_x + fiddle_x,
    );
    dz * dz + dy * dy + dx * dx
}

fn lcg_next(rval: i64, c: i64) -> i64 {
    rval.wrapping_mul(
        rval.wrapping_mul(6364136223846793005)
            .wrapping_add(1442695040888963407),
    )
    .wrapping_add(c)
}

fn fiddle(rval: i64) -> f64 {
    let uniform = (rval >> 24).rem_euclid(1024) as f64 / 1024.0;
    (uniform - 0.5) * 0.9
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Each value below is reproducible without this crate, the seed dropped
    /// into the pack:
    /// `python3 -c "import hashlib,struct;print(int.from_bytes(hashlib.sha256(struct.pack('<q',0)).digest()[:8],'little',signed=True))"`
    #[test]
    fn obfuscate_seed_matches_sha256() {
        assert_eq!(obfuscate_seed(0), 8794265229978523055);
        assert_eq!(obfuscate_seed(1), -6467378160175308932);
        assert_eq!(obfuscate_seed(-1), 6759447113877070610);
        assert_eq!(obfuscate_seed(42), -4111196313959201555);
        assert_eq!(obfuscate_seed(i64::MIN), 6374347445474471398);
    }

    #[test]
    fn selects_one_of_the_eight_surrounding_corners() {
        let seed = obfuscate_seed(-97);
        for x in -9..9 {
            for y in -9..9 {
                for z in -9..9 {
                    let (cx, cy, cz) = quart_cell(seed, x, y, z);
                    let (px, py, pz) = ((x - 2) >> 2, (y - 2) >> 2, (z - 2) >> 2);
                    assert!(cx == px || cx == px + 1, "x {x} -> {cx}, parent {px}");
                    assert!(cy == py || cy == py + 1, "y {y} -> {cy}, parent {py}");
                    assert!(cz == pz || cz == pz + 1, "z {z} -> {cz}, parent {pz}");
                }
            }
        }
    }

    /// The cell asserted below came out of the same independent transcription
    /// as the distances in `quart_cell_centre_picks_the_low_corner`.
    #[test]
    fn is_a_pure_function_of_seed_and_position() {
        let a = obfuscate_seed(42);
        let b = obfuscate_seed(43);
        assert_eq!(quart_cell(a, 100, 64, -37), quart_cell(a, 100, 64, -37));
        assert_eq!(quart_cell(a, 100, 64, -37), (24, 15, -10));

        let mut differs = 0;
        for x in 0..64 {
            if quart_cell(a, x, 64, -37) != quart_cell(b, x, 64, -37) {
                differs += 1;
            }
        }
        assert!(differs > 0, "the seed does not reach the selection");
    }

    /// The eight distances come from a transcription of the reference zoom
    /// written against it rather than against this module: six wrapping-i64 LCG
    /// steps over the corner coordinates and two over the seed, each fiddle
    /// `(((rval >> 24) mod 1024) / 1024 - 0.5) * 0.9`, summed as
    /// `(distance + fiddle)^2` over the three axes. The shift is arithmetic and
    /// the modulo floors; taking either the other way moves every negative
    /// `rval`.
    #[test]
    fn quart_cell_centre_picks_the_low_corner() {
        let seed = obfuscate_seed(42);
        assert_eq!(quart_cell(seed, 2, 2, 2), (0, 0, 0));

        let expected = [
            0.18402854919433595,
            2.2654219245910645,
            0.5916829013824463,
            3.285861644744873,
            0.6454002094268798,
            1.4180008125305177,
            1.3354431915283203,
            3.0898364067077635,
        ];
        for (i, want) in expected.iter().enumerate() {
            let i = i as i32;
            let (cx, dx) = if i & 4 == 0 { (0, 0.0) } else { (1, -1.0) };
            let (cy, dy) = if i & 2 == 0 { (0, 0.0) } else { (1, -1.0) };
            let (cz, dz) = if i & 1 == 0 { (0, 0.0) } else { (1, -1.0) };
            assert_eq!(fiddled_distance(seed, cx, cy, cz, dx, dy, dz), *want, "{i}");
        }
    }
}
