use crate::density_function::beta_seed::seed_beta_terrain_f64;
use crate::noise::octave_perlin_noise::OctavePerlinNoise;

/// Density grid dimensions matching Java's `computeDensity` output.
///
/// b0=4 cells per chunk axis, b2=17 Y levels, kk=ll=b0+1=5 grid points per XZ axis.
/// Grid layout: index = (ix * LL + iz) * B2 + iy
pub const CD_B0: usize = 4;
pub const CD_B2: usize = 17;
pub const CD_LL: usize = CD_B0 + 1; // 5
pub const CD_GRID: usize = CD_LL * CD_B2 * CD_LL; // 5 * 17 * 5 = 425

/// f64-precision Beta terrain density noises, seeded once per world.
///
/// Holds the five Java `NoiseGeneratorOctaves` used in `computeDensity`:
///   this.k / this.l  (low/high, 16 octaves each)  → e / f arrays
///   this.m           (selector, 8 octaves)          → d array
///   this.a           (scale, 10 octaves)             → g array
///   this.b           (depth, 16 octaves)             → h array
pub struct BetaTerrainF64 {
    low:      OctavePerlinNoise<f64>, // this.k (e)
    high:     OctavePerlinNoise<f64>, // this.l (f)
    selector: OctavePerlinNoise<f64>, // this.m (d)
    scale:    OctavePerlinNoise<f64>, // this.a (g)
    depth:    OctavePerlinNoise<f64>, // this.b (h)
}

impl BetaTerrainF64 {
    pub fn new(seed: u64) -> Self {
        let (low, high, selector, _beach, _surface, scale, depth) = seed_beta_terrain_f64(seed);
        Self { low, high, selector, scale, depth }
    }

