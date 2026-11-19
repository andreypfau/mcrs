use crate::compile::{
    CLAY_BAND_COUNT, CompiledCondition, CondId, Condition, MaterialProgram, NoiseId, Op, Scope,
    SurfaceNoise, Tri, VeinId,
};
use glam::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_noise::interval::Interval;
use mcrs_minecraft_noise::sample_grid::SampleGrid;
use mcrs_minecraft_numeric::{floor_to_i32, remap};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_worldgen_density::cell::{
    CELL_BOUNDS_SLACK, CellBounds, corner_bounds, sampled_bounds,
};
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_density::router::{CHUNK_SURFACE_LEVEL, NoiseRouter};

pub const NO_WATER: i32 = i32::MIN;

#[derive(Default)]
pub struct MaterialScratch {
    bypass_shortcuts: bool,
    workspace: Workspace,
    conditions: Vec<(u32, bool)>,
    folded: Vec<Option<bool>>,
    noises: Vec<(u32, f64)>,
    preliminary: Vec<f32>,
    vein_values: Vec<f32>,
    vein_cells: Vec<u8>,
    vein_lattice_for: Option<(usize, SampleGrid)>,
    vein_layout: Vec<VeinCells>,
    vein_strip: Vec<usize>,
    lattice: Vec<f32>,
    corners: Vec<Interval>,
    cell_terms: Vec<Interval>,
    cell_density: Vec<f32>,
    splits: Vec<CondId>,
    breaks: Vec<i32>,
    reachable: Vec<u16>,
}

const SHORT_RUN: i32 = 8;

const CELL_UNKNOWN: u8 = 0;
const CELL_OPEN: u8 = 1;
const CELL_CLOSED: u8 = 2;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum SettledState {
    Block(Option<VoxelId>),
    Bandlands,
    Resume(usize),
}

enum Settled {
    Answer(SettledState),
    Open {
        guard: usize,
        condition: CondId,
        on_true: usize,
        on_false: usize,
    },
    Blocked,
}

#[derive(Clone, Copy)]
struct Piece {
    lo: i32,
    hi: i32,
    biome: Option<u16>,
}

enum Split {
    Always(bool),
    At(i32),
    AtOrBelow(i32),
}

#[derive(Clone, Copy)]
struct VeinCells {
    cell: IVec3,
    size: IVec3,
    at: usize,
}

impl MaterialScratch {
    pub fn bypass_shortcuts(&mut self, bypass: bool) {
        self.bypass_shortcuts = bypass;
    }
}

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
    preliminary: SampleGrid,
    veins: SampleGrid,
    vein_row: usize,
    run_top: i32,
    run_ceiling: i32,
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
    biome: u16,
    biome_stamp: u32,
}

