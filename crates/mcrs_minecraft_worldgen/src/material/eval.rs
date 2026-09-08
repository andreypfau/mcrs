use crate::cell::CELL_BOUNDS_SLACK;
use crate::interval::Interval;
use crate::jmath::mth_floor;
use crate::material::compile::{
    CondId, Condition, MaterialProgram, NoiseId, Op, Scope, SurfaceNoise, Tri, VeinId,
};
use crate::program::{NodeId, Workspace};
use crate::router::{CHUNK_SURFACE_LEVEL, NoiseRouter};
use crate::volume::Volume;
use bevy_math::IVec3;
use mcrs_minecraft_random::Random;
use mcrs_voxel_storage::VoxelId;

/// The water level of a strip in which no fluid has been seen from above.
pub const NO_WATER: i32 = i32::MIN;

/// Every allocation the descent reuses column after column.
#[derive(Default)]
pub struct MaterialScratch {
    bypass_shortcuts: bool,
    workspace: Workspace,
    condition_stamp: Vec<u32>,
    condition_value: Vec<bool>,
    folded: Vec<Option<bool>>,
    noise_stamp: Vec<u32>,
    noise_value: Vec<f64>,
    preliminary: Vec<f32>,
    /// One row per vein over the column's block volume, written only inside
    /// the cells `vein_cells` leaves open.
    vein_values: Vec<f32>,
    /// Per vein, per cell of its lattice: whether the vein's density can be
    /// positive anywhere in the cell. A settled cell never reads `vein_values`.
    vein_cells: Vec<bool>,
    vein_layout: Vec<VeinCells>,
    /// Per vein, where the current strip's cells start in `vein_cells`.
    vein_strip: Vec<usize>,
    lattice: Vec<f32>,
    corners: Vec<Interval>,
    /// One bound per term of a vein's [`crate::cell::CellBounds`], reused
    /// across every cell of the column.
    cell_terms: Vec<Interval>,
    cell_density: Vec<f32>,
    /// The conditions whose answer changes with y alone, at a height a strip
    /// knows before its descent: they split a strip into runs.
    splits: Vec<CondId>,
    breaks: Vec<i32>,
    /// Every biome the zoom can select for the current strip over the y range
    /// `reachable_lo..=reachable_hi`, or wider.
    reachable: Vec<u32>,
}

/// Runs shorter than this are walked block by block rather than settled.
const SHORT_RUN: i32 = 8;

/// Where settling a piece of a run stopped: on an answer, on a guard that could
/// still go either way, or on a rule no piece can settle at all.
enum Settled {
    Answer(Option<VoxelId>),
    Open {
        condition: CondId,
        on_true: usize,
        on_false: usize,
    },
    Blocked,
}

/// A condition's answer along one solid run of a strip.
enum Split {
    Always(bool),
    /// True at and above this y.
    At(i32),
    /// True at and below this y.
    AtOrBelow(i32),
}

/// How one vein's cells are laid out in `MaterialScratch::vein_cells`: the
/// column's cells in x, then z, each a run of `size.y` along y.
#[derive(Clone, Copy)]
struct VeinCells {
    cell: IVec3,
    size: IVec3,
    at: usize,
}

impl MaterialScratch {
    /// Force every cache to miss and report no settled run, so every block
    /// goes through the tape and can be compared against a memoised descent.
    pub fn bypass_shortcuts(&mut self, bypass: bool) {
        self.bypass_shortcuts = bypass;
    }
}

/// The context the tape tests against, over one column.
///
/// `biome_at` answers the biome of a block through the zoom, which needs a grid
/// this crate cannot build; everything else here is a function of the program,
/// the router and the position.
pub struct MaterialEval<'a, B, R> {
    router: &'a NoiseRouter,
    pub(crate) program: &'a MaterialProgram,
    scratch: &'a mut MaterialScratch,
    biome_at: B,
    reachable_at: R,
    reachable_lo: i32,
    reachable_hi: i32,
    reachable_stamp: u32,
    reachable_ok: bool,
    memoise: bool,
    preliminary: Volume,
    veins: Volume,
    /// Where the current strip starts in a `vein_values` row.
    vein_row: usize,
    run_top: i32,
    run_bottom: i32,
    run_depth: i32,
    gen_xz: u32,
    gen_y: u32,
    pub(crate) block_x: i32,
    pub(crate) block_z: i32,
    pub(crate) gradient_x: i32,
    pub(crate) gradient_z: i32,
    pub(crate) surface_depth: i32,
    surface_secondary: f64,
    surface_secondary_stamp: u32,
    min_surface_level: i32,
    min_surface_level_stamp: u32,
    pub(crate) block_y: i32,
    pub(crate) depth_above: i32,
    pub(crate) depth_below: i32,
    pub(crate) water_level: i32,
    biome: u32,
    biome_stamp: u32,
}

