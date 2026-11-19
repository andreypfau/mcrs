use crate::compile::{
    CLAY_BAND_COUNT, CompiledCondition, CondId, Condition, MaterialProgram, NoiseId, Op, Scope,
    SurfaceNoise, Tri, VeinId,
};
use glam::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_noise::interval::Interval;
use mcrs_minecraft_noise::sample_grid::SampleGrid;
use mcrs_minecraft_noise::stack::ColumnScratch;
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
    run_splits: Vec<Option<Split>>,
    breaks: Vec<i32>,
    open_cells: Vec<OpenCell>,
    zones: Vec<Zone>,
    reachable: Vec<u16>,
    noise_columns: Vec<f32>,
    noise_column_stamps: Vec<u32>,
    window_ys: Vec<f64>,
    window_ys_stamp: u32,
    column: ColumnScratch,
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
    open_veins: Option<u64>,
}

#[derive(Clone, Copy)]
struct Zone {
    lo: i32,
    hi: i32,
    entry: usize,
}

impl Zone {
    const WHOLE: Self = Self {
        lo: i32::MIN,
        hi: i32::MAX,
        entry: 0,
    };
}

#[derive(Clone, Copy)]
struct OpenCell {
    bottom: i32,
    top: i32,
    vein: u32,
}

#[derive(Clone, Copy)]
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
    open_rows: OpenRows,
}