impl<'a, B, R> MaterialEval<'a, B, R>
where
    B: FnMut(i32, i32, i32) -> u16,
    R: FnMut(i32, i32, i32, i32, &mut Vec<u16>) -> bool,
{
    #[expect(
        clippy::too_many_arguments,
        reason = "independent inputs of one column's evaluator"
    )]
    pub fn new(
        router: &'a NoiseRouter,
        program: &'a MaterialProgram,
        scratch: &'a mut MaterialScratch,
        biome_at: B,
        reachable_at: R,
        block_x: i32,
        block_z: i32,
        top: i32,
        biomes: &[u16],
    ) -> Self {
        let min_y = router.noise.min_y;
        let memoise = !scratch.bypass_shortcuts;

        scratch.conditions.clear();
        scratch
            .conditions
            .resize(program.conditions().len(), (0, false));
        scratch.noises.clear();
        scratch.noises.resize(program.noise_count(), (0, 0.0));

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
                Condition::Biome { set } => match program
                    .biome_sets()
                    .get(set as usize)
                    .map(|set| set.fold(biomes.iter().copied()))
                {
                    Some(Tri::Never) | None => Some(false),
                    Some(Tri::Always) => Some(true),
                    Some(Tri::Maybe) => None,
                },
                Condition::Not(inner) => scratch
                    .folded
                    .get(inner as usize)
                    .copied()
                    .flatten()
                    .map(|value| !value),
                _ => None,
            };
            scratch.folded.push(folded);
        }

        let preliminary = SampleGrid::dense(IVec3::new(16, 1, 16), IVec3::new(block_x, 0, block_z));
        scratch.preliminary.clear();
        scratch.preliminary.resize(preliminary.len(), 0.0);
        router.program.fill(
            &mut scratch.workspace,
            &preliminary,
            CHUNK_SURFACE_LEVEL,
            &mut scratch.preliminary,
        );

        let veins = SampleGrid::dense(
            IVec3::new(16, (top - min_y + 1).max(1), 16),
            IVec3::new(block_x, min_y, block_z),
        );
        plan_veins(router, program, scratch, &veins);

        Self {
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
            run_ceiling: 0,
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
        }
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

    pub fn settled_runs(
        &mut self,
        top: i32,
        bottom: i32,
        ceiling: i32,
        depth_above: i32,
        water_level: i32,
        out: &mut Vec<(i32, i32, SettledState)>,
    ) {
        out.clear();
        if self.scratch.bypass_shortcuts || top - bottom < SHORT_RUN {
            return;
        }
        self.run_top = top;
        self.run_ceiling = ceiling;
        self.run_depth = depth_above;
        self.water_level = water_level;
        let min_y = self.veins.min_block().y;
        let program = self.program;
        let mut breaks = std::mem::take(&mut self.scratch.breaks);
        let splits = std::mem::take(&mut self.scratch.splits);
        breaks.clear();
        for &id in &splits {
            match program.conditions().get(id as usize).map(|c| &c.kind) {
                Some(&Condition::VerticalGradient {
                    true_at_and_below,
                    false_at_and_above,
                    ..
                }) => breaks.extend([true_at_and_below + 1, false_at_and_above]),
                _ => {
                    if let Some(Split::At(at)) | Some(Split::AtOrBelow(at)) = self.split(id) {
                        breaks.extend([at, at + 1]);
                    }
                }
            }
        }
        self.scratch.splits = splits;
        for vein in 0..self.scratch.vein_layout.len() {
            let Some(&layout) = self.scratch.vein_layout.get(vein) else {
                break;
            };
            let cell_of = |y: i32| {
                (y - min_y)
                    .div_euclid(layout.cell.y)
                    .clamp(0, layout.size.y - 1)
            };
            for cy in cell_of(bottom)..=cell_of(top) {
                if self.vein_cell_open(vein, cy) {
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

    fn settle_piece(&mut self, lo: i32, hi: i32, out: &mut Vec<(i32, i32, SettledState)>) {
        let piece = Piece {
            lo,
            hi,
            biome: None,
        };
        match self.settle_from(0, piece) {
            Settled::Open {
                guard, condition, ..
            } if self.reads_biome(condition) => self.settle_by_biome(guard, lo, hi, out),
            settled => self.settle_open(piece, settled, out),
        }
    }

    fn settle_by_biome(
        &mut self,
        guard: usize,
        lo: i32,
        hi: i32,
        out: &mut Vec<(i32, i32, SettledState)>,
    ) {
        let mut top = hi;
        let mut above = (self.biome_at)(self.block_x, hi, self.block_z);
        for y in (lo..hi).rev() {
            let biome = (self.biome_at)(self.block_x, y, self.block_z);
            if biome != above {
                self.settle_exact(guard, y + 1, top, above, out);
                top = y;
                above = biome;
            }
        }
        self.settle_exact(guard, lo, top, above, out);
    }

    fn settle_exact(
        &mut self,
        guard: usize,
        lo: i32,
        hi: i32,
        biome: u16,
        out: &mut Vec<(i32, i32, SettledState)>,
    ) {
        let piece = Piece {
            lo,
            hi,
            biome: Some(biome),
        };
        let settled = self.settle_from(guard, piece);
        self.settle_open(piece, settled, out);
    }

    fn reads_biome(&self, condition: CondId) -> bool {
        match self.program.conditions().get(condition as usize) {
            Some(CompiledCondition {
                kind: Condition::Biome { .. },
                ..
            }) => true,
            Some(CompiledCondition {
                kind: Condition::Not(inner),
                ..
            }) => self.reads_biome(*inner),
            _ => false,
        }
    }

    fn settle_open(
        &mut self,
        piece: Piece,
        settled: Settled,
        out: &mut Vec<(i32, i32, SettledState)>,
    ) {
        let Piece { lo, hi, .. } = piece;
        let (guard, condition, on_true, on_false) = match settled {
            Settled::Answer(state) => return out.push((lo, hi, state)),
            Settled::Blocked => return,
            Settled::Open {
                guard,
                condition,
                on_true,
                on_false,
            } => (guard, condition, on_true, on_false),
        };
        let resume = SettledState::Resume(guard);
        let Some(&CompiledCondition {
            kind:
                Condition::VerticalGradient {
                    random,
                    true_at_and_below,
                    false_at_and_above,
                },
            ..
        }) = self.program.conditions().get(condition as usize)
        else {
            return out.push((lo, hi, resume));
        };
        if lo <= true_at_and_below || hi >= false_at_and_above {
            return out.push((lo, hi, resume));
        }
        let (Settled::Answer(whenever), Settled::Answer(otherwise)) = (
            self.settle_from(on_true, piece),
            self.settle_from(on_false, piece),
        ) else {
            return out.push((lo, hi, resume));
        };
        if whenever == otherwise {
            return out.push((lo, hi, whenever));
        }
        for y in (lo..=hi).rev() {
            let probability = remap(
                f64::from(y),
                f64::from(true_at_and_below),
                f64::from(false_at_and_above),
                1.0,
                0.0,
            );
            let state = if self
                .program
                .random_at(random, IVec3::new(self.block_x, y, self.block_z))
                .is_some_and(|mut draw| f64::from(draw.next_f32()) < probability)
            {
                whenever
            } else {
                otherwise
            };
            out.push((y, y, state));
        }
    }

    fn settle_from(&mut self, mut pc: usize, piece: Piece) -> Settled {
        let program = self.program;
        while let Some(op) = program.tape().get(pc) {
            match *op {
                Op::Guard { condition, skip_to } => match self.over(condition, piece) {
                    Some(true) => pc += 1,
                    Some(false) => pc = skip_to as usize,
                    None => {
                        return Settled::Open {
                            guard: pc,
                            condition,
                            on_true: pc + 1,
                            on_false: skip_to as usize,
                        };
                    }
                },
                Op::Block { state } => return Settled::Answer(SettledState::Block(Some(state))),
                Op::Bandlands => return Settled::Answer(SettledState::Bandlands),
                Op::OreVein { vein } => {
                    if self.vein_open_in(vein as usize, piece.lo, piece.hi) {
                        return Settled::Blocked;
                    }
                    pc += 1;
                }
            }
        }
        Settled::Answer(SettledState::Block(None))
    }

    fn over(&mut self, condition: CondId, piece: Piece) -> Option<bool> {
        let Piece { lo, hi, biome } = piece;
        if let Some(&Some(value)) = self.scratch.folded.get(condition as usize) {
            return Some(value);
        }
        let program = self.program;
        let compiled = program.conditions().get(condition as usize)?;
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
            Condition::Not(inner) => self.over(inner, piece).map(|value| !value),
            Condition::Biome { set } => {
                let set = program.biome_sets().get(set as usize)?;
                if let Some(biome) = biome {
                    return Some(set.contains(biome));
                }
                if !self.reachable_over(lo, hi) {
                    return None;
                }
                match set.fold(self.scratch.reachable.iter().copied()) {
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

    fn split(&mut self, condition: CondId) -> Option<Split> {
        let (ceiling_y, water) = (self.run_ceiling, self.water_level);
        let top = self.run_top + self.run_depth - 1;
        let program = self.program;
        let compiled = program.conditions().get(condition as usize)?;
        Some(match compiled.kind {
            Condition::StoneDepth {
                offset,
                add_surface_depth,
                secondary_depth_range,
                ceiling,
            } => {
                let limit =
                    self.stone_depth_limit(offset, add_surface_depth, secondary_depth_range);
                if ceiling {
                    Split::AtOrBelow(ceiling_y + limit - 1)
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
            remap(
                self.surface_secondary(),
                -1.0,
                1.0,
                0.0,
                f64::from(secondary_depth_range),
            ) as i32
        };
        1 + offset + surface_depth + secondary
    }

    fn vein_open_in(&mut self, vein: usize, lo: i32, hi: i32) -> bool {
        let Some(&layout) = self.scratch.vein_layout.get(vein) else {
            return true;
        };
        let min_y = self.veins.min_block().y;
        let first = (lo - min_y) / layout.cell.y;
        let last = (hi - min_y) / layout.cell.y;
        if last >= layout.size.y {
            return true;
        }
        (first..=last).any(|cy| self.vein_cell_open(vein, cy))
    }

    pub fn update_y(&mut self, depth_above: i32, depth_below: i32, water_level: i32, block_y: i32) {
        self.gen_y += 1;
        self.block_y = block_y;
        self.water_level = water_level;
        self.depth_above = depth_above;
        self.depth_below = depth_below;
    }

    pub fn apply(&mut self) -> Option<VoxelId> {
        self.apply_from(0)
    }

    pub fn apply_from(&mut self, start: usize) -> Option<VoxelId> {
        let mut pc = start;
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
                Op::Bandlands => return self.bandlands(),
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
            .and_then(|index| self.scratch.preliminary.get(index))
        {
            Some(&level) => level,
            None => self.sample(
                CHUNK_SURFACE_LEVEL,
                IVec3::new(self.block_x, 0, self.block_z),
            ),
        };
        self.min_surface_level = floor_to_i32(level) + self.surface_depth - 8;
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

    pub(crate) fn biome(&mut self) -> u16 {
        if self.memoise && self.biome_stamp == self.gen_y {
            return self.biome;
        }
        self.biome_stamp = self.gen_y;
        self.biome = (self.biome_at)(self.block_x, self.block_y, self.block_z);
        self.biome
    }

    pub(crate) fn noise(&mut self, noise: NoiseId, is_3d: bool) -> f64 {
        let stamp = if is_3d { self.gen_y } else { self.gen_xz };
        if self.memoise
            && let Some(&(held, value)) = self.scratch.noises.get(noise as usize)
            && held == stamp
        {
            return value;
        }
        let Some(sampler) = self.program.noise(noise) else {
            return 0.0;
        };
        let y = if is_3d { f64::from(self.block_y) } else { 0.0 };
        let value = f64::from(sampler.get(f64::from(self.block_x), y, f64::from(self.block_z)));
        if let Some(slot) = self.scratch.noises.get_mut(noise as usize) {
            *slot = (stamp, value);
        }
        value
    }

    fn sample(&mut self, root: usize, pos: IVec3) -> f32 {
        let mut out = [0.0f32];
        self.router.program.fill(
            &mut self.scratch.workspace,
            &SampleGrid::point(pos),
            root,
            &mut out,
        );
        let [value] = out;
        value
    }

    fn prefilled(&mut self, vein: usize, root: usize) -> f32 {
        let dy = self.block_y - self.veins.min_block().y;
        if (0..self.veins.size().y).contains(&dy)
            && let Some(&value) = self
                .scratch
                .vein_values
                .get(vein * self.veins.len() + self.vein_row + dy as usize)
        {
            return value;
        }
        self.sample(root, IVec3::new(self.block_x, self.block_y, self.block_z))
    }

    fn vein_possible(&mut self, vein: usize) -> bool {
        let Some(&layout) = self.scratch.vein_layout.get(vein) else {
            return true;
        };
        let cy = (self.block_y - self.veins.min_block().y) / layout.cell.y;
        cy >= layout.size.y || self.vein_cell_open(vein, cy)
    }

    fn vein_cell_open(&mut self, vein: usize, cy: i32) -> bool {
        let Some(&strip) = self.scratch.vein_strip.get(vein) else {
            return true;
        };
        let slot = strip + cy as usize;
        match self.scratch.vein_cells.get(slot) {
            Some(&CELL_OPEN) | None => return true,
            Some(&CELL_CLOSED) => return false,
            Some(_) => {}
        }
        let program = self.program;
        let (Some(&layout), Some(bounds), Some(ore_vein)) = (
            self.scratch.vein_layout.get(vein),
            program.vein_cell_bounds(vein),
            program.veins().get(vein),
        ) else {
            return true;
        };

        let min = self.veins.min_block();
        let local = IVec3::new(self.block_x, 0, self.block_z) - min;
        let (cx, cz) = (local.x / layout.cell.x, local.z / layout.cell.z);
        let cell_min = min + IVec3::new(cx, cy, cz) * layout.cell;
        let cell_max = cell_min + layout.cell - 1;

        let lattice = SampleGrid::new(layout.size + IVec3::ONE, min, layout.cell);
        self.pin_vein_lattice(vein, bounds, lattice);

        corner_bounds(
            &self.scratch.lattice,
            &lattice,
            IVec3::new(cx, cy, cz),
            &mut self.scratch.corners,
        );
        let closed = cell_min.y <= self.veins.max_block().y
            && bounds
                .eval(
                    &self.router.program,
                    &self.scratch.corners,
                    cell_min,
                    cell_max,
                    &mut self.scratch.cell_terms,
                )
                .is_some_and(|bound| bound.max() < -CELL_BOUNDS_SLACK);
        let closed = closed
            || (cell_min.y <= self.veins.max_block().y && {
                sampled_bounds(
                    &self.scratch.lattice,
                    &lattice,
                    IVec3::new(cx, cy, cz),
                    layout.cell,
                    &mut self.scratch.corners,
                );
                bounds
                    .eval(
                        &self.router.program,
                        &self.scratch.corners,
                        cell_min,
                        cell_max,
                        &mut self.scratch.cell_terms,
                    )
                    .is_some_and(|bound| bound.max() < -CELL_BOUNDS_SLACK)
            });
        if let Some(cell) = self.scratch.vein_cells.get_mut(slot) {
            *cell = if closed { CELL_CLOSED } else { CELL_OPEN };
        }
        if closed {
            return false;
        }

        let rows = (self.veins.max_block().y - cell_min.y + 1).min(layout.cell.y);
        if rows > 0 {
            self.fill_vein_cell(vein, layout, ore_vein.density, cell_min, rows, cx, cz);
        }
        true
    }

    fn pin_vein_lattice(&mut self, vein: usize, bounds: &CellBounds, lattice: SampleGrid) {
        let inputs = bounds.inputs();
        if let Some((held, volume)) = &self.scratch.vein_lattice_for
            && *volume == lattice
            && self
                .program
                .vein_cell_bounds(*held)
                .is_some_and(|held| held.inputs() == inputs)
        {
            return;
        }
        self.scratch.lattice.clear();
        self.scratch
            .lattice
            .resize(inputs.len() * lattice.len(), 0.0);
        self.router.fill_nodes(
            &mut self.scratch.workspace,
            &lattice,
            inputs,
            &mut self.scratch.lattice,
        );
        self.router.pin_lattice_of(
            bounds,
            &mut self.scratch.workspace,
            &lattice,
            &self.scratch.lattice,
        );
        self.scratch
            .corners
            .resize(inputs.len(), Interval::exact(0.0));
        self.scratch.vein_lattice_for = Some((vein, lattice));
    }

    #[expect(
        clippy::too_many_arguments,
        reason = "the vein, its layout and density root, and the cell's origin, height and column offsets"
    )]
    fn fill_vein_cell(
        &mut self,
        vein: usize,
        layout: VeinCells,
        density: usize,
        cell_min: IVec3,
        rows: i32,
        cx: i32,
        cz: i32,
    ) {
        let dense = SampleGrid::dense(IVec3::new(layout.cell.x, rows, layout.cell.z), cell_min);
        self.scratch.cell_density.clear();
        self.scratch.cell_density.resize(dense.len(), 0.0);
        self.router.program.fill(
            &mut self.scratch.workspace,
            &dense,
            density,
            &mut self.scratch.cell_density,
        );

        let points = self.veins.len();
        let min_y = self.veins.min_block().y;
        let row_base = vein * points;
        for dz in 0..layout.cell.z {
            for dx in 0..layout.cell.x {
                let from = dense.index_unchecked(dx, 0, dz);
                let to = row_base
                    + self.veins.index_unchecked(
                        cx * layout.cell.x + dx,
                        cell_min.y - min_y,
                        cz * layout.cell.z + dz,
                    );
                if let (Some(to), Some(from)) = (
                    self.scratch.vein_values.get_mut(to..to + rows as usize),
                    self.scratch.cell_density.get(from..from + rows as usize),
                ) {
                    to.copy_from_slice(from);
                }
            }
        }
    }

    fn compute(&mut self, condition: CondId) -> bool {
        let program = self.program;
        let Some(compiled) = program.conditions().get(condition as usize) else {
            return false;
        };
        match compiled.kind {
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
                program
                    .biome_sets()
                    .get(set as usize)
                    .is_some_and(|set| set.contains(biome))
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
                    let probability = remap(
                        f64::from(self.block_y),
                        f64::from(true_at_and_below),
                        f64::from(false_at_and_above),
                        1.0,
                        0.0,
                    );
                    program
                        .random_at(random, IVec3::new(self.block_x, self.block_y, self.block_z))
                        .is_some_and(|mut random| f64::from(random.next_f32()) < probability)
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
    pub(crate) fn set_surface_depth(&mut self, depth: i32) {
        self.surface_depth = depth;
        self.min_surface_level_stamp = 0;
    }
}

impl<B, R> MaterialEval<'_, B, R>
where
    B: FnMut(i32, i32, i32) -> u16,
    R: FnMut(i32, i32, i32, i32, &mut Vec<u16>) -> bool,
{
    pub(crate) fn test(&mut self, condition: CondId) -> bool {
        if !self.memoise {
            return self.compute(condition);
        }
        if let Some(&Some(value)) = self.scratch.folded.get(condition as usize) {
            return value;
        }
        let Some(compiled) = self.program.conditions().get(condition as usize) else {
            return false;
        };
        let stamp = match compiled.scope {
            Scope::Xz => self.gen_xz,
            Scope::Y => self.gen_y,
        };
        if let Some(&(held, value)) = self.scratch.conditions.get(condition as usize)
            && held == stamp
        {
            return value;
        }
        let value = self.compute(condition);
        if let Some(slot) = self.scratch.conditions.get_mut(condition as usize) {
            *slot = (stamp, value);
        }
        value
    }

    pub(crate) fn bandlands(&mut self) -> Option<VoxelId> {
        self.bandlands_at(self.block_y)
    }

    pub fn bandlands_at(&mut self, y: i32) -> Option<VoxelId> {
        let noise = self.noise(
            self.program.surface_noise(SurfaceNoise::ClayBandsOffset),
            false,
        );
        let offset = ((noise as f32 * 4.0) + 0.5).floor() as i32;
        self.program
            .clay_bands()
            .get((y + offset).rem_euclid(CLAY_BAND_COUNT as i32) as usize)
            .copied()
    }

    pub(crate) fn ore_vein(&mut self, vein: VeinId) -> Option<VoxelId> {
        if !self.vein_possible(vein as usize) {
            return None;
        }
        let ore_vein = *self.program.veins().get(vein as usize)?;
        let density = self.prefilled(vein as usize, ore_vein.density);
        if density <= 0.0 {
            return None;
        }
        let at = IVec3::new(self.block_x, self.block_y, self.block_z);
        let mut random = self.program.random_at(ore_vein.random, at)?;
        if random.next_f32() > density {
            return None;
        }
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

fn plan_veins(
    router: &NoiseRouter,
    program: &MaterialProgram,
    scratch: &mut MaterialScratch,
    veins: &SampleGrid,
) {
    let points = veins.len();
    let height = router.noise.height as i32;
    scratch.vein_values.clear();
    scratch
        .vein_values
        .resize(program.veins().len() * points, 0.0);
    scratch.vein_cells.clear();
    scratch.vein_layout.clear();
    scratch.vein_strip.clear();
    scratch.vein_lattice_for = None;

    for (index, vein) in program.veins().iter().enumerate() {
        let cell = program
            .vein_cell_bounds(index)
            .and_then(CellBounds::cell_size)
            .filter(|c| 16 % c.x == 0 && 16 % c.z == 0 && height % c.y == 0);
        let at = scratch.vein_cells.len();
        scratch.vein_strip.push(at);
        let Some(cell) = cell else {
            scratch.vein_layout.push(VeinCells {
                cell: IVec3::new(16, height, 16),
                size: IVec3::ONE,
                at,
            });
            scratch.vein_cells.push(CELL_OPEN);
            if let Some(row) = scratch
                .vein_values
                .get_mut(index * points..(index + 1) * points)
            {
                router
                    .program
                    .fill(&mut scratch.workspace, veins, vein.density, row);
            }
            continue;
        };

        let size = IVec3::new(16 / cell.x, height / cell.y, 16 / cell.z);
        scratch.vein_layout.push(VeinCells { cell, size, at });
        scratch
            .vein_cells
            .resize(at + (size.x * size.y * size.z) as usize, CELL_UNKNOWN);
    }
}