impl<'a, B, R> MaterialEval<'a, B, R>
where
    B: FnMut(i32, i32, i32) -> u32,
    R: FnMut(i32, i32, i32, i32, &mut Vec<u32>) -> bool,
{
    /// `None` for a router built without material rules.
    ///
    /// `top` is the highest non-air block of the column and `biomes` every biome
    /// the column can select, which must include the border ring of the grid the
    /// zoom reads: a set narrower than the zoom's reach folds a condition that
    /// should have matched to `never` and writes the wrong block.
    pub fn new(
        router: &'a NoiseRouter,
        scratch: &'a mut MaterialScratch,
        biome_at: B,
        reachable_at: R,
        block_x: i32,
        block_z: i32,
        top: i32,
        biomes: &[u32],
    ) -> Option<Self> {
        let program = router.material()?;
        let min_y = router.noise.min_y;
        let memoise = !scratch.bypass_shortcuts;

        scratch.condition_stamp.clear();
        scratch
            .condition_stamp
            .resize(program.conditions().len(), 0);
        scratch.condition_value.clear();
        scratch
            .condition_value
            .resize(program.conditions().len(), false);
        scratch.noise_stamp.clear();
        scratch.noise_stamp.resize(program.noise_count(), 0);
        scratch.noise_value.clear();
        scratch.noise_value.resize(program.noise_count(), 0.0);

        scratch.splits.clear();
        for (id, condition) in program.conditions().iter().enumerate() {
            if matches!(
                condition.kind,
                Condition::VerticalGradient { .. }
                    | Condition::AbovePreliminarySurface
                    | Condition::YAbove { .. }
                    | Condition::Water { .. }
                    | Condition::StoneDepth { .. }
            ) {
                scratch.splits.push(id as CondId);
            }
        }

        scratch.folded.clear();
        for condition in program.conditions() {
            let folded = match condition.kind {
                Condition::Biome { set } => {
                    match program.biome_sets()[set as usize].fold(biomes.iter().copied()) {
                        Tri::Never => Some(false),
                        Tri::Always => Some(true),
                        Tri::Maybe => None,
                    }
                }
                // An operand is interned before the `not` that names it, so its
                // own fold is already settled here.
                Condition::Not(inner) => scratch.folded[inner as usize].map(|value| !value),
                _ => None,
            };
            scratch.folded.push(folded);
        }

        let preliminary = Volume::dense(IVec3::new(16, 1, 16), IVec3::new(block_x, 0, block_z));
        scratch.preliminary.clear();
        scratch.preliminary.resize(preliminary.len(), 0.0);
        router.program.fill(
            &mut scratch.workspace,
            &preliminary,
            CHUNK_SURFACE_LEVEL,
            &mut scratch.preliminary,
        );

        let veins = Volume::dense(
            IVec3::new(16, (top - min_y + 1).max(1), 16),
            IVec3::new(block_x, min_y, block_z),
        );
        prefill_veins(router, program, scratch, &veins);

        Some(Self {
            router,
            program,
            scratch,
            biome_at,
            reachable_at,
            reachable_lo: 0,
            reachable_hi: 0,
            reachable_stamp: 0,
            reachable_ok: false,
            memoise,
            preliminary,
            veins,
            vein_row: 0,
            run_top: 0,
            run_bottom: 0,
            run_depth: 0,
            gen_xz: 0,
            gen_y: 0,
            block_x,
            block_z,
            gradient_x: 0,
            gradient_z: 0,
            surface_depth: 0,
            surface_secondary: 0.0,
            surface_secondary_stamp: 0,
            min_surface_level: 0,
            min_surface_level_stamp: 0,
            block_y: 0,
            depth_above: 0,
            depth_below: 0,
            water_level: NO_WATER,
            biome: 0,
            biome_stamp: 0,
        })
    }

    pub fn begin_strip(&mut self, block_x: i32, block_z: i32, gradient_x: i32, gradient_z: i32) {
        self.gen_xz += 1;
        self.gen_y += 1;
        self.block_x = block_x;
        self.block_z = block_z;
        self.gradient_x = gradient_x;
        self.gradient_z = gradient_z;
        self.surface_depth = self.compute_surface_depth();

        let local = IVec3::new(block_x, 0, block_z) - self.veins.min_block();
        self.vein_row = (local.x + local.z * 16) as usize * self.veins.size().y as usize;
        for (strip, layout) in self
            .scratch
            .vein_strip
            .iter_mut()
            .zip(&self.scratch.vein_layout)
        {
            let cell = local / layout.cell;
            *strip =
                layout.at + (cell.x + cell.z * layout.size.x) as usize * layout.size.y as usize;
        }
    }

    /// The runs of y, descending, over which the tape answers the same for
    /// every block — one state, or nothing — so the descent need not run it
    /// there. Asked once per solid run of a strip, from `top`, whose block sits
    /// `depth_above` deep, down to `bottom`, with `water_level` above it: along
    /// such a run the depths are affine in y, so every depth, water and height
    /// condition is a threshold in y, the y-only ones too; the per-strip ones
    /// hold across it, a biome set the column folded is settled, and an ore
    /// vein is silent through a cell its bound closed. The run is split at
    /// every threshold, and any other guard met on the way leaves the piece
    /// open.
    pub fn settled_runs(
        &mut self,
        top: i32,
        bottom: i32,
        depth_above: i32,
        water_level: i32,
        out: &mut Vec<(i32, i32, Option<VoxelId>)>,
    ) {
        out.clear();
        // Splitting a run costs about as much as walking a few blocks of it.
        if self.scratch.bypass_shortcuts || top - bottom < SHORT_RUN {
            return;
        }
        self.run_top = top;
        self.run_bottom = bottom;
        self.run_depth = depth_above;
        self.water_level = water_level;
        let min_y = self.veins.min_block().y;
        let mut breaks = std::mem::take(&mut self.scratch.breaks);
        breaks.clear();
        for k in 0..self.scratch.splits.len() {
            let id = self.scratch.splits[k];
            match self.program.conditions()[id as usize].kind {
                Condition::VerticalGradient {
                    true_at_and_below,
                    false_at_and_above,
                    ..
                } => breaks.extend([true_at_and_below + 1, false_at_and_above]),
                _ => {
                    if let Some(Split::At(at)) | Some(Split::AtOrBelow(at)) = self.split(id) {
                        breaks.extend([at, at + 1]);
                    }
                }
            }
        }
        for (vein, layout) in self.scratch.vein_layout.iter().enumerate() {
            let at = self.scratch.vein_strip[vein];
            for cy in 0..layout.size.y {
                if self.scratch.vein_cells[at + cy as usize] {
                    let cell_bottom = min_y + cy * layout.cell.y;
                    breaks.extend([cell_bottom, cell_bottom + layout.cell.y]);
                }
            }
        }
        breaks.retain(|&y| y > bottom && y <= top);
        breaks.sort_unstable_by(|a, b| b.cmp(a));
        breaks.dedup();

        let mut hi = top;
        for &lo in &breaks {
            self.settle_piece(lo, hi, out);
            hi = lo - 1;
        }
        if hi >= bottom {
            self.settle_piece(bottom, hi, out);
        }
        self.scratch.breaks = breaks;
    }

    /// What the piece `lo..=hi` answers, appended to `out`.
    ///
    /// A piece the tape leaves open on a vertical gradient is still worth
    /// settling: inside the band the draw is the only thing that varies, so if
    /// both sides of the guard settle, each y is one draw against the band's
    /// probability instead of a walk of the whole tape. The draw is a fork of
    /// the stream at the block, so making it here rather than during the
    /// descent asks the same stream the same question.
    fn settle_piece(&mut self, lo: i32, hi: i32, out: &mut Vec<(i32, i32, Option<VoxelId>)>) {
        let (condition, on_true, on_false) = match self.settle_from(0, lo, hi) {
            Settled::Answer(state) => return out.push((lo, hi, state)),
            Settled::Blocked => return,
            Settled::Open {
                condition,
                on_true,
                on_false,
            } => (condition, on_true, on_false),
        };
        let Condition::VerticalGradient {
            random,
            true_at_and_below,
            false_at_and_above,
        } = self.program.conditions()[condition as usize].kind
        else {
            return;
        };
        if lo <= true_at_and_below || hi >= false_at_and_above {
            return;
        }
        let (Settled::Answer(whenever), Settled::Answer(otherwise)) = (
            self.settle_from(on_true, lo, hi),
            self.settle_from(on_false, lo, hi),
        ) else {
            return;
        };
        if whenever == otherwise {
            return out.push((lo, hi, whenever));
        }
        for y in (lo..=hi).rev() {
            let probability = map(
                f64::from(y),
                f64::from(true_at_and_below),
                f64::from(false_at_and_above),
                1.0,
                0.0,
            );
            let mut draw = self
                .program
                .random_at(random, IVec3::new(self.block_x, y, self.block_z));
            let state = if f64::from(draw.next_f32()) < probability {
                whenever
            } else {
                otherwise
            };
            out.push((y, y, state));
        }
    }

    /// What the tape returns for every y in `lo..=hi` of the run, if it is the
    /// same block, or the same nothing, throughout.
    fn settle_from(&mut self, mut pc: usize, lo: i32, hi: i32) -> Settled {
        let program = self.program;
        while let Some(op) = program.tape().get(pc) {
            match *op {
                Op::Guard { condition, skip_to } => match self.over(condition, lo, hi) {
                    Some(true) => pc += 1,
                    Some(false) => pc = skip_to as usize,
                    None => {
                        return Settled::Open {
                            condition,
                            on_true: pc + 1,
                            on_false: skip_to as usize,
                        };
                    }
                },
                Op::Block { state } => return Settled::Answer(Some(state)),
                Op::Bandlands => return Settled::Blocked,
                Op::OreVein { vein } => {
                    if self.vein_open_in(vein as usize, lo, hi) {
                        return Settled::Blocked;
                    }
                    pc += 1;
                }
            }
        }
        Settled::Answer(None)
    }

    /// The condition's answer if it is the same for every y in `lo..=hi`.
    fn over(&mut self, condition: CondId, lo: i32, hi: i32) -> Option<bool> {
        if let Some(value) = self.scratch.folded[condition as usize] {
            return Some(value);
        }
        let compiled = &self.program.conditions()[condition as usize];
        match compiled.kind {
            Condition::VerticalGradient {
                true_at_and_below,
                false_at_and_above,
                ..
            } => {
                if hi <= true_at_and_below {
                    Some(true)
                } else if lo >= false_at_and_above {
                    Some(false)
                } else {
                    None
                }
            }
            Condition::Not(inner) => self.over(inner, lo, hi).map(|value| !value),
            // The column-wide fold above covers the whole grid, ring included,
            // so any column near a border loses it. Over one strip and one run
            // the zoom can only reach the four corner quart columns across the
            // run's cells, which is a far tighter set to fold.
            Condition::Biome { set } => {
                if !self.reachable_over(lo, hi) {
                    return None;
                }
                match self.program.biome_sets()[set as usize]
                    .fold(self.scratch.reachable.iter().copied())
                {
                    Tri::Never => Some(false),
                    Tri::Always => Some(true),
                    Tri::Maybe => None,
                }
            }
            _ => match self.split(condition) {
                Some(Split::Always(value)) => Some(value),
                Some(Split::At(at)) if lo >= at => Some(true),
                Some(Split::At(at)) if hi < at => Some(false),
                Some(Split::AtOrBelow(at)) if hi <= at => Some(true),
                Some(Split::AtOrBelow(at)) if lo > at => Some(false),
                Some(_) => None,
                None if compiled.scope == Scope::Xz => Some(self.test(condition)),
                None => None,
            },
        }
    }

    /// Fills `scratch.reachable` with a superset of the biomes the zoom can
    /// select over `lo..=hi` of the current strip. `false` where the caller
    /// cannot say, which leaves every biome condition open.
    fn reachable_over(&mut self, lo: i32, hi: i32) -> bool {
        if self.memoise
            && self.reachable_stamp == self.gen_xz
            && self.reachable_lo == lo
            && self.reachable_hi == hi
        {
            return self.reachable_ok;
        }
        let mut reachable = std::mem::take(&mut self.scratch.reachable);
        self.reachable_ok = (self.reachable_at)(self.block_x, self.block_z, lo, hi, &mut reachable);
        self.scratch.reachable = reachable;
        self.reachable_stamp = self.gen_xz;
        self.reachable_lo = lo;
        self.reachable_hi = hi;
        self.reachable_ok
    }

    /// How a condition answers along the current run, where the depth above
    /// is `run_depth + run_top - y` and the depth below `y - run_bottom + 1`.
    fn split(&mut self, condition: CondId) -> Option<Split> {
        let (bottom, water) = (self.run_bottom, self.water_level);
        // What `y + depth_above` comes to anywhere on the run.
        let top = self.run_top + self.run_depth - 1;
        Some(match self.program.conditions()[condition as usize].kind {
            Condition::StoneDepth {
                offset,
                add_surface_depth,
                secondary_depth_range,
                ceiling,
            } => {
                let limit =
                    self.stone_depth_limit(offset, add_surface_depth, secondary_depth_range);
                if ceiling {
                    Split::AtOrBelow(bottom + limit - 1)
                } else {
                    Split::At(top + 1 - limit)
                }
            }
            Condition::Water {
                offset,
                surface_depth_multiplier,
                add_stone_depth,
            } => {
                if water == NO_WATER {
                    Split::Always(true)
                } else {
                    let at = water + offset + self.surface_depth * surface_depth_multiplier;
                    if add_stone_depth {
                        Split::Always(top + 1 >= at)
                    } else {
                        Split::At(at)
                    }
                }
            }
            Condition::YAbove {
                anchor,
                surface_depth_multiplier,
                add_stone_depth,
            } => {
                let at = anchor + self.surface_depth * surface_depth_multiplier;
                if add_stone_depth {
                    Split::Always(top + 1 >= at)
                } else {
                    Split::At(at)
                }
            }
            Condition::AbovePreliminarySurface => Split::At(self.min_surface_level()),
            _ => return None,
        })
    }

    /// The depth a stone-depth condition admits.
    fn stone_depth_limit(
        &mut self,
        offset: i32,
        add_surface_depth: bool,
        secondary_depth_range: i32,
    ) -> i32 {
        let surface_depth = if add_surface_depth {
            self.surface_depth
        } else {
            0
        };
        let secondary = if secondary_depth_range == 0 {
            0
        } else {
            map(
                self.surface_secondary(),
                -1.0,
                1.0,
                0.0,
                f64::from(secondary_depth_range),
            ) as i32
        };
        1 + offset + surface_depth + secondary
    }

    /// Whether any cell of the vein's lattice meeting `lo..=hi` in this strip
    /// is open, or the run reaches above the lattice.
    fn vein_open_in(&self, vein: usize, lo: i32, hi: i32) -> bool {
        let layout = self.scratch.vein_layout[vein];
        let min_y = self.veins.min_block().y;
        let first = (lo - min_y) / layout.cell.y;
        let last = (hi - min_y) / layout.cell.y;
        if last >= layout.size.y {
            return true;
        }
        let at = self.scratch.vein_strip[vein];
        self.scratch.vein_cells[at + first as usize..=at + last as usize]
            .iter()
            .any(|&open| open)
    }

    pub fn update_y(&mut self, depth_above: i32, depth_below: i32, water_level: i32, block_y: i32) {
        self.gen_y += 1;
        self.block_y = block_y;
        self.water_level = water_level;
        self.depth_above = depth_above;
        self.depth_below = depth_below;
    }

    /// The block the rules produce at the current position: the tape walked from
    /// the top, each guard answered against this position.
    pub fn apply(&mut self) -> Option<VoxelId> {
        let mut pc = 0usize;
        while let Some(op) = self.program.tape().get(pc) {
            match *op {
                Op::Guard { condition, skip_to } => {
                    if self.test(condition) {
                        pc += 1;
                    } else {
                        pc = skip_to as usize;
                    }
                }
                Op::Block { state } => return Some(state),
                Op::Bandlands => return Some(self.bandlands()),
                Op::OreVein { vein } => {
                    if let Some(state) = self.ore_vein(vein) {
                        return Some(state);
                    }
                    pc += 1;
                }
            }
        }
        None
    }

    pub fn min_surface_level(&mut self) -> i32 {
        if self.memoise && self.min_surface_level_stamp == self.gen_xz {
            return self.min_surface_level;
        }
        self.min_surface_level_stamp = self.gen_xz;
        let level = match self
            .preliminary
            .index_of_block(self.block_x, 0, self.block_z)
        {
            Some(index) => self.scratch.preliminary[index],
            None => self.sample(
                CHUNK_SURFACE_LEVEL,
                IVec3::new(self.block_x, 0, self.block_z),
            ),
        };
        self.min_surface_level = mth_floor(level) + self.surface_depth - 8;
        self.min_surface_level
    }

    fn compute_surface_depth(&mut self) -> i32 {
        let noise = self.noise(self.program.surface_noise(SurfaceNoise::Surface), false);
        let mut random = self
            .program
            .noise_random_at(IVec3::new(self.block_x, 0, self.block_z));
        (noise * 2.75 + 3.0 + random.next_f64() * 0.25) as i32
    }

    pub(crate) fn surface_secondary(&mut self) -> f64 {
        if self.memoise && self.surface_secondary_stamp == self.gen_xz {
            return self.surface_secondary;
        }
        self.surface_secondary_stamp = self.gen_xz;
        self.surface_secondary = self.noise(
            self.program.surface_noise(SurfaceNoise::SurfaceSecondary),
            false,
        );
        self.surface_secondary
    }

    pub(crate) fn biome(&mut self) -> u32 {
        if self.memoise && self.biome_stamp == self.gen_y {
            return self.biome;
        }
        self.biome_stamp = self.gen_y;
        self.biome = (self.biome_at)(self.block_x, self.block_y, self.block_z);
        self.biome
    }

    pub(crate) fn noise(&mut self, noise: NoiseId, is_3d: bool) -> f64 {
        let stamp = if is_3d { self.gen_y } else { self.gen_xz };
        if self.memoise && self.scratch.noise_stamp[noise as usize] == stamp {
            return self.scratch.noise_value[noise as usize];
        }
        let y = if is_3d { f64::from(self.block_y) } else { 0.0 };
        let value = f64::from(self.program.noise(noise).get(
            f64::from(self.block_x),
            y,
            f64::from(self.block_z),
        ));
        self.scratch.noise_stamp[noise as usize] = stamp;
        self.scratch.noise_value[noise as usize] = value;
        value
    }

    fn sample(&mut self, root: usize, pos: IVec3) -> f32 {
        let mut out = [0.0f32];
        self.router.program.fill(
            &mut self.scratch.workspace,
            &Volume::point(pos),
            root,
            &mut out,
        );
        out[0]
    }

    /// A vein's density prefilled over the column, point-sampled where the
    /// position sits above the fill — which a pillar built before the descent
    /// can.
    fn prefilled(&mut self, vein: usize, root: usize) -> f32 {
        let dy = self.block_y - self.veins.min_block().y;
        if (0..self.veins.size().y).contains(&dy) {
            self.scratch.vein_values[vein * self.veins.len() + self.vein_row + dy as usize]
        } else {
            self.sample(root, IVec3::new(self.block_x, self.block_y, self.block_z))
        }
    }

    /// Whether the vein's density can be positive in the cell holding the
    /// position; above the classified lattice nothing is known.
    fn vein_possible(&self, vein: usize) -> bool {
        let layout = self.scratch.vein_layout[vein];
        let cy = (self.block_y - self.veins.min_block().y) / layout.cell.y;
        cy >= layout.size.y || self.scratch.vein_cells[self.scratch.vein_strip[vein] + cy as usize]
    }

    fn compute(&mut self, condition: CondId) -> bool {
        let program = self.program;
        match program.conditions()[condition as usize].kind {
            Condition::StoneDepth {
                offset,
                add_surface_depth,
                secondary_depth_range,
                ceiling,
            } => {
                let depth = if ceiling {
                    self.depth_below
                } else {
                    self.depth_above
                };
                depth <= self.stone_depth_limit(offset, add_surface_depth, secondary_depth_range)
            }
            Condition::Water {
                offset,
                surface_depth_multiplier,
                add_stone_depth,
            } => {
                self.water_level == NO_WATER
                    || self.block_y + if add_stone_depth { self.depth_above } else { 0 }
                        >= self.water_level + offset + self.surface_depth * surface_depth_multiplier
            }
            Condition::YAbove {
                anchor,
                surface_depth_multiplier,
                add_stone_depth,
            } => {
                self.block_y + if add_stone_depth { self.depth_above } else { 0 }
                    >= anchor + self.surface_depth * surface_depth_multiplier
            }
            Condition::Biome { set } => {
                let biome = self.biome();
                program.biome_sets()[set as usize].contains(biome)
            }
            Condition::NoiseThreshold {
                noise,
                min,
                max,
                is_3d,
            } => {
                let value = self.noise(noise, is_3d);
                value >= min.0 && value <= max.0
            }
            Condition::VerticalGradient {
                random,
                true_at_and_below,
                false_at_and_above,
            } => {
                if self.block_y <= true_at_and_below {
                    true
                } else if self.block_y >= false_at_and_above {
                    false
                } else {
                    let probability = map(
                        f64::from(self.block_y),
                        f64::from(true_at_and_below),
                        f64::from(false_at_and_above),
                        1.0,
                        0.0,
                    );
                    let mut random = program
                        .random_at(random, IVec3::new(self.block_x, self.block_y, self.block_z));
                    f64::from(random.next_f32()) < probability
                }
            }
            Condition::Steep => self.gradient_x <= -4 || self.gradient_z >= 4,
            Condition::Hole => self.surface_depth <= 0,
            Condition::AbovePreliminarySurface => self.block_y >= self.min_surface_level(),
            Condition::Not(inner) => !self.test(inner),
        }
    }
}

