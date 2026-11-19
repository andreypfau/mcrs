use mcrs_minecraft_core::QuartPos;
use mcrs_voxel::BlockPos;
use sha2::{Digest, Sha256};
use std::array;

pub fn obfuscate_seed(seed: i64) -> i64 {
    let digest: [u8; 32] = Sha256::digest(seed.to_le_bytes()).into();
    let [b0, b1, b2, b3, b4, b5, b6, b7, ..] = digest;
    i64::from_le_bytes([b0, b1, b2, b3, b4, b5, b6, b7])
}

pub fn quart_cell(zoom_seed: i64, pos: BlockPos) -> QuartPos {
    pick_corner(pos.x, pos.y, pos.z, |cx, cy, cz| {
        corner_fiddles(zoom_seed, cx, cy, cz)
    })
}

fn cell_corners<B>(pos: BlockPos, mut corner: impl FnMut(QuartPos) -> B) -> [B; 8] {
    let (x, y, z) = ((pos.x - 2) >> 2, (pos.y - 2) >> 2, (pos.z - 2) >> 2);
    [
        corner(QuartPos::new(x, y, z)),
        corner(QuartPos::new(x, y, z + 1)),
        corner(QuartPos::new(x, y + 1, z)),
        corner(QuartPos::new(x, y + 1, z + 1)),
        corner(QuartPos::new(x + 1, y, z)),
        corner(QuartPos::new(x + 1, y, z + 1)),
        corner(QuartPos::new(x + 1, y + 1, z)),
        corner(QuartPos::new(x + 1, y + 1, z + 1)),
    ]
}

fn pick_corner(
    x: i32,
    y: i32,
    z: i32,
    mut fiddles: impl FnMut(i32, i32, i32) -> [f64; 3],
) -> QuartPos {
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

    QuartPos::new(
        if best & 4 == 0 {
            parent_x
        } else {
            parent_x + 1
        },
        if best & 2 == 0 {
            parent_y
        } else {
            parent_y + 1
        },
        if best & 1 == 0 {
            parent_z
        } else {
            parent_z + 1
        },
    )
}

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
        let [ox, oy, oz] = self.origin;
        let [sx, sy, sz] = self.size;
        let (x, y, z) = (x - ox, y - oy, z - oz);
        if !((0..sx).contains(&x) && (0..sy).contains(&y) && (0..sz).contains(&z)) {
            return None;
        }
        Some(((z * sx + x) * sy + y) as usize)
    }

    #[inline]
    fn fiddles(&mut self, x: i32, y: i32, z: i32) -> [f64; 3] {
        let Some(slot) = self.slot(x, y, z) else {
            return corner_fiddles(self.seed, x, y, z);
        };
        let (Some(stamp), Some(value)) = (self.stamps.get_mut(slot), self.values.get_mut(slot))
        else {
            return corner_fiddles(self.seed, x, y, z);
        };
        if *stamp != self.generation {
            *stamp = self.generation;
            *value = corner_fiddles(self.seed, x, y, z);
        }
        *value
    }

    pub fn cell_terms(&mut self, pos: BlockPos) -> CellTerms {
        let fiddles = cell_corners(pos, |corner| self.fiddles(corner.x, corner.y, corner.z));
        CellTerms {
            x: axis_terms(fiddles.map(|[x, _, _]| x), 4),
            y: axis_terms(fiddles.map(|[_, y, _]| y), 2),
            z: axis_terms(fiddles.map(|[_, _, z]| z), 1),
        }
    }
}

pub struct CellTerms {
    x: [[f64; 8]; 4],
    y: [[f64; 8]; 4],
    z: [[f64; 8]; 4],
}

