use crate::jmath::floor_div;
use crate::material::eval::{clamped_map, map};
use crate::program::{Program, Workspace};
use crate::router::NoiseRouter;
use crate::volume::Volume;
use bevy_math::IVec3;
use mcrs_minecraft_random::{Random, RandomSource};
use mcrs_voxel_storage::VoxelId;

/// `DimensionType.WAY_BELOW_MIN_Y`: the level of a dry cell. Still a number,
/// and it enters the pressure formula as one.
pub const WAY_BELOW_MIN_Y: i32 = -2032 << 4;

const AIR: VoxelId = VoxelId(0);

const X_SPACING_SHIFT: i32 = 4;
const Y_SPACING: i32 = 12;
const SAMPLE_OFFSET_X: i32 = -5;
const SAMPLE_OFFSET_Y: i32 = 1;
const SAMPLE_OFFSET_Z: i32 = -5;

/// In this order: which offset returns first decides between two statuses
/// wherever their levels differ.
const SURFACE_SAMPLING_OFFSETS_IN_CHUNKS: [[i32; 2]; 13] = [
    [0, 0],
    [-2, -1],
    [-1, -1],
    [0, -1],
    [1, -1],
    [-3, 0],
    [-2, 0],
    [-1, 0],
    [1, 0],
    [-2, 1],
    [-1, 1],
    [0, 1],
    [1, 1],
];

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub struct FluidStatus {
    pub level: i32,
    pub fluid: VoxelId,
}

impl FluidStatus {
    #[inline]
    pub fn at(self, y: i32) -> VoxelId {
        if y < self.level { self.fluid } else { AIR }
    }
}

/// The rule of a dimension without an aquifer field, and what the field falls
/// back to: the dimension's fluid up to its sea level over a lava floor.
#[derive(Clone, Copy, Debug)]
pub struct GlobalFluid {
    pub sea: FluidStatus,
    pub lava: FluidStatus,
    /// Below this the lava status answers.
    pub floor: i32,
    /// Below this the rule places lava outright, whatever the field says.
    pub lava_below: i32,
}

impl GlobalFluid {
    pub fn new(sea_level: i32, default_fluid: VoxelId, lava: VoxelId) -> Self {
        let floor = sea_level.min(-54);
        Self {
            sea: FluidStatus {
                level: sea_level,
                fluid: default_fluid,
            },
            lava: FluidStatus {
                level: -54,
                fluid: lava,
            },
            floor,
            lava_below: if default_fluid == lava { sea_level } else { floor },
        }
    }

    #[inline]
    pub fn at(&self, y: i32) -> FluidStatus {
        if y < self.floor { self.lava } else { self.sea }
    }
}

/// The `aquifers` block of a `noise_settings`, compiled: root indices into the
/// dimension's program, the positional stream the centres are jittered from,
/// and the shell margins the barrier's declared interval allows.
pub struct AquiferConfig {
    pub barrier: usize,
    pub floodedness: usize,
    pub spread: usize,
    pub lava: usize,
    pub exclusion: usize,
    pub surface_level: usize,
    pub random: RandomSource,
    pub margin_above: i32,
    pub margin_below: i32,
}

impl AquiferConfig {
    /// The pressure reads the barrier noise only while its gradient is within
    /// `±2`; past that the gradient alone is negative. Inside, the noise cannot
    /// outweigh a gradient at or below `-N`. Both caps and both slopes come
    /// straight from the pressure formula.
    pub fn margins(barrier_max: f32) -> (i32, i32) {
        let n = f64::from(barrier_max);
        let above = (2.5 * n - 0.5).ceil().clamp(0.0, 5.0);
        let below = (10.0 * n + 3.5).ceil().clamp(1.0, 24.0);
        (above as i32, below as i32)
    }
}

/// What the field says about one block whose density is not positive. `None`
/// is solid: the barrier pushed the density over zero.
pub type Substance = Option<VoxelId>;

/// The twelve statuses every block of one lattice cell selects from, reduced
/// to what settles a run of blocks without the search.
#[derive(Clone, Copy, Debug)]
pub struct Window {
    pub lmax: i32,
    pub lmin: i32,
    /// The one fluid every status holds, when none is dry.
    pub one_type: Option<VoxelId>,
}

impl Window {
    fn of(status: FluidStatus) -> Self {
        Self {
            lmax: status.level,
            lmin: status.level,
            one_type: (status.level != WAY_BELOW_MIN_Y).then_some(status.fluid),
        }
    }

