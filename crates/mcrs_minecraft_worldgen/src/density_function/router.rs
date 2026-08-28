use super::*;

pub struct NoiseRouter {
    pub(super) temperature_index: usize,
    pub(super) vegetation_index: usize,
    pub(super) continents_index: usize,
    pub(super) erosion_index: usize,
    pub(super) depth_index: usize,
    pub(super) ridges_index: usize,
    pub(super) chunk_surface_level_index: usize,
    pub(super) final_density_index: usize,
    pub(super) noise_min_y: i32,
    pub(super) noise_height: u32,
    pub(super) sea_level: i32,
    pub(super) default_block_state: VoxelId,
    pub(super) default_fluid_state: VoxelId,
    pub(super) world_seed: u64,
    /// Beta beach octave noise (4 octaves, stream position 4 in seed_beta_terrain).
    /// None for the modern router. Used by apply_beta_surface to determine beach columns.
    pub(super) beta_beach_noise: Option<Box<OctavePerlinNoise<f64>>>,
    /// Beta surface octave noise (4 octaves, stream position 5 in seed_beta_terrain).
    /// None for the modern router. Used by apply_beta_surface to determine surface depth.
    pub(super) beta_surface_noise: Option<Box<OctavePerlinNoise<f64>>>,
    /// f64-precision Beta terrain density noises for exact Java parity.
    /// None for the modern router. Replaces the f32 density-function tree for the Beta path.
    pub(super) beta_terrain_f64: Option<Box<beta_terrain_f64::BetaTerrainF64>>,
    /// `column_ready[i]` means entry i and everything it reads hold for a whole
    /// column, so the column pass can evaluate it and every fill of that column
    /// can reuse the result.
    pub(super) column_ready: Box<[bool]>,
    /// Terms of `final_density` at or above every `interpolated` wrapper, ascending.
    pub(super) outer_terms: Box<[usize]>,
    /// The `interpolated` wrappers `final_density` reads, ascending.
    pub(super) outer_wrappers: Box<[usize]>,
    /// Stack index of each outer wrapper's input, in the same order.
    pub(super) outer_wrapper_inputs: Box<[usize]>,
    /// First index of Zone B (per-Y entries for final_density).
    /// Zone A [0..column_boundary): column-only entries reachable from final_density.
    pub(super) column_boundary: usize,
    /// First index of Zone C (entries not reachable from final_density).
    /// Zone B [column_boundary..fd_boundary): per-Y entries for final_density.
    pub(super) fd_boundary: usize,
    /// The one cell size in blocks (typically 4 x 8 x 4) every `interpolated`
    /// wrapper `final_density` reads agrees on, or `None` when they disagree.
    pub(super) cell_size: Option<IVec3>,
    pub(super) stack: Box<[DensityFunctionComponent]>,
    pub(super) node_labels: Box<[String]>,
    pub(super) zone_b_schedule: BranchSchedule,
    pub(super) zone_b_roots: Box<[usize]>,
}

impl NoiseRouter {
    /// Per stack entry, the mask of `AXIS_X` / `AXIS_Y` / `AXIS_Z` its value may
    /// vary along.
    pub fn domain_axes(&self) -> Vec<u8> {
        compile::compute_domain_axes(&self.stack)
    }

    /// All noise router entries as (name, index) pairs.
    pub fn roots(&self) -> Vec<(&'static str, usize)> {
        vec![
            ("temperature", self.temperature_index),
            ("vegetation", self.vegetation_index),
            ("continents", self.continents_index),
            ("erosion", self.erosion_index),
            ("depth", self.depth_index),
            ("ridges", self.ridges_index),
            ("chunk_surface_level", self.chunk_surface_level_index),
            ("final_density", self.final_density_index),
        ]
    }

    pub fn world_seed(&self) -> u64 {
        self.world_seed
    }

    pub fn temperature_index(&self) -> usize {
        self.temperature_index
    }

    pub fn vegetation_index(&self) -> usize {
        self.vegetation_index
    }

    pub fn final_density_index(&self) -> usize {
        self.final_density_index
    }

    pub fn noise_min_y(&self) -> i32 {
        self.noise_min_y
    }

    pub fn noise_height(&self) -> u32 {
        self.noise_height
    }

    pub fn sea_level(&self) -> i32 {
        self.sea_level
    }

    pub fn default_block_state(&self) -> VoxelId {
        self.default_block_state
    }

    pub fn default_fluid_state(&self) -> VoxelId {
        self.default_fluid_state
    }

