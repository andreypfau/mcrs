use sha2::{Digest, Sha256};

/// The seed enters the digest as eight little-endian bytes and the first eight
/// digest bytes are read back the same way; both orderings are part of the value.
pub fn obfuscate_seed(seed: i64) -> i64 {
    let digest = Sha256::digest(seed.to_le_bytes());
    i64::from_le_bytes(digest[..8].try_into().unwrap())
}

pub fn quart_cell(zoom_seed: i64, x: i32, y: i32, z: i32) -> (i32, i32, i32) {
    pick_corner(x, y, z, |cx, cy, cz| corner_fiddles(zoom_seed, cx, cy, cz))
}

/// The eight corners around a block, in the order the reference tries them.
/// `fiddles` answers with a corner's three offsets, which depend on the seed
/// and the corner alone.
fn pick_corner(
    x: i32,
    y: i32,
    z: i32,
    mut fiddles: impl FnMut(i32, i32, i32) -> [f64; 3],
) -> (i32, i32, i32) {
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
        let [fiddle_x, fiddle_y, fiddle_z] = fiddles(corner_x, corner_y, corner_z);
        let (dz, dy, dx) = (
            distance_z + fiddle_z,
            distance_y + fiddle_y,
            distance_x + fiddle_x,
        );
        let next = dz * dz + dy * dy + dx * dx;
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

/// A corner's offsets memoised over the box one column's zoom can reach. Eight
/// LCG chains per corner are paid once per corner instead of once per block:
/// a strip asks for the same corners at every height it descends through.
#[derive(Default)]
pub struct FiddleCache {
    seed: i64,
    origin: [i32; 3],
    size: [i32; 3],
    generation: u64,
    stamps: Vec<u64>,
    values: Vec<[f64; 3]>,
}

impl FiddleCache {
    /// `origin` and `size` are in corner cells. A corner outside the box is
    /// still answered, just not remembered.
    pub fn begin(&mut self, seed: i64, origin: [i32; 3], size: [i32; 3]) {
        let cells = size.iter().map(|n| (*n).max(0) as usize).product();
        if self.stamps.len() != cells {
            self.stamps.clear();
            self.stamps.resize(cells, 0);
            self.values.clear();
            self.values.resize(cells, [0.0; 3]);
            self.generation = 0;
        }
        self.seed = seed;
        self.origin = origin;
        self.size = size;
        self.generation += 1;
    }

    #[inline]
    fn slot(&self, x: i32, y: i32, z: i32) -> Option<usize> {
        let at = [x - self.origin[0], y - self.origin[1], z - self.origin[2]];
        if (0..3).any(|axis| at[axis] < 0 || at[axis] >= self.size[axis]) {
            return None;
        }
        Some(((at[2] * self.size[0] + at[0]) * self.size[1] + at[1]) as usize)
    }

    #[inline]
    fn fiddles(&mut self, x: i32, y: i32, z: i32) -> [f64; 3] {
        let Some(slot) = self.slot(x, y, z) else {
            return corner_fiddles(self.seed, x, y, z);
        };
        if self.stamps[slot] != self.generation {
            self.stamps[slot] = self.generation;
            self.values[slot] = corner_fiddles(self.seed, x, y, z);
        }
        self.values[slot]
    }

    pub fn quart_cell(&mut self, x: i32, y: i32, z: i32) -> (i32, i32, i32) {
        pick_corner(x, y, z, |cx, cy, cz| self.fiddles(cx, cy, cz))
    }
}

fn corner_fiddles(seed: i64, x_random: i32, y_random: i32, z_random: i32) -> [f64; 3] {
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
    [fiddle_x, fiddle_y, fiddle_z]
}

#[cfg(test)]
fn fiddled_distance(
    seed: i64,
    x_random: i32,
    y_random: i32,
    z_random: i32,
    distance_x: f64,
    distance_y: f64,
    distance_z: f64,
) -> f64 {
    let [fiddle_x, fiddle_y, fiddle_z] = corner_fiddles(seed, x_random, y_random, z_random);
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

    /// The cache only remembers corners; a box that covers part of the range
    /// and a second `begin` over a different box must both still answer with
    /// what the plain function computes.
    #[test]
    fn the_fiddle_cache_answers_as_the_plain_zoom_does() {
        let seed = obfuscate_seed(777);
        let mut cache = FiddleCache::default();
        for (origin, size) in [
            ([0, 0, 0], [6, 8, 6]),
            ([-3, -20, 4], [6, 98, 6]),
            ([0, 0, 0], [0, 0, 0]),
        ] {
            cache.begin(seed, origin, size);
            for x in -20..20 {
                for y in -12..12 {
                    for z in -20..20 {
                        assert_eq!(
                            cache.quart_cell(x, y, z),
                            quart_cell(seed, x, y, z),
                            "{origin:?} {size:?} at {x},{y},{z}"
                        );
                    }
                }
            }
        }
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