    fn merge(self, other: Self) -> Self {
        Self {
            lmax: self.lmax.max(other.lmax),
            lmin: self.lmin.min(other.lmin),
            one_type: agree(self.one_type, other.one_type),
        }
    }

    /// Lemma U: one status over the whole window. A window of nothing but dry
    /// cells is left to Lemma A, which answers air for every `y` a world holds.
    fn uniform(self) -> Option<FluidStatus> {
        let fluid = self.one_type?;
        (self.lmin == self.lmax).then_some(FluidStatus {
            level: self.lmin,
            fluid,
        })
    }
}

fn agree<T: PartialEq>(left: Option<T>, right: Option<T>) -> Option<T> {
    if left == right { left } else { None }
}

/// The fluid level field over one lattice region, as dense tables owned by the
/// task that generates it. Everything here is a pure function of the seed, so
/// the region may be any size and two regions agree wherever they overlap.
pub struct FluidField<'a> {
    program: &'a Program,
    config: Option<&'a AquiferConfig>,
    global: GlobalFluid,
    water: VoxelId,
    lava: VoxelId,
    ws: Workspace,
    min_grid: IVec3,
    grid_size: IVec3,
    centres: Box<[IVec3]>,
    status: Box<[Option<FluidStatus>]>,
    surface: Box<[i32]>,
    surface_min_quart: (i32, i32),
    surface_quarts_x: i32,
    windows: Box<[Option<Window>]>,
}

impl<'a> FluidField<'a> {
    /// The field over every block in `[min, max]`, both inclusive.
    pub fn new(router: &'a NoiseRouter, min: IVec3, max: IVec3) -> Self {
        let mut field = Self {
            program: &router.program,
            config: router.aquifer.as_ref(),
            global: router.global_fluid,
            water: router.water_state,
            lava: router.lava_state,
            ws: Workspace::new(),
            min_grid: IVec3::ZERO,
            grid_size: IVec3::ZERO,
            centres: Box::new([]),
            status: Box::new([]),
            surface: Box::new([]),
            surface_min_quart: (0, 0),
            surface_quarts_x: 0,
            windows: Box::new([]),
        };
        let Some(config) = router.aquifer.as_ref() else {
            return field;
        };

        let min_grid = Self::anchor(min.x, min.y, min.z) - IVec3::Y;
        let max_grid = Self::anchor(max.x, max.y, max.z) + IVec3::ONE;
        let grid_size = max_grid - min_grid + IVec3::ONE;
        field.min_grid = min_grid;
        field.grid_size = grid_size;

        let mut centres = Vec::with_capacity((grid_size.x * grid_size.y * grid_size.z) as usize);
        for gy in min_grid.y..=max_grid.y {
            for gz in min_grid.z..=max_grid.z {
                for gx in min_grid.x..=max_grid.x {
                    let mut random = config.random.clone().fork_at(IVec3::new(gx, gy, gz));
                    let jx = random.next_i32_bound(10);
                    let jy = random.next_i32_bound(9);
                    let jz = random.next_i32_bound(10);
                    centres.push(IVec3::new(
                        (gx << X_SPACING_SHIFT) + jx,
                        gy * Y_SPACING + jy,
                        (gz << X_SPACING_SHIFT) + jz,
                    ));
                }
            }
        }
        field.status = vec![None; centres.len()].into_boxed_slice();
        field.centres = centres.into_boxed_slice();

        let quart_min = (
            ((min_grid.x << X_SPACING_SHIFT) - 48) >> 2,
            ((min_grid.z << X_SPACING_SHIFT) - 16) >> 2,
        );
        let quart_max = (
            ((max_grid.x << X_SPACING_SHIFT) + 25) >> 2,
            ((max_grid.z << X_SPACING_SHIFT) + 25) >> 2,
        );
        let quarts = IVec3::new(
            quart_max.0 - quart_min.0 + 1,
            1,
            quart_max.1 - quart_min.1 + 1,
        );
        let volume = Volume::new(
            quarts,
            IVec3::new(quart_min.0 << 2, 0, quart_min.1 << 2),
            IVec3::new(4, 1, 4),
        );
        let mut sampled = vec![0.0f32; volume.len()];
        field
            .program
            .fill(&mut field.ws, &volume, config.surface_level, &mut sampled);
        field.surface = sampled
            .iter()
            .map(|&v| f64::from(v).floor() as i32)
            .collect();
        field.surface_min_quart = quart_min;
        field.surface_quarts_x = quarts.x;

        let windows = ((grid_size.x - 1) * (grid_size.y - 2) * (grid_size.z - 1)) as usize;
        field.windows = vec![None; windows].into_boxed_slice();
        field
    }