    /// Port of Java `computeDensity(double[], i, j2=0, kk=chunkZ*4, ll=5, i1=17, j1=5)`.
    ///
    /// Fills a 5×17×5 density grid in f64.  Layout: `grid[(ix * LL + iz) * B2 + iy]`.
    ///
    /// `chunk_x` / `chunk_z` are chunk coordinates (multiply by 4 to get noise-cell coords).
    /// `temp_grid` is the 16×16 temperature map (row-major, index = x*16+z) produced by
    /// the climate simplex noise.  Java reads `adouble1[k2*16+i3]` and `adouble2[k2*16+i3]`
    /// where k2=ix_cell*4+2, i3=iz_cell*4+2 (centre of the 4×4 block-cell).
    pub fn compute_density(
        &self,
        chunk_x: i32,
        chunk_z: i32,
        temp_grid: &[f32; 256],
        rain_grid: &[f32; 256],
    ) -> [f64; CD_GRID] {
        let i  = chunk_x * CD_B0 as i32;  // x start in noise-cell coords
        let kk = chunk_z * CD_B0 as i32;  // z start in noise-cell coords
        let ll = CD_LL as i32;            // xSize = zSize = 5
        let j1 = CD_LL as i32;
        let i1 = CD_B2 as i32;           // ySize = 17

        let d0 = 684.412_f64;
        let d1 = 684.412_f64;

        // Sample 2D scale/depth grids (xSize=5, zSize=5, ySize=1, y=10.0 fixed)
        // Java: this.a.a(this.g, i, kk, ll, j1, 1.121, 1.121, 0.5)
        //       → a(arr, i, 10.0, kk, 5, 1, 5, 1.121, 1.0, 1.121)
        let mut g = [0.0_f64; 25]; // 5*5
        {
            let mut idx = 0usize;
            for ix in 0..ll {
                for iz in 0..j1 {
                    let x = (i + ix) as f64;
                    let z = (kk + iz) as f64;
                    g[idx] = self.scale.sample_xz_beta(x, z, 1.121, 1.121);
                    idx += 1;
                }
            }
        }

        let mut h = [0.0_f64; 25]; // 5*5
        {
            let mut idx = 0usize;
            for ix in 0..ll {
                for iz in 0..j1 {
                    let x = (i + ix) as f64;
                    let z = (kk + iz) as f64;
                    h[idx] = self.depth.sample_xz_beta(x, z, 200.0, 200.0);
                    idx += 1;
                }
            }
        }

        // Sample 3D selector/low/high grids using Java-exact bulk fill.
        // The bulk fill replicates Java's y-lattice cache that persists across (x,z) columns,
        // which differs from per-point evaluation for high-frequency octaves.
        let mut d_arr = [0.0_f64; CD_GRID]; // selector
        let mut e_arr = [0.0_f64; CD_GRID]; // low
        let mut f_arr = [0.0_f64; CD_GRID]; // high
        self.selector.fill_3d_bulk(
            &mut d_arr,
            i as f64, 0.0, kk as f64,
            ll as usize, i1 as usize, j1 as usize,
            d0 / 80.0, d1 / 160.0, d0 / 80.0,
        );
        self.low.fill_3d_bulk(
            &mut e_arr,
            i as f64, 0.0, kk as f64,
            ll as usize, i1 as usize, j1 as usize,
            d0, d1, d0,
        );
        self.high.fill_3d_bulk(
            &mut f_arr,
            i as f64, 0.0, kk as f64,
            ll as usize, i1 as usize, j1 as usize,
            d0, d1, d0,
        );

        // Java's nested loop: k1=0..ll (x), l2=0..j1 (z), j3=0..i1 (y)
        // l1 indexes the 2D g/h arrays; k1 indexes the 3D d/e/f arrays.
        let mut out = [0.0_f64; CD_GRID];
        let mut l1 = 0usize; // index into g/h (2D, per xz cell)
        let mut k1 = 0usize; // index into d/e/f (3D, per xyz voxel)

        // Java cell stride: cell centre for climate lookup
        // i2 = ix_cell * cell_size + cell_size/2, where cell_size = 16/ll = 16/5 ... wait
        // Java: i2 = 16 / ll = 3 (integer division), k2 = j2i * i2 + i2/2 = ix*3+1
        //       i3 = l2 * i2 + i2/2 = iz*3+1
        // But back2beta uses ll=5, so i2=16/5=3 (integer division), i2/2=1
        let cell_size = 16 / (ll as usize); // = 3
        let cell_half = cell_size / 2;      // = 1

        for j2i in 0..(ll as usize) {
            let k2 = j2i * cell_size + cell_half; // x offset into 16x16 for climate
            for l2 in 0..(j1 as usize) {
                let i3 = l2 * cell_size + cell_half; // z offset into 16x16 for climate

                let temp  = temp_grid[k2 * 16 + i3] as f64;
                let rain  = rain_grid[k2 * 16 + i3] as f64 * temp;

                let mut d4 = 1.0 - rain;
                d4 *= d4;
                d4 *= d4;
                d4 = 1.0 - d4;

                let mut d5 = (g[l1] + 256.0) / 512.0;
                d5 *= d4;
                if d5 > 1.0 { d5 = 1.0; }

                let mut d6 = h[l1] / 8000.0;
                if d6 < 0.0 { d6 = -d6 * 0.3; }
                d6 = d6 * 3.0 - 2.0;
                if d6 < 0.0 {
                    d6 /= 2.0;
                    if d6 < -1.0 { d6 = -1.0; }
                    d6 /= 1.4;
                    d6 /= 2.0;
                    d5 = 0.0;
                } else {
                    if d6 > 1.0 { d6 = 1.0; }
                    d6 /= 8.0;
                }
                if d5 < 0.0 { d5 = 0.0; }
                d5 += 0.5;
                d6 = d6 * (i1 as f64) / 16.0;

                let d7 = (i1 as f64) / 2.0 + d6 * 4.0;
                l1 += 1;

                for j3 in 0..(i1 as usize) {
                    let mut d8 = 0.0_f64;
                    let d9_raw = (j3 as f64 - d7) * 12.0 / d5;
                    let d9 = if d9_raw < 0.0 { d9_raw * 4.0 } else { d9_raw };

                    let d10 = e_arr[k1] / 512.0;
                    let d11 = f_arr[k1] / 512.0;
                    let d12 = (d_arr[k1] / 10.0 + 1.0) / 2.0;

                    if d12 < 0.0 {
                        d8 = d10;
                    } else if d12 > 1.0 {
                        d8 = d11;
                    } else {
                        d8 = d10 + (d11 - d10) * d12;
                    }
                    d8 -= d9;

                    if j3 > (i1 as usize) - 4 {
                        let d13 = (j3 as f64 - ((i1 as f64) - 4.0)) / 3.0;
                        d8 = d8 * (1.0 - d13) + -10.0 * d13;
                    }

                    out[k1] = d8;
                    k1 += 1;
                }
            }
        }

        out
    }