    /// Return the Beta beach octave noise sampler (4 octaves, stream position 4).
    /// None for the modern router. Used by apply_beta_surface for beach/sand conditions.
    pub fn beta_beach_noise(&self) -> Option<&OctavePerlinNoise<f64>> {
        self.beta_beach_noise.as_deref()
    }

    /// Return the Beta surface octave noise sampler (4 octaves, stream position 5).
    /// None for the modern router. Used by apply_beta_surface for surface depth calculation.
    pub fn beta_surface_noise(&self) -> Option<&OctavePerlinNoise<f64>> {
        self.beta_surface_noise.as_deref()
    }

    /// Return the f64 Beta terrain noises, if this is a Beta router.
    /// None for the modern overworld router.
    pub fn beta_terrain_f64(&self) -> Option<&beta_terrain_f64::BetaTerrainF64> {
        self.beta_terrain_f64.as_deref()
    }

    /// Sample a 16×16 temperature grid (index = x*16+z) and a 16×16 rain/vegetation grid
    /// for the Beta `computeDensity` call. Both grids are in block coordinates starting at
    /// `(block_x, block_z)`.
    pub fn sample_beta_climate_grids(
        &self,
        block_x: i32,
        block_z: i32,
    ) -> ([f32; 256], [f32; 256]) {
        let mut temp_grid = [0.0f32; 256];
        let mut rain_grid = [0.0f32; 256];
        let volume = Volume::new(
            IVec3::new(16, 1, 16),
            IVec3::new(block_x, 0, block_z),
            IVec3::ONE,
        );
        let mut values = vec![0.0f32; 2 * volume.len()];
        self.sample_volume_roots(
            &[self.temperature_index, self.vegetation_index],
            &volume,
            &mut values,
            &mut FillScratch::new(),
        );
        for x in 0..16i32 {
            for z in 0..16i32 {
                let source = volume.index_unchecked(x, 0, z);
                temp_grid[(x * 16 + z) as usize] = values[source];
                rain_grid[(x * 16 + z) as usize] = values[volume.len() + source];
            }
        }
        (temp_grid, rain_grid)
    }

    /// Evaluate temperature and vegetation at `(block_x, block_z)`.
    pub fn sample_beta_climate(&self, block_x: i32, block_z: i32) -> (f32, f32) {
        let volume = Volume::point(IVec3::new(block_x, 0, block_z));
        let mut values = [0.0f32; 2];
        self.sample_volume_roots(
            &[self.temperature_index, self.vegetation_index],
            &volume,
            &mut values,
            &mut FillScratch::new(),
        );
        (values[0], values[1])
    }

    /// Roots to fill over a cell-corner volume to feed `final_density_cell_bounds`,
    /// in the order that call expects them.
    #[inline]
    pub fn cell_value_roots(&self) -> &[usize] {
        &self.outer_wrapper_inputs
    }

    /// The cell lattice `final_density` interpolates on, or `None` when its
    /// wrappers disagree and there is no single one.
    #[inline]
    pub fn cell_size(&self) -> Option<IVec3> {
        self.cell_size
    }
}

impl NoiseRouter {
    /// Evaluate `root` at one position.
    pub fn sample_value(&self, root: usize, pos: IVec3, scratch: &mut FillScratch) -> f32 {
        let mut out = [0.0f32];
        self.sample_volume(root, &Volume::point(pos), &mut out, scratch);
        out[0]
    }

    /// Evaluate `root` over every position of `volume`, writing `volume.len()`
    /// values into `out` in the volume's own index layout.
    pub fn sample_volume(
        &self,
        root: usize,
        volume: &Volume,
        out: &mut [f32],
        scratch: &mut FillScratch,
    ) {
        self.sample_volume_roots(&[root], volume, out, scratch);
    }