    #[inline]
    fn cell_index(&self, grid: IVec3) -> usize {
        flat(grid - self.min_grid, self.grid_size)
    }

    #[inline]
    fn surface_level(&self, x: i32, z: i32) -> i32 {
        let (qx, qz) = (
            (x >> 2) - self.surface_min_quart.0,
            (z >> 2) - self.surface_min_quart.1,
        );
        debug_assert!(qx >= 0 && qx < self.surface_quarts_x && qz >= 0);
        self.surface[(qz * self.surface_quarts_x + qx) as usize]
    }

    #[inline]
    fn global_at(&self, y: i32) -> VoxelId {
        self.global.at(y).at(y)
    }

    fn sample(&mut self, root: usize, at: IVec3) -> f32 {
        let mut out = [0.0f32];
        self.program
            .fill(&mut self.ws, &Volume::point(at), root, &mut out);
        out[0]
    }

    fn status(&mut self, index: usize) -> FluidStatus {
        if let Some(status) = self.status[index] {
            return status;
        }
        let status = self.compute_status(self.centres[index]);
        self.status[index] = Some(status);
        status
    }

    fn compute_status(&mut self, centre: IVec3) -> FluidStatus {
        let global = self.global.at(centre.y);
        let (top, bottom) = (centre.y + 12, centre.y - 12);
        let mut lowest = i32::MAX;
        let mut under_global_fluid = false;
        for (i, [ox, oz]) in SURFACE_SAMPLING_OFFSETS_IN_CHUNKS.iter().enumerate() {
            let (sx, sz) = (centre.x + (ox << 4), centre.z + (oz << 4));
            let surface = self.surface_level(sx, sz);
            let adjusted = surface + 8;
            let start = i == 0;
            if start && bottom > adjusted {
                return global;
            }
            let pokes_above = top > adjusted;
            if pokes_above || start {
                let at_surface = self.global.at(adjusted);
                if at_surface.at(adjusted) != AIR {
                    if start {
                        under_global_fluid = true;
                    }
                    if pokes_above {
                        return at_surface;
                    }
                }
            }
            lowest = lowest.min(surface);
        }
        let level = self.compute_level(centre, global, lowest, under_global_fluid);
        FluidStatus {
            level,
            fluid: self.compute_type(centre, global, level),
        }
    }

    fn compute_level(
        &mut self,
        centre: IVec3,
        global: FluidStatus,
        lowest_surface: i32,
        under_global_fluid: bool,
    ) -> i32 {
        let config = self.config.expect("statuses exist only with a field");
        let (exclusion, floodedness, spread) =
            (config.exclusion, config.floodedness, config.spread);
        let (partially, fully) = if f64::from(self.sample(exclusion, centre)) > 0.0 {
            (-1.0, -1.0)
        } else {
            let below_surface = f64::from(lowest_surface + 8 - centre.y);
            let factor = if under_global_fluid {
                clamped_map(below_surface, 0.0, 64.0, 1.0, 0.0)
            } else {
                0.0
            };
            let noise = f64::from(self.sample(floodedness, centre)).clamp(-1.0, 1.0);
            let fully_threshold = map(factor, 1.0, 0.0, -0.3, 0.8);
            let partially_threshold = map(factor, 1.0, 0.0, -0.8, 0.4);
            (noise - partially_threshold, noise - fully_threshold)
        };
        if fully > 0.0 {
            global.level
        } else if partially > 0.0 {
            let (cell_x, cell_y, cell_z) = (
                floor_div(centre.x, 16),
                floor_div(centre.y, 40),
                floor_div(centre.z, 16),
            );
            let noise = self.sample(spread, IVec3::new(cell_x, cell_y, cell_z));
            let spread = f64::from(noise * 10.0f32);
            let quantized = (spread / 3.0).floor() as i32 * 3;
            lowest_surface.min(cell_y * 40 + 20 + quantized)
        } else {
            WAY_BELOW_MIN_Y
        }
    }

    fn compute_type(&mut self, centre: IVec3, global: FluidStatus, level: i32) -> VoxelId {
        if level > -10 || level == WAY_BELOW_MIN_Y || global.fluid == self.lava {
            return global.fluid;
        }
        let root = self.config.expect("statuses exist only with a field").lava;
        let at = IVec3::new(
            floor_div(centre.x, 64),
            floor_div(centre.y, 40),
            floor_div(centre.z, 64),
        );
        let noise = self.sample(root, at);
        if f64::from(noise.abs()) > 0.3 {
            self.lava
        } else {
            global.fluid
        }
    }