    /// Port of Java `fillDensityTerrain(i, jj, abyte, abiomebase, adouble)`.
    ///
    /// Trilinearly interpolates the 5×17×5 density grid to per-block density,
    /// then places stone / water / ice according to the Java sign and sea-level rules.
    ///
    /// Returns a flat `[u32; 16*128*16]` block array (Y-major order: `arr[x*128+y]` for
    /// each z in the outer loop — matching Java's byte array layout for the caller).
    /// Specifically: index = `(x * 16 + z) * 128 + y` i.e. Java's `j2 = i2+i1*4<<11 | j1*4<<7 | k1*8+l1`.
    /// We return 0 for air.
    ///
    /// `density_grid` is the output of `compute_density`.
    /// `temp_grid`    is the 16×16 temperature map (index x*16+z).
    /// `sea_level`    is typically 64.
    /// `stone_id`, `water_id`, `ice_id` are the block state IDs to place.
    pub fn fill_terrain(
        density_grid: &[f64; CD_GRID],
        temp_grid: &[f32; 256],
        sea_level: i32,
        stone_id: u32,
        water_id: u32,
        ice_id: u32,
    ) -> [u32; 16 * 128 * 16] {
        let mut out = [0u32; 16 * 128 * 16];

        let b0 = CD_B0 as i32;  // 4
        let b1 = sea_level as usize;
        let b2 = CD_B2 as i32;  // 17
        let ll = CD_LL as i32;  // 5

        for i1 in 0..b0 {
            for j1 in 0..b0 {
                for k1 in 0..16i32 {
                    let d0 = 0.125_f64;

                    let d1 = density_grid[((i1 + 0) * ll + j1 + 0) as usize * CD_B2 + k1 as usize];
                    let d2 = density_grid[((i1 + 0) * ll + j1 + 1) as usize * CD_B2 + k1 as usize];
                    let d3 = density_grid[((i1 + 1) * ll + j1 + 0) as usize * CD_B2 + k1 as usize];
                    let d4 = density_grid[((i1 + 1) * ll + j1 + 1) as usize * CD_B2 + k1 as usize];

                    let mut dd1 = d1;
                    let mut dd2 = d2;
                    let mut dd3 = d3;
                    let mut dd4 = d4;

                    let d5 = (density_grid[((i1 + 0) * ll + j1 + 0) as usize * CD_B2 + k1 as usize + 1] - d1) * d0;
                    let d6 = (density_grid[((i1 + 0) * ll + j1 + 1) as usize * CD_B2 + k1 as usize + 1] - d2) * d0;
                    let d7 = (density_grid[((i1 + 1) * ll + j1 + 0) as usize * CD_B2 + k1 as usize + 1] - d3) * d0;
                    let d8 = (density_grid[((i1 + 1) * ll + j1 + 1) as usize * CD_B2 + k1 as usize + 1] - d4) * d0;

                    for l1 in 0..8i32 {
                        let d9 = 0.25_f64;
                        let mut d10 = dd1;
                        let mut d11 = dd2;
                        let d12 = (dd3 - dd1) * d9;
                        let d13 = (dd4 - dd2) * d9;

                        let world_y_base = k1 * 8 + l1;

                        for i2 in 0..4i32 {
                            // Java: j2 = i2+i1*4<<11 | 0+j1*4<<7 | k1*8+l1
                            // This is the flat byte index for (x=i2+i1*4, z=j1*4, y=k1*8+l1).
                            // We use the same layout: flat[x * 16 * 128 + z * 128 + y]
                            // But Java uses (x<<11|z<<7|y) with short1=128 strides in z.
                            // Equivalent: flat[(i2+i1*4)*16*128 + (j1*4)*128 + y]
                            let short1 = 128i32;
                            let d14 = 0.25_f64;
                            let mut d15 = d10;
                            let d16 = (d11 - d10) * d14;

                            for k2 in 0..4i32 {
                                // Climate temperature at this xz block position
                                let bx = i2 + i1 * 4;
                                let bz = j1 * 4 + k2;
                                let d17 = temp_grid[(bx * 16 + bz) as usize] as f64;

                                let world_y = world_y_base as usize;
                                let mut block = 0u32;

                                if world_y < b1 {
                                    if d17 < 0.5 && world_y >= b1 - 1 {
                                        block = ice_id;
                                    } else {
                                        block = water_id;
                                    }
                                }
                                if d15 > 0.0 {
                                    block = stone_id;
                                }

                                // Java: abyte[j2] = block; j2 += short1 (strides in z)
                                // j2 = (i2+i1*4)<<11 | (j1*4)<<7 | (k1*8+l1) + k2*short1
                                // = (i2+i1*4)*2048 + (j1*4)*128 + (k1*8+l1) + k2*128
                                // = (i2+i1*4)*16*128 + (j1*4+k2)*128 + (k1*8+l1)
                                // Our layout: out[bx * 16 * 128 + bz * 128 + world_y]
                                let flat_idx = (bx as usize) * 16 * 128
                                    + (bz as usize) * 128
                                    + world_y;
                                if flat_idx < out.len() {
                                    out[flat_idx] = block;
                                }

                                d15 += d16;
                            }
                            d10 += d12;
                            d11 += d13;
                        }
                        dd1 += d5;
                        dd2 += d6;
                        dd3 += d7;
                        dd4 += d8;
                    }
                }
            }
        }

        out
    }
}