impl CellTerms {
    #[must_use]
    #[inline]
    pub fn nearest<B: Copy>(&self, corners: &[B; 8], pos: BlockPos) -> B {
        let [x0, x1, x2, x3, x4, x5, x6, x7] = *fract_row(&self.x, pos.x);
        let [y0, y1, y2, y3, y4, y5, y6, y7] = *fract_row(&self.y, pos.y);
        let [z0, z1, z2, z3, z4, z5, z6, z7] = *fract_row(&self.z, pos.z);
        let [c0, c1, c2, c3, c4, c5, c6, c7] = *corners;
        Nearest {
            corner: c0,
            distance: f64::INFINITY,
        }
        .offer(c0, x0 + y0 + z0)
        .offer(c1, x1 + y1 + z1)
        .offer(c2, x2 + y2 + z2)
        .offer(c3, x3 + y3 + z3)
        .offer(c4, x4 + y4 + z4)
        .offer(c5, x5 + y5 + z5)
        .offer(c6, x6 + y6 + z6)
        .offer(c7, x7 + y7 + z7)
        .corner
    }
}

struct Nearest<B> {
    corner: B,
    distance: f64,
}

impl<B> Nearest<B> {
    #[inline]
    fn offer(self, corner: B, distance: f64) -> Self {
        if self.distance > distance {
            Self { corner, distance }
        } else {
            self
        }
    }
}

#[inline]
fn fract_row(rows: &[[f64; 8]; 4], coordinate: i32) -> &[f64; 8] {
    let [first, second, third, fourth] = rows;
    match (coordinate - 2) & 3 {
        0 => first,
        1 => second,
        2 => third,
        _ => fourth,
    }
}

fn axis_terms(fiddles: [f64; 8], high_bit: usize) -> [[f64; 8]; 4] {
    array::from_fn(|fract| {
        let fract = fract as f64 / 4.0;
        let mut terms = fiddles;
        for (corner, term) in terms.iter_mut().enumerate() {
            let distance = if corner & high_bit == 0 {
                fract
            } else {
                fract - 1.0
            };
            let offset = distance + *term;
            *term = offset * offset;
        }
        terms
    })
}

fn corner_fiddles(seed: i64, x_random: i32, y_random: i32, z_random: i32) -> [f64; 3] {
    let (x, y, z) = (
        i64::from(x_random),
        i64::from(y_random),
        i64::from(z_random),
    );
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
                    let QuartPos {
                        x: cx,
                        y: cy,
                        z: cz,
                    } = quart_cell(seed, BlockPos::new(x, y, z));
                    let (px, py, pz) = ((x - 2) >> 2, (y - 2) >> 2, (z - 2) >> 2);
                    assert!(cx == px || cx == px + 1, "x {x} -> {cx}, parent {px}");
                    assert!(cy == py || cy == py + 1, "y {y} -> {cy}, parent {py}");
                    assert!(cz == pz || cz == pz + 1, "z {z} -> {cz}, parent {pz}");
                }
            }
        }
    }

    #[test]
    fn a_cell_away_from_the_origin_matches_the_transcription() {
        assert_eq!(
            quart_cell(obfuscate_seed(42), BlockPos::new(100, 64, -37)),
            QuartPos::new(24, 15, -10)
        );
    }

    #[test]
    fn quart_cell_centre_picks_the_low_corner() {
        let seed = obfuscate_seed(42);
        assert_eq!(
            quart_cell(seed, BlockPos::new(2, 2, 2)),
            QuartPos::new(0, 0, 0)
        );

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

    mod exhaustive {
        use super::*;

        #[test]
        #[expect(
            clippy::excessive_nesting,
            reason = "sweeps both axes of two begun boxes and compares each cell with the plain zoom"
        )]
        fn the_cell_terms_pick_the_corner_the_plain_zoom_picks() {
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
                            let pos = BlockPos::new(x, y, z);
                            assert_eq!(
                                cache
                                    .cell_terms(pos)
                                    .nearest(&cell_corners(pos, |corner| corner), pos),
                                quart_cell(seed, pos),
                                "{origin:?} {size:?} at {x},{y},{z}"
                            );
                        }
                    }
                }
            }
        }
    }
}