    /// The three nearest centres in the reference's order: a later candidate at
    /// the same distance displaces an earlier one.
    fn select(&self, x: i32, y: i32, z: i32) -> ([i32; 3], [usize; 3]) {
        let anchor = Self::anchor(x, y, z);
        let mut distance = [i32::MAX; 3];
        let mut index = [0usize; 3];
        for dx in 0..=1 {
            for dy in -1..=1 {
                for dz in 0..=1 {
                    let cell = self.cell_index(anchor + IVec3::new(dx, dy, dz));
                    let d = self.centres[cell] - IVec3::new(x, y, z);
                    let new = d.x * d.x + d.y * d.y + d.z * d.z;
                    if distance[0] >= new {
                        index = [cell, index[0], index[1]];
                        distance = [new, distance[0], distance[1]];
                    } else if distance[1] >= new {
                        index = [index[0], cell, index[1]];
                        distance = [distance[0], new, distance[1]];
                    } else if distance[2] >= new {
                        index[2] = cell;
                        distance[2] = new;
                    }
                }
            }
        }
        (distance, index)
    }

    /// The reference's decision tree, block by block. The oracle for every
    /// shortcut in this module, and the search the shell runs.
    ///
    /// `barrier` is read at most once, and only where the pressure gradient
    /// leaves the sign to the noise.
    pub fn substance(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        density: f64,
        barrier: &mut dyn FnMut(i32, i32, i32) -> f64,
    ) -> Substance {
        if density > 0.0 {
            return None;
        }
        let global = self.global.at(y);
        if self.config.is_none() {
            return Some(global.at(y));
        }
        if self.global_at(y) == self.lava {
            return Some(self.lava);
        }
        let (distance, index) = self.select(x, y, z);
        let status1 = self.status(index[0]);
        let similarity12 = similarity(distance[0], distance[1]);
        let fluid_state = status1.at(y);
        if similarity12 <= 0.0 {
            return Some(fluid_state);
        }
        if fluid_state == self.water && self.global_at(y - 1) == self.lava {
            return Some(fluid_state);
        }
        let mut noise = None;
        let status2 = self.status(index[1]);
        let barrier12 =
            similarity12 * self.pressure(x, y, z, &mut noise, status1, status2, barrier);
        if density + barrier12 > 0.0 {
            return None;
        }
        let status3 = self.status(index[2]);
        let similarity13 = similarity(distance[0], distance[2]);
        if similarity13 > 0.0 {
            let barrier13 = similarity12
                * similarity13
                * self.pressure(x, y, z, &mut noise, status1, status3, barrier);
            if density + barrier13 > 0.0 {
                return None;
            }
        }
        let similarity23 = similarity(distance[1], distance[2]);
        if similarity23 > 0.0 {
            let barrier23 = similarity12
                * similarity23
                * self.pressure(x, y, z, &mut noise, status2, status3, barrier);
            if density + barrier23 > 0.0 {
                return None;
            }
        }
        Some(fluid_state)
    }

    #[allow(clippy::too_many_arguments)]
    fn pressure(
        &self,
        x: i32,
        y: i32,
        z: i32,
        noise: &mut Option<f64>,
        a: FluidStatus,
        b: FluidStatus,
        barrier: &mut dyn FnMut(i32, i32, i32) -> f64,
    ) -> f64 {
        let (type_a, type_b) = (a.at(y), b.at(y));
        if (type_a == self.lava && type_b == self.water)
            || (type_a == self.water && type_b == self.lava)
        {
            return 2.0;
        }
        let level_diff = (a.level - b.level).abs();
        if level_diff == 0 {
            return 0.0;
        }
        let average = 0.5 * f64::from(a.level + b.level);
        let above_average = f64::from(y) + 0.5 - average;
        let towards_middle = f64::from(level_diff) / 2.0 - above_average.abs();
        let gradient = if above_average > 0.0 {
            towards_middle / if towards_middle > 0.0 { 1.5 } else { 2.5 }
        } else {
            let centre = 3.0 + towards_middle;
            centre / if centre > 0.0 { 3.0 } else { 10.0 }
        };
        let noise_value = if (-2.0..=2.0).contains(&gradient) {
            *noise.get_or_insert_with(|| barrier(x, y, z))
        } else {
            0.0
        };
        2.0 * (noise_value + gradient)
    }