#[cfg(test)]
impl<B, R> MaterialEval<'_, B, R> {
    /// Reaches the depths the surface noise would only reach at rare positions.
    /// The stamp goes back to zero because `begin_strip` never leaves `gen_xz`
    /// there, so the preliminary level recomputes against the new depth.
    pub(crate) fn set_surface_depth(&mut self, depth: i32) {
        self.surface_depth = depth;
        self.min_surface_level_stamp = 0;
    }
}

impl<B, R> MaterialEval<'_, B, R>
where
    B: FnMut(i32, i32, i32) -> u32,
    R: FnMut(i32, i32, i32, i32, &mut Vec<u32>) -> bool,
{
    /// Answers one guard, memoised against the scope its condition was compiled
    /// with, so a condition shared by dozens of rules is computed once per strip
    /// or per y.
    pub(crate) fn test(&mut self, condition: CondId) -> bool {
        if !self.memoise {
            return self.compute(condition);
        }
        if let Some(value) = self.scratch.folded[condition as usize] {
            return value;
        }
        let stamp = match self.program.conditions()[condition as usize].scope {
            Scope::Xz => self.gen_xz,
            Scope::Y => self.gen_y,
        };
        if self.scratch.condition_stamp[condition as usize] == stamp {
            return self.scratch.condition_value[condition as usize];
        }
        let value = self.compute(condition);
        self.scratch.condition_stamp[condition as usize] = stamp;
        self.scratch.condition_value[condition as usize] = value;
        value
    }

    pub(crate) fn bandlands(&mut self) -> VoxelId {
        let noise = self.noise(
            self.program.surface_noise(SurfaceNoise::ClayBandsOffset),
            false,
        );
        // Rounding here is a floor of the half-shifted value, not Rust's
        // away-from-zero `round`: the two disagree on every negative half and
        // shift the whole band table.
        let offset = ((noise as f32 * 4.0) + 0.5).floor() as i32;
        let bands = self.program.clay_bands();
        bands[(self.block_y + offset).rem_euclid(bands.len() as i32) as usize]
    }

    pub(crate) fn ore_vein(&mut self, vein: VeinId) -> Option<VoxelId> {
        if !self.vein_possible(vein as usize) {
            return None;
        }
        let ore_vein = self.program.veins()[vein as usize];
        let density = self.prefilled(vein as usize, ore_vein.density);
        if density <= 0.0 {
            return None;
        }
        let at = IVec3::new(self.block_x, self.block_y, self.block_z);
        let mut random = self.program.random_at(ore_vein.random, at);
        if random.next_f32() > density {
            return None;
        }
        // Richness and the gap are read only inside a vein, a handful of
        // blocks per column, so neither is worth a prefill.
        let richness = self.sample(ore_vein.richness, at);
        if random.next_f32() < richness && self.sample(ore_vein.filler_gap, at) < 0.0 {
            Some(if random.next_f32() < ore_vein.raw_ore_chance {
                ore_vein.raw_ore
            } else {
                ore_vein.ore
            })
        } else {
            Some(ore_vein.filler)
        }
    }
}