#[derive(Clone, Copy)]
struct OpenRows {
    first: i32,
    last: i32,
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
    window_lo: i32,
    window_hi: i32,
    gen_window: u32,
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
        scratch.run_splits.clear();
        scratch.run_splits.resize(program.conditions().len(), None);
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
        plan_noise_columns(program, scratch, &veins);

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
            window_lo: 0,
            window_hi: -1,
            gen_window: 0,
        }
    }

    pub fn begin_strip(&mut self, block_x: i32, block_z: i32, gradient_x: i32, gradient_z: i32) {
        self.gen_xz += 1;
        self.gen_y += 1;
        self.block_x = block_x;
        self.block_z = block_z;
        self.gradient_x = gradient_x;
        self.gradient_z = gradient_z;
        self.close_window();
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
        self.close_window();
        if self.scratch.bypass_shortcuts {
            return;
        }
        if top - bottom >= SHORT_RUN {
            self.settle_run(top, bottom, ceiling, depth_above, water_level, out);
        }
        self.open_window(top, bottom, out);
    }

    fn close_window(&mut self) {
        self.window_lo = 0;
        self.window_hi = -1;
    }

    fn open_window(&mut self, top: i32, bottom: i32, settled: &[(i32, i32, SettledState)]) {
        let (mut lo, mut hi) = (i32::MAX, i32::MIN);
        let mut cursor = top;
        for &(piece_lo, piece_hi, state) in settled {
            if piece_hi < cursor {
                lo = lo.min(piece_hi + 1);
                hi = hi.max(cursor);
            }
            if let SettledState::Resume(_) = state {
                lo = lo.min(piece_lo);
                hi = hi.max(piece_hi);
            }
            cursor = piece_lo - 1;
        }
        if cursor >= bottom {
            lo = lo.min(bottom);
            hi = hi.max(cursor);
        }
        let min_y = self.veins.min_block().y;
        if lo <= hi && lo >= min_y && hi - min_y < self.veins.size().y {
            self.window_lo = lo;
            self.window_hi = hi;
            self.gen_window = self.gen_window.wrapping_add(1);
        }
    }

    fn settle_run(
        &mut self,
        top: i32,
        bottom: i32,
        ceiling: i32,
        depth_above: i32,
        water_level: i32,
        out: &mut Vec<(i32, i32, SettledState)>,
    ) {
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
                    let split = self.split(id);
                    if let Some(Split::At(at) | Split::AtOrBelow(at)) = split {
                        breaks.extend([at, at + 1]);
                    }
                    if let Some(slot) = self.scratch.run_splits.get_mut(id as usize) {
                        *slot = split;
                    }
                }
            }
        }
        self.scratch.splits = splits;
        let mut open_cells = std::mem::take(&mut self.scratch.open_cells);
        open_cells.clear();
        let mut cells_known = bottom >= min_y;
        for vein in 0..self.scratch.vein_layout.len() {
            let Some(&layout) = self.scratch.vein_layout.get(vein) else {
                break;
            };
            let cell_of = |y: i32| {
                (y - min_y)
                    .div_euclid(layout.cell.y)
                    .clamp(0, layout.size.y - 1)
            };
            cells_known &= (top - min_y) / layout.cell.y < layout.size.y;
            let first = cell_of(bottom).max(layout.open_rows.first);
            let last = cell_of(top).min(layout.open_rows.last);
            for cy in first..=last {
                if self.vein_cell_open(vein, cy) {
                    let cell_bottom = min_y + cy * layout.cell.y;
                    breaks.extend([cell_bottom, cell_bottom + layout.cell.y]);
                    open_cells.push(OpenCell {
                        bottom: cell_bottom,
                        top: cell_bottom + layout.cell.y - 1,
                        vein: vein as u32,
                    });
                }
            }
        }
        breaks.retain(|&y| y > bottom && y <= top);
        breaks.sort_unstable_by(|a, b| b.cmp(a));
        breaks.dedup();

        let piece_of = |lo: i32, hi: i32| Piece {
            lo,
            hi,
            biome: None,
            open_veins: cells_known.then(|| {
                open_cells
                    .iter()
                    .filter(|cell| cell.bottom <= hi && cell.top >= lo)
                    .fold(0, |mask, cell| {
                        mask | 1u64.checked_shl(cell.vein).unwrap_or(0)
                    })
            }),
        };
        let mut zones = std::mem::take(&mut self.scratch.zones);
        zones.clear();
        let mut hi = top;
        for &lo in &breaks {
            let piece = piece_of(lo, hi);
            let entry = self.entry_for(piece, &open_cells, &mut zones);
            self.settle_piece(entry, piece, out);
            hi = lo - 1;
        }
        if hi >= bottom {
            let piece = piece_of(bottom, hi);
            let entry = self.entry_for(piece, &open_cells, &mut zones);
            self.settle_piece(entry, piece, out);
        }
        self.scratch.breaks = breaks;
        self.scratch.open_cells = open_cells;
        self.scratch.zones = zones;
    }

    fn entry_for(&mut self, piece: Piece, open_cells: &[OpenCell], zones: &mut Vec<Zone>) -> usize {
        while zones
            .last()
            .is_some_and(|zone| zone.lo > piece.lo || piece.hi > zone.hi)
        {
            zones.pop();
        }
        let Zone {
            mut lo,
            mut hi,
            entry: mut pc,
        } = zones.last().copied().unwrap_or(Zone::WHOLE);
        let program = self.program;
        while let Some(&op) = program.tape().get(pc) {
            let (was_lo, was_hi) = (lo, hi);
            match op {
                Op::Guard { condition, skip_to } => {
                    let decided = if self.reads_biome(condition) {
                        self.scratch
                            .folded
                            .get(condition as usize)
                            .copied()
                            .flatten()
                    } else {
                        self.over(condition, piece)
                    };
                    let Some(value) = decided else {
                        break;
                    };
                    self.narrow(condition, value, &mut lo, &mut hi);
                    pc = if value { pc + 1 } else { skip_to as usize };
                }
                Op::OreVein { vein } => {
                    if piece.open_veins.is_none() || self.vein_open_over(vein, piece) {
                        break;
                    }
                    for cell in open_cells.iter().filter(|cell| cell.vein == vein) {
                        if cell.top < piece.lo {
                            lo = lo.max(cell.top.saturating_add(1));
                        } else {
                            hi = hi.min(cell.bottom.saturating_sub(1));
                        }
                    }
                    pc += 1;
                }
                Op::Block { .. } | Op::Bandlands => break,
            }
            match zones.last_mut() {
                Some(zone) if (lo, hi) == (was_lo, was_hi) => zone.entry = pc,
                _ => zones.push(Zone { lo, hi, entry: pc }),
            }
        }
        pc
    }

    fn narrow(&self, condition: CondId, value: bool, lo: &mut i32, hi: &mut i32) {
        if let Some(&Some(_)) = self.scratch.folded.get(condition as usize) {
            return;
        }
        let Some(compiled) = self.program.conditions().get(condition as usize) else {
            return;
        };
        let split = match compiled.kind {
            Condition::Not(inner) => return self.narrow(inner, !value, lo, hi),
            Condition::VerticalGradient {
                true_at_and_below,
                false_at_and_above,
                ..
            } => {
                if value {
                    *hi = (*hi).min(true_at_and_below);
                } else {
                    *lo = (*lo).max(false_at_and_above);
                }
                return;
            }
            _ => self
                .scratch
                .run_splits
                .get(condition as usize)
                .copied()
                .flatten(),
        };
        match (split, value) {
            (Some(Split::At(at)), true) => *lo = (*lo).max(at),
            (Some(Split::At(at)), false) => *hi = (*hi).min(at.saturating_sub(1)),
            (Some(Split::AtOrBelow(at)), true) => *hi = (*hi).min(at),
            (Some(Split::AtOrBelow(at)), false) => *lo = (*lo).max(at.saturating_add(1)),
            _ => {}
        }
    }

    fn settle_piece(
        &mut self,
        entry: usize,
        piece: Piece,
        out: &mut Vec<(i32, i32, SettledState)>,
    ) {
        match self.settle_from(entry, piece) {
            Settled::Open {
                guard, condition, ..
            } if self.reads_biome(condition) => self.settle_by_biome(guard, piece, out),
            settled => self.settle_open(piece, settled, out),
        }
    }

    fn settle_by_biome(
        &mut self,
        guard: usize,
        piece: Piece,
        out: &mut Vec<(i32, i32, SettledState)>,
    ) {
        let Piece { lo, hi, .. } = piece;
        let mut top = hi;
        let mut above = (self.biome_at)(self.block_x, hi, self.block_z);
        for y in (lo..hi).rev() {
            let biome = (self.biome_at)(self.block_x, y, self.block_z);
            if biome != above {
                self.settle_exact(guard, piece, y + 1, top, above, out);
                top = y;
                above = biome;
            }
        }
        self.settle_exact(guard, piece, lo, top, above, out);
    }

    fn settle_exact(
        &mut self,
        guard: usize,
        parent: Piece,
        lo: i32,
        hi: i32,
        biome: u16,
        out: &mut Vec<(i32, i32, SettledState)>,
    ) {
        let piece = Piece {
            lo,
            hi,
            biome: Some(biome),
            open_veins: parent.open_veins,
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
                    if self.vein_open_over(vein, piece) {
                        return Settled::Blocked;
                    }
                    pc += 1;
                }
            }
        }
        Settled::Answer(SettledState::Block(None))
    }

    fn over(&mut self, condition: CondId, piece: Piece) -> Option<bool> {
        let Piece { lo, hi, biome, .. } = piece;
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
            _ => match self
                .scratch
                .run_splits
                .get(condition as usize)
                .copied()
                .flatten()
            {
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

    fn vein_open_over(&mut self, vein: VeinId, piece: Piece) -> bool {
        match (piece.open_veins, 1u64.checked_shl(vein)) {
            (Some(mask), Some(bit)) if (vein as usize) < self.scratch.vein_layout.len() => {
                mask & bit != 0
            }
            _ => self.vein_open_in(vein as usize, piece.lo, piece.hi),
        }
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
        let first = first.max(layout.open_rows.first);
        let last = last.min(layout.open_rows.last);
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
        let program = self.program;
        while let Some(op) = program.tape().get(pc) {
            match *op {
                Op::Guard { condition, skip_to } => {
                    if self.memoise
                        && let Some(band) = program.noise_bands().at(pc)
                    {
                        let value = self.noise(band.noise, band.is_3d);
                        pc = program.noise_bands().select(band, value);
                    } else if self.test(condition) {
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
        let value = match self.windowed_noise(noise, is_3d) {
            Some(value) => value,
            None => f64::from(sampler.get(f64::from(self.block_x), y, f64::from(self.block_z))),
        };
        if let Some(slot) = self.scratch.noises.get_mut(noise as usize) {
            *slot = (stamp, value);
        }
        value
    }

    fn windowed_noise(&mut self, noise: NoiseId, is_3d: bool) -> Option<f64> {
        if !is_3d || !self.memoise || !(self.window_lo..=self.window_hi).contains(&self.block_y) {
            return None;
        }
        let stride = self.veins.size().y as usize;
        let len = (self.window_hi - self.window_lo + 1) as usize;
        let at = noise as usize * stride;
        let stamp = self.scratch.noise_column_stamps.get_mut(noise as usize)?;
        if *stamp != self.gen_window {
            let sampler = self.program.noise(noise)?;
            let out = self.scratch.noise_columns.get_mut(at..at + len)?;
            if self.scratch.window_ys_stamp != self.gen_window {
                self.scratch.window_ys_stamp = self.gen_window;
                self.scratch.window_ys.clear();
                self.scratch
                    .window_ys
                    .extend((self.window_lo..=self.window_hi).map(f64::from));
            }
            sampler.fill_column(
                out,
                f64::from(self.block_x),
                f64::from(self.block_z),
                &self.scratch.window_ys,
                &mut self.scratch.column,
            );
            *stamp = self.gen_window;
        }
        let index = (self.block_y - self.window_lo) as usize;
        self.scratch
            .noise_columns
            .get(at + index)
            .map(|&value| f64::from(value))
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
        if let Some(layout) = self.scratch.vein_layout.get(vein)
            && cy < layout.size.y
            && !(layout.open_rows.first..=layout.open_rows.last).contains(&cy)
        {
            return false;
        }
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

fn plan_noise_columns(
    program: &MaterialProgram,
    scratch: &mut MaterialScratch,
    veins: &SampleGrid,
) {
    scratch.noise_column_stamps.clear();
    scratch.noise_column_stamps.resize(program.noise_count(), 0);
    scratch.window_ys_stamp = 0;
    scratch.noise_columns.clear();
    scratch
        .noise_columns
        .resize(program.noise_count() * veins.size().y as usize, 0.0);
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
                open_rows: OpenRows { first: 0, last: 0 },
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
        let open_rows = match program.vein_cell_bounds(index) {
            Some(bounds) => open_vein_rows(router, bounds, scratch, veins, cell, size.y),
            None => OpenRows {
                first: 0,
                last: size.y - 1,
            },
        };
        scratch.vein_layout.push(VeinCells {
            cell,
            size,
            at,
            open_rows,
        });
        scratch
            .vein_cells
            .resize(at + (size.x * size.y * size.z) as usize, CELL_UNKNOWN);
    }
}

fn open_vein_rows(
    router: &NoiseRouter,
    bounds: &CellBounds,
    scratch: &mut MaterialScratch,
    veins: &SampleGrid,
    cell: IVec3,
    rows: i32,
) -> OpenRows {
    scratch.corners.clear();
    scratch
        .corners
        .resize(bounds.wrappers().len(), Interval::INFINITE);
    let mut open = OpenRows {
        first: rows,
        last: -1,
    };
    for cy in 0..rows {
        let min = veins.min_block() + IVec3::new(0, cy * cell.y, 0);
        let max = min + IVec3::new(15, cell.y - 1, 15);
        let closed = bounds
            .eval(
                &router.program,
                &scratch.corners,
                min,
                max,
                &mut scratch.cell_terms,
            )
            .is_some_and(|bound| bound.max() < -CELL_BOUNDS_SLACK);
        if !closed {
            open.first = open.first.min(cy);
            open.last = cy;
        }
    }
    open
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::compile::tests::build;

    #[test]
    fn every_positive_vein_density_lies_in_an_open_vein_row() {
        let (router, material) = build("overworld");
        let min_y = router.noise.min_y;
        let height = router.noise.height as i32;
        let top = min_y + height - 1;
        let mut scratch = MaterialScratch::default();
        let mut workspace = Workspace::default();
        let mut density = Vec::new();
        let mut positive = vec![0usize; material.veins().len()];

        for chunk in 0..12 {
            let block_x = chunk * 16 * 37 - 2048;
            let block_z = chunk * 16 * 53 + 496;
            let eval = MaterialEval::new(
                &router,
                &material,
                &mut scratch,
                |_, _, _| 0,
                |_, _, _, _, _| false,
                block_x,
                block_z,
                top,
                &[0],
            );
            let layouts = eval.scratch.vein_layout.clone();
            let veins = material.veins().iter().zip(&layouts);
            for ((vein, layout), seen) in veins.zip(&mut positive) {
                let grid = SampleGrid::new(
                    IVec3::new(8, height, 8),
                    IVec3::new(block_x, min_y, block_z),
                    IVec3::new(2, 1, 2),
                );
                density.clear();
                density.resize(grid.len(), 0.0);
                router
                    .program
                    .fill(&mut workspace, &grid, vein.density, &mut density);
                for point in 0..64 * height {
                    let (column, dy) = (point / height, point % height);
                    if density[grid.index_unchecked(column % 8, dy, column / 8)] <= 0.0 {
                        continue;
                    }
                    *seen += 1;
                    let row = dy / layout.cell.y;
                    assert!(
                        (layout.open_rows.first..=layout.open_rows.last).contains(&row),
                        "vein density is positive at y {} in closed row {row}",
                        min_y + dy,
                    );
                }
            }
        }
        assert!(
            positive.iter().all(|&count| count > 0),
            "every vein shows positive density somewhere: {positive:?}"
        );
    }
}