    /// The window of the cell `p` anchors to, for any `p` with that anchor.
    pub fn window(&mut self, anchor: IVec3) -> Window {
        if self.config.is_none() {
            return Window::of(self.global.sea);
        }
        let slot = flat(
            anchor - self.min_grid - IVec3::Y,
            self.grid_size - IVec3::new(1, 2, 1),
        );
        if let Some(window) = self.windows[slot] {
            return window;
        }
        let mut fold: Option<Window> = None;
        for dx in 0..=1 {
            for dy in -1..=1 {
                for dz in 0..=1 {
                    let index = self.cell_index(anchor + IVec3::new(dx, dy, dz));
                    let cell = Window::of(self.status(index));
                    fold = Some(fold.map_or(cell, |acc| acc.merge(cell)));
                }
            }
        }
        let window = fold.expect("a window covers twelve cells");
        self.windows[slot] = Some(window);
        window
    }

    #[inline]
    fn anchor(x: i32, y: i32, z: i32) -> IVec3 {
        IVec3::new(
            (x + SAMPLE_OFFSET_X) >> X_SPACING_SHIFT,
            floor_div(y + SAMPLE_OFFSET_Y, Y_SPACING),
            (z + SAMPLE_OFFSET_Z) >> X_SPACING_SHIFT,
        )
    }

    /// [`Self::substance`] for a block with non-positive density, answered by
    /// its cell's window where a lemma applies and by the search otherwise.
    pub fn substance_settled(
        &mut self,
        x: i32,
        y: i32,
        z: i32,
        density: f64,
        barrier: &mut dyn FnMut(i32, i32, i32) -> f64,
    ) -> Substance {
        debug_assert!(density <= 0.0);
        let at = IVec3::new(x, y, z);
        match self.settle(at, at) {
            Some(status) => Some(status.at(y)),
            None => self.substance(x, y, z, density, barrier),
        }
    }

    fn margins(&self) -> (i32, i32) {
        let config = self.config.expect("margins settle nothing without a field");
        (config.margin_above, config.margin_below)
    }

    /// One status answering every block of `[min, max]`, both inclusive, for a
    /// box whose density is non-positive throughout. `None` where the box needs
    /// the per-block search.
    ///
    /// The status holds outside the box no more than the lemma that produced it
    /// does: a dry box answers with the level at its own floor and a drowned one
    /// with the level just past its own ceiling.
    pub fn settle(&mut self, min: IVec3, max: IVec3) -> Option<FluidStatus> {
        if max.y < self.global.lava_below {
            return Some(FluidStatus {
                level: max.y + 1,
                fluid: self.lava,
            });
        }
        if min.y < self.global.lava_below {
            return None;
        }
        let lo = Self::anchor(min.x, min.y, min.z);
        let hi = Self::anchor(max.x, max.y, max.z);
        let mut fold: Option<Window> = None;
        for gx in lo.x..=hi.x {
            for gy in lo.y..=hi.y {
                for gz in lo.z..=hi.z {
                    let window = self.window(IVec3::new(gx, gy, gz));
                    fold = Some(fold.map_or(window, |acc| acc.merge(window)));
                }
            }
        }
        let window = fold.expect("a box anchors to at least one window");
        if let Some(status) = window.uniform() {
            return Some(status);
        }
        let (margin_above, margin_below) = self.margins();
        if min.y >= window.lmax.saturating_add(margin_above) {
            return Some(FluidStatus {
                level: min.y,
                fluid: AIR,
            });
        }
        if let Some(fluid) = window.one_type
            && max.y <= window.lmin - margin_below
        {
            return Some(FluidStatus {
                level: max.y + 1,
                fluid,
            });
        }
        None
    }
}

/// The barrier noise one point at a time, for a caller that reads it in
/// scattered places rather than over a box.
pub fn point_barrier<'a>(
    router: &'a NoiseRouter,
    ws: &'a mut Workspace,
) -> impl FnMut(i32, i32, i32) -> f64 + 'a {
    let root = router.aquifer.as_ref().map(|aquifer| aquifer.barrier);
    move |x, y, z| {
        let root = root.expect("only a field with a barrier asks for it");
        let mut out = [0.0f32];
        router
            .program
            .fill(ws, &Volume::point(IVec3::new(x, y, z)), root, &mut out);
        f64::from(out[0])
    }
}

#[inline]
fn flat(at: IVec3, size: IVec3) -> usize {
    debug_assert!(
        at.cmpge(IVec3::ZERO).all() && at.cmplt(size).all(),
        "{at} lies outside the lattice region {size}"
    );
    ((at.y * size.z + at.z) * size.x + at.x) as usize
}

#[inline]
fn similarity(distance1: i32, distance2: i32) -> f64 {
    1.0 - f64::from(distance2 - distance1) / 25.0
}