/// Classify every cell of each vein's lattice from interval bounds over its
/// corners, and fill the density block by block only where a bound leaves the
/// sign open. Veins are rare, so nearly every cell settles as "no vein" from
/// its eight corners, and the descent then answers the rule without a read.
///
/// A vein whose wrappers do not share one lattice tiling the column has no
/// cells to settle and is filled whole.
fn prefill_veins(
    router: &NoiseRouter,
    program: &MaterialProgram,
    scratch: &mut MaterialScratch,
    veins: &Volume,
) {
    let points = veins.len();
    let min = veins.min_block();
    let height = router.noise.height as i32;
    scratch.vein_values.clear();
    scratch
        .vein_values
        .resize(program.veins().len() * points, 0.0);
    scratch.vein_cells.clear();
    scratch.vein_layout.clear();
    scratch.vein_strip.clear();
    let mut filled: Option<(&[NodeId], Volume)> = None;

    for (index, vein) in program.veins().iter().enumerate() {
        let row = &mut scratch.vein_values[index * points..(index + 1) * points];
        let bounds = router.vein_cell_bounds(index);
        let cell = bounds
            .cell_size()
            .filter(|c| 16 % c.x == 0 && 16 % c.z == 0 && height % c.y == 0);
        let at = scratch.vein_cells.len();
        scratch.vein_strip.push(at);
        let Some(cell) = cell else {
            scratch.vein_layout.push(VeinCells {
                cell: IVec3::new(16, height, 16),
                size: IVec3::ONE,
                at,
            });
            scratch.vein_cells.push(true);
            router
                .program
                .fill(&mut scratch.workspace, veins, vein.density, row);
            continue;
        };

        let size = IVec3::new(16 / cell.x, height / cell.y, 16 / cell.z);
        scratch.vein_layout.push(VeinCells { cell, size, at });
        let lattice = Volume::new(size + IVec3::ONE, min, cell);
        let inputs = bounds.inputs();
        // The veins of one dimension interpolate the same noises, so the
        // lattice sampled for the first serves the rest.
        if filled != Some((inputs, lattice)) {
            scratch.lattice.clear();
            scratch.lattice.resize(inputs.len() * lattice.len(), 0.0);
            router.fill_nodes(
                &mut scratch.workspace,
                &lattice,
                inputs,
                &mut scratch.lattice,
            );
            router.pin_lattice_of(bounds, &mut scratch.workspace, &lattice, &scratch.lattice);
            filled = Some((inputs, lattice));
        }
        scratch.corners.resize(inputs.len(), Interval::exact(0.0));

        for cz in 0..size.z {
            for cx in 0..size.x {
                for cy in 0..size.y {
                    let cell_min = min + IVec3::new(cx, cy, cz) * cell;
                    let cell_max = cell_min + cell - 1;
                    // Cells above the column's top are left open: the descent
                    // reaches them only under a badlands pillar, and samples
                    // the point there.
                    let settled = cell_min.y <= veins.max_block().y && {
                        corner_bounds(
                            &scratch.lattice,
                            &lattice,
                            IVec3::new(cx, cy, cz),
                            &mut scratch.corners,
                        );
                        bounds
                            .eval(
                                &router.program,
                                &scratch.corners,
                                cell_min,
                                cell_max,
                                &mut scratch.cell_terms,
                            )
                            .is_some_and(|bound| bound.max() < -CELL_BOUNDS_SLACK)
                    };
                    scratch.vein_cells.push(!settled);
                    if settled {
                        continue;
                    }
                    // Only the part of the cell the column volume covers is
                    // prefilled; above it the rule samples the point.
                    let rows = (veins.max_block().y - cell_min.y + 1).min(cell.y);
                    if rows <= 0 {
                        continue;
                    }
                    let dense = Volume::dense(IVec3::new(cell.x, rows, cell.z), cell_min);
                    scratch.cell_density.clear();
                    scratch.cell_density.resize(dense.len(), 0.0);
                    router.program.fill(
                        &mut scratch.workspace,
                        &dense,
                        vein.density,
                        &mut scratch.cell_density,
                    );
                    for dz in 0..cell.z {
                        for dx in 0..cell.x {
                            let from = dense.index_unchecked(dx, 0, dz);
                            let to = veins.index_unchecked(
                                cx * cell.x + dx,
                                cell_min.y - min.y,
                                cz * cell.z + dz,
                            );
                            row[to..to + rows as usize]
                                .copy_from_slice(&scratch.cell_density[from..from + rows as usize]);
                        }
                    }
                }
            }
        }
    }
}

/// The hull of one cell's eight corner values, per lattice row.
fn corner_bounds(values: &[f32], lattice: &Volume, at: IVec3, out: &mut [Interval]) {
    let stride = lattice.len();
    for (k, bound) in out.iter_mut().enumerate() {
        let row = &values[k * stride..(k + 1) * stride];
        let (mut lo, mut hi) = (f32::INFINITY, f32::NEG_INFINITY);
        for dz in 0..2 {
            for dx in 0..2 {
                for dy in 0..2 {
                    let v = row[lattice.index_unchecked(at.x + dx, at.y + dy, at.z + dz)];
                    lo = lo.min(v);
                    hi = hi.max(v);
                }
            }
        }
        *bound = Interval::of(lo, hi);
    }
}

/// A lerp over an inverse lerp, unclamped: a value outside the source range
/// maps outside the target range.
pub(crate) fn map(value: f64, from_min: f64, from_max: f64, to_min: f64, to_max: f64) -> f64 {
    to_min + (value - from_min) / (from_max - from_min) * (to_max - to_min)
}