    /// Evaluate several roots over `volume` in one pass, writing one
    /// `volume.len()`-long row per root into `out`, in the order given.
    pub fn sample_volume_roots(
        &self,
        roots: &[usize],
        volume: &Volume,
        out: &mut [f32],
        scratch: &mut FillScratch,
    ) {
        let n = volume.len();
        assert_eq!(
            out.len(),
            roots.len() * n,
            "output length must match the volume"
        );
        let live = roots.iter().copied().max().expect("at least one root") + 1;

        let arena = Arena::new(&self.stack, scratch);
        let pool = &scratch.pool;
        let column_volume = volume.column();

        // An entry whose value holds for a whole column is evaluated on the
        // column volume, so that is the volume deciding which rows it reads.
        let mut needed = pool.bools(live);
        needed.fill(false);
        for &root in roots {
            needed[root] = true;
        }
        for i in (0..live).rev() {
            if !needed[i] {
                continue;
            }
            let at = if self.column_ready[i] {
                &column_volume
            } else {
                volume
            };
            self.stack[i].visit_row_inputs(at, &mut |dep| {
                if dep < live {
                    needed[dep] = true;
                }
            });
        }

        // Rows go to the entries this fill needs and no others, in ascending
        // order so every reader's slot sits above the slots it reads.
        let mut slots = pool.slots(live);
        let mut columns = pool.slots(live);
        let (mut row_count, mut column_count) = (0usize, 0usize);
        for i in 0..live {
            if !needed[i] {
                slots[i] = NO_SLOT;
                continue;
            }
            slots[i] = row_count as u32;
            row_count += 1;
            if self.column_ready[i] {
                columns[column_count] = i as u32;
                column_count += 1;
            }
        }

        let mut rows = pool.floats(row_count * n);
        let mut positions = pool.positions(n);
        volume.positions_into(&mut positions);
        if column_count > 0 {
            arena.spread_column(&columns[..column_count], &slots, volume, &mut rows);
        }

        if live <= self.fd_boundary && roots.iter().all(|r| self.zone_b_roots.contains(r)) {
            self.fill_zone_b_scheduled(arena, volume, &positions, &slots, &mut rows, &needed);
        } else {
            for i in 0..live {
                if needed[i] && !self.column_ready[i] {
                    arena.fill_node(i, volume, &positions, &slots, &mut rows);
                }
            }
        }

        for (k, &root) in roots.iter().enumerate() {
            let base = slots[root] as usize * n;
            out[k * n..(k + 1) * n].copy_from_slice(&rows[base..base + n]);
        }
    }

    /// Zone B over the whole volume, jumping over every arm-exclusive run no
    /// position in the volume can select. The schedule only preserves the Zone B
    /// roots, so it may not drive a fill of anything else.
    fn fill_zone_b_scheduled(
        &self,
        arena: Arena<'_>,
        volume: &Volume,
        positions: &[IVec3],
        slots: &[u32],
        rows: &mut [f32],
        needed: &[bool],
    ) {
        let n = volume.len();
        let sched = &self.zone_b_schedule;
        let mut s = 0usize;
        while s < sched.steps.len() {
            match sched.steps[s] {
                Step::Eval { start, end } => {
                    for &i in &sched.order[start as usize..end as usize] {
                        if i < needed.len() && needed[i] && !self.column_ready[i] {
                            arena.fill_node(i, volume, positions, slots, rows);
                        }
                    }
                    s += 1;
                }
                Step::Guard {
                    input,
                    min_inclusive,
                    max_exclusive,
                    want_in,
                    unguard,
                } => {
                    let input = input as usize;
                    let reachable = input >= needed.len() || !needed[input] || {
                        let base = slots[input] as usize * n;
                        rows[base..base + n]
                            .iter()
                            .any(|&v| (v >= min_inclusive && v < max_exclusive) == want_in)
                    };
                    s = if reachable { s + 1 } else { unguard as usize };
                }
                Step::Unguard => s += 1,
            }
        }
        #[cfg(debug_assertions)]
        self.verify_fill_zone_b(arena, volume, positions, slots, rows, needed);
    }

    /// Re-evaluate the skipped runs and check nothing the caller reads moved.
    #[cfg(debug_assertions)]
    fn verify_fill_zone_b(
        &self,
        arena: Arena<'_>,
        volume: &Volume,
        positions: &[IVec3],
        slots: &[u32],
        rows: &mut [f32],
        needed: &[bool],
    ) {
        let n = volume.len();
        let live = needed.len();
        let roots = || {
            self.zone_b_roots
                .iter()
                .copied()
                .filter(|&r| r < live && needed[r])
        };
        let mut guarded = arena.pool().floats(roots().count() * n);
        for (k, root) in roots().enumerate() {
            let base = slots[root] as usize * n;
            guarded[k * n..(k + 1) * n].copy_from_slice(&rows[base..base + n]);
        }
        for i in self.column_boundary..=self.final_density_index {
            if i < live && needed[i] && !self.column_ready[i] {
                arena.fill_node(i, volume, positions, slots, rows);
            }
        }
        for (k, root) in roots().enumerate() {
            let base = slots[root] as usize * n;
            for p in 0..n {
                assert_eq!(
                    guarded[k * n + p].to_bits(),
                    rows[base + p].to_bits(),
                    "branch skip changed node {root} at {:?}",
                    positions[p]
                );
            }
        }
    }
}
