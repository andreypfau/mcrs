use crate::biome_upscale::upscale_biomes;
use crate::heightmap::{HeightmapKinds, HeightmapPredicates};
use crate::multi_noise_biomes::{BiomeGrid, MultiNoiseBiomeTable};
use crate::task::CancellationToken;
use bevy_math::IVec3;
use mcrs_minecraft_biome::climate::TargetPoint;
use mcrs_minecraft_biome::source::{BetaLandBiome, BiomeSource, beta_biome_from_climate};
use mcrs_minecraft_biome::zoom::{FiddleCache, obfuscate_seed};
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_block_predicate::predicate::HeightmapName;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_level::palette::{BiomePalette, BlockPalette};
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_registry::{BlockStateId, NarrowError};
use mcrs_minecraft_worldgen::beard::Beard;
use mcrs_minecraft_worldgen_density::aquifer::{FluidField, FluidStatus};
use mcrs_minecraft_worldgen_density::cell::{CELL_BOUNDS_SLACK, corner_bounds, sampled_bounds};
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_density::router::{
    CONTINENTS, DEPTH, EROSION, FINAL_DENSITY, NoiseRouter, RIDGES, TEMPERATURE, VEGETATION,
};
use mcrs_minecraft_worldgen_noise::interval::Interval;
use mcrs_minecraft_worldgen_noise::sample_grid::SampleGrid;
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

/// The `interpolated` wrapper inputs at every cell corner of a whole chunk
/// column, laid out one `volume`-shaped row per wrapper.
struct CellLattice {
    volume: SampleGrid,
    cell: IVec3,
    values: Vec<f32>,
    width: usize,
}

/// The lattice columns recently produced, so a column reads the two planes it
/// shares with the neighbours before it instead of evaluating them again.
///
/// A node's value is a function of the node and the router alone, never of the
/// volume it was asked about as part of, so the shared plane is the same plane
/// (`docs/worldgen.md` §4, L3). A column produces 5 x 49 x 5 nodes and shares
/// its low planes in x and z, which is nine of the twenty-five node columns:
/// what stays is a 4 x 49 x 4 evaluation, 64% of the work.
///
/// Two generations rather than one map: the useful life of a column is until
/// the neighbours past it have been filled, and rotating keeps at least
/// `KEPT_COLUMNS` of the most recent without ever growing without bound. A
/// worker that jumps between distant columns misses and pays what it paid
/// before.
#[derive(Default)]
struct LatticePlanes {
    /// Which router these belong to. One worker fills columns for every
    /// dimension, and a node's value differs by seed and by graph.
    owner: Option<(usize, u64)>,
    current: HashMap<i64, Box<[f32]>>,
    previous: HashMap<i64, Box<[f32]>>,
}

const KEPT_COLUMNS: usize = 2048;

impl LatticePlanes {
    fn retarget(&mut self, router: &NoiseRouter) {
        let owner = (std::ptr::from_ref(router) as usize, router.world_seed);
        if self.owner != Some(owner) {
            self.owner = Some(owner);
            self.current.clear();
            self.previous.clear();
        }
    }

    fn get(&self, key: i64) -> Option<&[f32]> {
        self.current
            .get(&key)
            .or_else(|| self.previous.get(&key))
            .map(Box::as_ref)
    }

    fn put(&mut self, key: i64, column: &[f32]) {
        if self.current.len() >= KEPT_COLUMNS {
            std::mem::swap(&mut self.current, &mut self.previous);
            self.current.clear();
        }
        self.current.insert(key, column.into());
    }
}

thread_local! {
    static LATTICE_PLANES: RefCell<LatticePlanes> = RefCell::new(LatticePlanes::default());
}

/// The key one node column answers to. Node coordinates are block coordinates,
/// which is what makes two columns agree on the plane between them.
fn plane_key(node_x: i32, node_z: i32) -> i64 {
    (i64::from(node_x) << 32) | i64::from(node_z as u32)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CellFill {
    Solid,
    /// Empty under one fluid status: its fluid below its level, air above.
    Uniform(FluidStatus),
    Mixed,
}

impl CellLattice {
    /// `None` for a router with no single cell lattice, or one whose cells do
    /// not tile a section; such a chunk is filled block by block throughout.
    fn fill(
        noise_router: &NoiseRouter,
        block_x: i32,
        block_z: i32,
        ws: &mut Workspace,
    ) -> Option<Self> {
        let cell = noise_router.cell_size()?;
        // Sections must tile into whole cells, and the lattice must start on
        // one, or the eight values below would not be a cell's corners and the
        // interval bound over them would not hold.
        if 16 % cell.x != 0 || 16 % cell.y != 0 || 16 % cell.z != 0 {
            return None;
        }
        let height = noise_router.noise.height as i32;
        if noise_router.noise.min_y % 16 != 0 || height % 16 != 0 {
            return None;
        }
        let volume = SampleGrid::new(
            IVec3::new(16 / cell.x + 1, height / cell.y + 1, 16 / cell.z + 1),
            IVec3::new(block_x, noise_router.noise.min_y, block_z),
            cell,
        );
        let inputs = noise_router.cell_inputs();
        let width = inputs.len();
        let rows = volume.size().y as usize;
        let (nx, nz) = (volume.size().x, volume.size().z);
        let mut values = vec![0.0f32; width * volume.len()];

        // The node columns a neighbour already produced: the whole low plane in
        // x, and the low plane in z above it.
        let borrowed = (0..nz)
            .map(|z| (0, z))
            .chain((1..nx).map(|x| (x, 0)))
            .collect::<Vec<_>>();
        let shared = LATTICE_PLANES.with_borrow_mut(|planes| {
            planes.retarget(noise_router);
            borrowed.iter().all(|&(x, z)| {
                let Some(column) = planes.get(plane_key(volume.block_x(x), volume.block_z(z)))
                else {
                    return false;
                };
                for k in 0..width {
                    let at = k * volume.len() + volume.index_unchecked(x, 0, z);
                    values[at..at + rows].copy_from_slice(&column[k * rows..(k + 1) * rows]);
                }
                true
            })
        });

        if shared {
            let inner = SampleGrid::new(
                IVec3::new(nx - 1, volume.size().y, nz - 1),
                volume.min_block() + IVec3::new(cell.x, 0, cell.z),
                cell,
            );
            let mut inner_values = vec![0.0f32; width * inner.len()];
            noise_router.fill_nodes(ws, &inner, inputs, &mut inner_values);
            for z in 0..nz - 1 {
                for x in 0..nx - 1 {
                    for k in 0..width {
                        let from = k * inner.len() + inner.index_unchecked(x, 0, z);
                        let to = k * volume.len() + volume.index_unchecked(x + 1, 0, z + 1);
                        values[to..to + rows].copy_from_slice(&inner_values[from..from + rows]);
                    }
                }
            }
        } else {
            noise_router.fill_nodes(ws, &volume, inputs, &mut values);
        }

        // The high planes, which the neighbours after this column read as their
        // own low ones.
        let mut column = vec![0.0f32; width * rows];
        LATTICE_PLANES.with_borrow_mut(|planes| {
            for (x, z) in (0..nz)
                .map(|z| (nx - 1, z))
                .chain((0..nx - 1).map(|x| (x, nz - 1)))
            {
                for k in 0..width {
                    let at = k * volume.len() + volume.index_unchecked(x, 0, z);
                    column[k * rows..(k + 1) * rows].copy_from_slice(&values[at..at + rows]);
                }
                planes.put(plane_key(volume.block_x(x), volume.block_z(z)), &column);
            }
        });

        noise_router.pin_cell_lattice(ws, &volume, &values);
        Some(Self {
            volume,
            cell,
            values,
            width,
        })
    }

    fn classify(
        &self,
        noise_router: &NoiseRouter,
        at: IVec3,
        fluid: &mut FluidField<'_>,
        fill: &mut FillBuffers,
    ) -> CellFill {
        corner_bounds(&self.values, &self.volume, at, &mut fill.corners);
        let min = IVec3::new(
            self.volume.block_x(at.x),
            self.volume.block_y(at.y),
            self.volume.block_z(at.z),
        );
        let Some(bounds) = noise_router.final_density_cell_bounds(
            &fill.corners,
            min,
            min + self.cell - 1,
            &mut fill.cell_terms,
        ) else {
            return CellFill::Mixed;
        };
        match self.verdict(bounds, at, fluid) {
            CellFill::Mixed => {}
            settled => return settled,
        }

        // The hull bounds the closed cell, and the blocks stop one lattice step
        // short of its far face. Asking again over the range they do reach is
        // exact and settles cells the hull leaves open; it runs only here, on
        // the few per column the first pass could not answer.
        sampled_bounds(&self.values, &self.volume, at, self.cell, &mut fill.corners);
        let Some(bounds) = noise_router.final_density_cell_bounds(
            &fill.corners,
            min,
            min + self.cell - 1,
            &mut fill.cell_terms,
        ) else {
            return CellFill::Mixed;
        };
        self.verdict(bounds, at, fluid)
    }

    /// A void cell is settled only where the fluid field is settled over it:
    /// the barrier adds to the block's density, not to the cell's bound.
    fn verdict(&self, bounds: Interval, at: IVec3, fluid: &mut FluidField<'_>) -> CellFill {
        if bounds.min() > CELL_BOUNDS_SLACK {
            return CellFill::Solid;
        }
        if bounds.max() < -CELL_BOUNDS_SLACK {
            let min = IVec3::new(
                self.volume.block_x(at.x),
                self.volume.block_y(at.y),
                self.volume.block_z(at.z),
            );
            return match fluid.settle(min, min + self.cell - 1) {
                Some(status) => CellFill::Uniform(status),
                None => CellFill::Mixed,
            };
        }
        CellFill::Mixed
    }
}

/// A strip whose blocks are all air.
pub const NO_TOP: i32 = i32::MIN;

/// What one dense fill of a column leaves behind besides the blocks themselves.
pub struct FilledColumn<'a> {
    pub biomes: Vec<BiomePalette>,
    /// Highest non-air block per strip, indexed `z * 16 + x`, `NO_TOP` where
    /// the strip holds none.
    pub tops: [i32; 256],
    /// Whether the material rules run for the column: a multi-noise source
    /// with a table and a fixed source, never a Beta column.
    pub material_surface: bool,
    /// The fluid field over the column, for the stages after the fill that
    /// still ask it.
    pub fluid: FluidField<'a>,
}

/// Buffers every fill in a chunk column reuses.
#[derive(Default)]
struct FillBuffers {
    ws: Workspace,
    density: Vec<f32>,
    barrier: Vec<f32>,
    corners: Vec<Interval>,
    cell_terms: Vec<Interval>,
}

/// Place `final_density` over a whole chunk column.
///
/// The corner lattice is the pass-through case of the fill — every
/// `interpolated` wrapper hands its input straight back — so one fill over the
/// column settles most cells outright from interval arithmetic over their eight
/// corners, and only the rest are filled block by block.
///
/// Returns `false` if the column was cancelled part-way.
#[allow(clippy::too_many_arguments)]
fn fill_column(
    column: &ColumnBlocks,
    block_x: i32,
    block_z: i32,
    noise_router: &NoiseRouter,
    tops: &mut [i32; 256],
    fluid: &mut FluidField<'_>,
    beard: Option<&Beard>,
    cancel: &CancellationToken,
) -> bool {
    let default_block = noise_router.default_block_state;
    let mut fill = FillBuffers::default();

    let Some(lattice) = CellLattice::fill(noise_router, block_x, block_z, &mut fill.ws) else {
        return fill_column_dense(
            column,
            block_x,
            block_z,
            noise_router,
            tops,
            fluid,
            beard,
            &mut fill,
            cancel,
        );
    };

    let cell = lattice.cell;
    fill.corners.resize(lattice.width, Interval::exact(0.0));

    for cell_z in 0..lattice.volume.size().z - 1 {
        if cancel.is_cancelled() {
            return false;
        }
        for cell_x in 0..lattice.volume.size().x - 1 {
            for cell_y in (0..lattice.volume.size().y - 1).rev() {
                let at = IVec3::new(cell_x, cell_y, cell_z);
                let world = IVec3::new(
                    lattice.volume.block_x(cell_x),
                    lattice.volume.block_y(cell_y),
                    lattice.volume.block_z(cell_z),
                );
                let Some(index) = column.section_index(world.y) else {
                    continue;
                };
                let base = IVec3::new(cell_x * cell.x, world.y.rem_euclid(16), cell_z * cell.z);
                let grid = SampleGrid::dense(cell, world);
                // The interval bound knows nothing of the beard, which is exactly
                // zero only outside its affected box.
                let verdict = if beard.is_some_and(|beard| beard.intersects(&grid)) {
                    CellFill::Mixed
                } else {
                    lattice.classify(noise_router, at, fluid, &mut fill)
                };
                match verdict {
                    CellFill::Solid => {
                        fill_cell_box(column, index, base, cell, default_block);
                        record_tops(tops, base, cell, world.y + cell.y - 1);
                    }
                    CellFill::Uniform(status) => {
                        let height = (status.level - world.y).clamp(0, cell.y);
                        if height > 0 {
                            fill_cell_box(
                                column,
                                index,
                                base,
                                IVec3::new(cell.x, height, cell.z),
                                status.fluid,
                            );
                            record_tops(tops, base, cell, world.y + height - 1);
                        }
                    }
                    CellFill::Mixed => fill_blocks(
                        column,
                        index,
                        &grid,
                        base,
                        noise_router,
                        tops,
                        fluid,
                        beard,
                        &mut fill,
                    ),
                }
            }
        }
    }
    true
}

/// The block-by-block fallback for a router whose cells do not tile a section.
#[allow(clippy::too_many_arguments)]
fn fill_column_dense(
    column: &ColumnBlocks,
    block_x: i32,
    block_z: i32,
    noise_router: &NoiseRouter,
    tops: &mut [i32; 256],
    fluid: &mut FluidField<'_>,
    beard: Option<&Beard>,
    fill: &mut FillBuffers,
    cancel: &CancellationToken,
) -> bool {
    let noise_min_y = noise_router.noise.min_y;
    let noise_max_y = noise_min_y + noise_router.noise.height as i32;
    for (index, &section_y) in column.y_sections().iter().enumerate() {
        if cancel.is_cancelled() {
            return false;
        }
        let section_min_y = section_y * 16;
        if section_min_y >= noise_max_y || section_min_y + 16 <= noise_min_y {
            continue;
        }
        let volume = SampleGrid::dense(
            IVec3::splat(16),
            IVec3::new(block_x, section_min_y, block_z),
        );
        fill_blocks(
            column,
            index,
            &volume,
            IVec3::ZERO,
            noise_router,
            tops,
            fluid,
            beard,
            fill,
        );
    }
    true
}

/// Settle every strip a whole-class cell covers at the highest block its box
/// actually reaches, which for a sea cell is one below sea level rather than
/// the cell top.
fn record_tops(tops: &mut [i32; 256], base: IVec3, cell: IVec3, top: i32) {
    for z in base.z..base.z + cell.z {
        for x in base.x..base.x + cell.x {
            let slot = &mut tops[(z * 16 + x) as usize];
            *slot = (*slot).max(top);
        }
    }
}

fn fill_cell_box(column: &ColumnBlocks, index: usize, base: IVec3, cell: IVec3, state: VoxelId) {
    column.fill_box_in_section(
        index,
        base.x,
        base.x + cell.x,
        base.y,
        base.y + cell.y,
        base.z,
        base.z + cell.z,
        state,
    );
}

#[allow(clippy::too_many_arguments)]
fn fill_blocks(
    column: &ColumnBlocks,
    index: usize,
    volume: &SampleGrid,
    origin: IVec3,
    noise_router: &NoiseRouter,
    tops: &mut [i32; 256],
    fluid: &mut FluidField<'_>,
    beard: Option<&Beard>,
    fill: &mut FillBuffers,
) {
    let default_block = noise_router.default_block_state;
    let FillBuffers {
        ws,
        density,
        barrier,
        ..
    } = fill;
    density.clear();
    density.resize(volume.len(), 0.0);
    noise_router
        .program
        .fill(ws, volume, FINAL_DENSITY, density);
    if let Some(beard) = beard {
        beard.fill(volume, density);
    }
    let mut barrier_at = volume_barrier(noise_router, ws, volume, barrier);
    for z in 0..volume.size().z {
        for x in 0..volume.size().x {
            for y in (0..volume.size().y).rev() {
                let value = density[volume.index_unchecked(x, y, z)];
                let (px, py, pz) = (origin.x + x, origin.y + y, origin.z + z);
                let world = IVec3::new(volume.block_x(x), volume.block_y(y), volume.block_z(z));
                let state = column_block(
                    default_block,
                    value,
                    fluid,
                    world.x,
                    world.y,
                    world.z,
                    &mut barrier_at,
                );
                if state != VoxelId(0) {
                    column.set_in_section(index, px, py, pz, state);
                    let slot = &mut tops[(pz * 16 + px) as usize];
                    *slot = (*slot).max(world.y);
                }
            }
        }
    }
}

/// The barrier noise over the whole box, sampled on the first block whose
/// pressure asks for it; the shell asks in runs, one point at a time would
/// pay the fill's setup per block.
fn volume_barrier<'a>(
    noise_router: &'a NoiseRouter,
    ws: &'a mut Workspace,
    volume: &'a SampleGrid,
    barrier: &'a mut Vec<f32>,
) -> impl FnMut(i32, i32, i32) -> f64 + 'a {
    barrier.clear();
    let barrier_root = noise_router.aquifer.as_ref().map(|aquifer| aquifer.barrier);
    move |bx: i32, by: i32, bz: i32| {
        if barrier.is_empty() {
            barrier.resize(volume.len(), 0.0);
            let root = barrier_root.expect("only a field with a barrier asks for it");
            noise_router.program.fill(ws, volume, root, barrier);
        }
        let at = volume
            .index_of_block(bx, by, bz)
            .expect("the barrier is read inside the box being filled");
        f64::from(barrier[at])
    }
}

/// The block the terrain fill places at one position: `VoxelId(0)` is air.
#[allow(clippy::too_many_arguments)]
fn column_block(
    default_block: VoxelId,
    density: f32,
    fluid: &mut FluidField<'_>,
    x: i32,
    y: i32,
    z: i32,
    barrier: &mut dyn FnMut(i32, i32, i32) -> f64,
) -> VoxelId {
    if density > 0.0 {
        return default_block;
    }
    fluid
        .substance_settled(x, y, z, f64::from(density), barrier)
        .unwrap_or(default_block)
}

pub fn heightmap_kind(name: HeightmapName) -> HeightmapKinds {
    match name {
        HeightmapName::WorldSurfaceWg | HeightmapName::WorldSurface => HeightmapKinds::SURFACE,
        HeightmapName::OceanFloorWg | HeightmapName::OceanFloor => HeightmapKinds::SOLID,
        HeightmapName::MotionBlocking => HeightmapKinds::MOTION,
        HeightmapName::MotionBlockingNoLeaves => HeightmapKinds::NO_LEAVES,
    }
}

/// What the unfilled terrain column answers a heightmap with: any block for
/// the surface maps, solid terrain alone for every other, fluids never.
pub fn first_free_kind(name: HeightmapName) -> HeightmapKinds {
    match name {
        HeightmapName::WorldSurfaceWg | HeightmapName::WorldSurface => HeightmapKinds::SURFACE,
        _ => HeightmapKinds::SOLID,
    }
}

/// One above the topmost block of the unfilled terrain column at `(x, z)`
/// satisfying `kind`, over the noise range clipped to the accessor range, or
/// the floor of that range when no block does.
#[allow(clippy::too_many_arguments)]
pub fn base_height(
    router: &NoiseRouter,
    ws: &mut Workspace,
    predicates: &HeightmapPredicates,
    kind: HeightmapKinds,
    x: i32,
    z: i32,
    accessor_min_y: i32,
    accessor_height: i32,
) -> i32 {
    let (min_y, column) = base_column(router, ws, x, z, accessor_min_y, accessor_height);
    column
        .iter()
        .rposition(|state| predicates.get(*state).contains(kind))
        .map_or(min_y, |index| min_y + index as i32 + 1)
}

/// `getBaseColumn`: the unfilled terrain column at `(x, z)` over the noise
/// range clipped to the accessor range, bottom up from the returned floor.
pub fn base_column(
    router: &NoiseRouter,
    ws: &mut Workspace,
    x: i32,
    z: i32,
    accessor_min_y: i32,
    accessor_height: i32,
) -> (i32, Vec<VoxelId>) {
    let min_y = router.noise.min_y.max(accessor_min_y);
    let height = (router.noise.min_y + router.noise.height as i32)
        .min(accessor_min_y + accessor_height)
        - min_y;
    if height <= 0 {
        return (min_y, Vec::new());
    }
    let volume = SampleGrid::dense(IVec3::new(1, height, 1), IVec3::new(x, min_y, z));
    let mut density = vec![0.0f32; volume.len()];
    router
        .program
        .fill(ws, &volume, FINAL_DENSITY, &mut density);
    let mut fluid = FluidField::new(
        router,
        IVec3::new(x, min_y, z),
        IVec3::new(x, min_y + height - 1, z),
    );
    let mut barrier_buffer = Vec::new();
    let mut barrier_at = volume_barrier(router, ws, &volume, &mut barrier_buffer);
    let mut column = vec![VoxelId(0); height as usize];
    for y in (0..height).rev() {
        column[y as usize] = column_block(
            router.default_block_state,
            density[volume.index_unchecked(0, y, 0)],
            &mut fluid,
            x,
            volume.block_y(y),
            z,
            &mut barrier_at,
        );
    }
    (min_y, column)
}

/// What a column's biomes are read from, every biome id already narrowed to
/// the byte a palette stores.
#[derive(Clone, Default)]
pub enum ColumnBiomes {
    #[default]
    None,
    MultiNoise(Arc<MultiNoiseBiomeTable>),
    Fixed(u8),
    Beta(BetaBiomes),
}

#[derive(Clone)]
pub struct BetaBiomes {
    land: [u8; 11],
    lookup: Arc<[[BetaLandBiome; 64]; 64]>,
}

impl BetaBiomes {
    #[inline]
    pub fn biome_at(&self, temperature: f32, rain: f32) -> u8 {
        self.land[beta_biome_from_climate(&self.lookup, temperature, rain) as usize]
    }
}

impl ColumnBiomes {
    /// A multi-noise source answers through `multi_noise`, and with no table
    /// it answers nothing.
    pub fn new(
        source: Option<&BiomeSource>,
        multi_noise: Option<Arc<MultiNoiseBiomeTable>>,
    ) -> Result<Self, NarrowError> {
        Ok(match source {
            Some(BiomeSource::MultiNoise(_)) => multi_noise.map_or(Self::None, Self::MultiNoise),
            Some(BiomeSource::Fixed { biome }) => Self::Fixed(biome.narrow()?),
            Some(BiomeSource::Beta {
                land_biomes,
                lookup,
            }) => {
                let mut land = [0; 11];
                for (stored, biome) in land.iter_mut().zip(land_biomes) {
                    *stored = biome.narrow()?;
                }
                Self::Beta(BetaBiomes {
                    land,
                    lookup: Arc::clone(lookup),
                })
            }
            _ => Self::None,
        })
    }
}

/// One `BiomePalette` per section of the column, empty when no source can
/// answer for the position.
///
/// Every source stores block-resolution containers: a quart grid is upscaled
/// with the game's zoom, and a fixed source is one value per section.
fn column_biome_palettes(
    noise_router: &NoiseRouter,
    biomes: &ColumnBiomes,
    block_x: i32,
    block_z: i32,
    y_sections: &[i32],
) -> (Vec<BiomePalette>, bool) {
    match biomes {
        ColumnBiomes::MultiNoise(table) => (
            multi_noise_palettes(noise_router, table, block_x, block_z, y_sections),
            true,
        ),
        ColumnBiomes::Fixed(biome) => (
            vec![BiomePalette::homogeneous(*biome); y_sections.len()],
            true,
        ),
        ColumnBiomes::Beta(beta) => {
            let grid = beta_biome_grid(noise_router, beta, block_x, block_z);
            (upscale_column(&grid, noise_router, y_sections), false)
        }
        ColumnBiomes::None => (vec![BiomePalette::default(); y_sections.len()], false),
    }
}

fn upscale_column(
    grid: &BiomeGrid,
    noise_router: &NoiseRouter,
    y_sections: &[i32],
) -> Vec<BiomePalette> {
    thread_local! {
        static FIDDLE: RefCell<FiddleCache> = RefCell::new(FiddleCache::default());
    }
    let zoom_seed = obfuscate_seed(noise_router.world_seed as i64);
    FIDDLE.with_borrow_mut(|fiddle| upscale_biomes(grid, zoom_seed, y_sections, fiddle))
}

/// The stored containers of a multi-noise column: the quart grid of
/// `multi_noise_grid` upscaled to block resolution.
pub fn multi_noise_palettes(
    noise_router: &NoiseRouter,
    table: &MultiNoiseBiomeTable,
    block_x: i32,
    block_z: i32,
    y_sections: &[i32],
) -> Vec<BiomePalette> {
    let Some(grid) = multi_noise_grid(noise_router, table, block_x, block_z, y_sections) else {
        return Vec::new();
    };
    upscale_column(&grid, noise_router, y_sections)
}

/// The quart grid of a multi-noise column: its own rows, and one cell of ring
/// on each horizontal side.
///
/// The ring is horizontal: above and below the column a pick reads the edge
/// row, so the grid holds the column's own rows only.
///
/// The density program fills a whole strided volume in one pass, so asking for
/// the column's cells at once costs a fraction of evaluating them one by one.
pub fn multi_noise_grid(
    noise_router: &NoiseRouter,
    table: &MultiNoiseBiomeTable,
    block_x: i32,
    block_z: i32,
    y_sections: &[i32],
) -> Option<BiomeGrid> {
    let (&first, &last) = (y_sections.first()?, y_sections.last()?);
    let volume = SampleGrid::new(
        IVec3::new(6, (last - first + 1) * 4, 6),
        IVec3::new(block_x - 4, first * 16, block_z - 4),
        IVec3::splat(4),
    );
    let roots = [TEMPERATURE, VEGETATION, CONTINENTS, EROSION, DEPTH, RIDGES];
    let points = volume.len();
    let mut values = vec![0.0f32; roots.len() * points];
    let mut ws = Workspace::new();
    noise_router.fill_roots(&mut ws, &volume, &roots, &mut values);

    let mut last = None;
    let ids: Vec<u8> = (0..points)
        .map(|at| {
            table.biome_at_from(
                TargetPoint::new(
                    values[at],
                    values[points + at],
                    values[2 * points + at],
                    values[3 * points + at],
                    values[4 * points + at],
                    values[5 * points + at],
                ),
                &mut last,
            )
        })
        .collect();

    Some(BiomeGrid { volume, ids })
}

/// The quart grid of a Beta column: one row, its own sixteen cells and the ring
/// of twenty around them, each the biome its climate answers at the position
/// the column owning that cell samples it at.
pub fn beta_biome_grid(
    noise_router: &NoiseRouter,
    beta: &BetaBiomes,
    block_x: i32,
    block_z: i32,
) -> BiomeGrid {
    let volume = SampleGrid::new(
        IVec3::new(6, 1, 6),
        IVec3::new(block_x - 4, 0, block_z - 4),
        IVec3::new(4, 1, 4),
    );
    let points = volume.len();
    let mut values = vec![0.0f32; 2 * points];
    noise_router.fill_roots(
        &mut Workspace::new(),
        &volume,
        &[TEMPERATURE, VEGETATION],
        &mut values,
    );
    let ids = (0..points)
        .map(|at| beta.biome_at(values[at], values[points + at]))
        .collect();
    BiomeGrid { volume, ids }
}

/// The per-chunk seed Beta derives for its population passes: two odd multipliers
/// drawn once from the world seed, dotted with the chunk coordinate.
///
/// Caves feed it the *neighbouring* chunk they are carving from, ores the chunk
/// itself, so the coordinate is a parameter rather than the chunk under work.
pub fn beta_chunk_seed(world_seed: i64, chunk_x: i32, chunk_z: i32) -> i64 {
    let mut rng = LegacyRandom::new(world_seed as u64);
    let x_multiplier = rng.next_java_long() / 2 * 2 + 1;
    let z_multiplier = rng.next_java_long() / 2 * 2 + 1;
    (chunk_x as i64)
        .wrapping_mul(x_multiplier)
        .wrapping_add((chunk_z as i64).wrapping_mul(z_multiplier))
        ^ world_seed
}

/// Generate all sections in a column.
///
/// `final_density` is filled once over the whole column, then walked as a single
/// z, x, descending-y sweep over its cells.
///
/// A column `cancel` stops part-way returns `None` for every section: one
/// column-wide fill leaves no section boundary at which a partial result is
/// meaningful.
#[cfg_attr(
    feature = "telemetry-tracy",
    tracing::instrument(name = "world::column_gen", skip_all)
)]
pub fn generate_column(
    section_x: i32,
    section_z: i32,
    y_sections: &[i32],
    noise_router: &NoiseRouter,
    biomes: &ColumnBiomes,
    cancel: &CancellationToken,
) -> Vec<Option<(BlockPalette, BiomePalette)>> {
    let mut column = ColumnBlocks::new(y_sections);
    let Some(filled) = fill_column_dense_any(
        &mut column,
        section_x,
        section_z,
        y_sections,
        noise_router,
        biomes,
        None,
        cancel,
    ) else {
        return vec![None; y_sections.len()];
    };

    column.into_sections(&filled.biomes)
}

/// Fill a column densely from the density graph, for every preset.
///
/// Beta is data here like any other preset: `beta.json` describes its terrain as
/// density functions, so it runs the same graph, the same cell fill and the same
/// packing as the overworld. `None` means the column was cancelled.
#[allow(clippy::too_many_arguments)]
pub fn fill_column_dense_any<'a>(
    column: &mut ColumnBlocks,
    section_x: i32,
    section_z: i32,
    y_sections: &[i32],
    noise_router: &'a NoiseRouter,
    biomes: &ColumnBiomes,
    beard: Option<&Beard>,
    cancel: &CancellationToken,
) -> Option<FilledColumn<'a>> {
    let block_x = section_x * 16;
    let block_z = section_z * 16;
    let (biomes, material_surface) =
        column_biome_palettes(noise_router, biomes, block_x, block_z, y_sections);
    column.reset(y_sections);

    let mut tops = [NO_TOP; 256];
    let mut fluid = column_fluid_field(noise_router, block_x, block_z);
    if !fill_column(
        column,
        block_x,
        block_z,
        noise_router,
        &mut tops,
        &mut fluid,
        beard,
        cancel,
    ) {
        return None;
    }
    Some(FilledColumn {
        biomes,
        tops,
        material_surface,
        fluid,
    })
}

/// The fluid field over one chunk column of the dimension's whole height.
pub fn column_fluid_field(
    noise_router: &NoiseRouter,
    block_x: i32,
    block_z: i32,
) -> FluidField<'_> {
    let min_y = noise_router.noise.min_y;
    FluidField::new(
        noise_router,
        IVec3::new(block_x, min_y, block_z),
        IVec3::new(
            block_x + 15,
            min_y + noise_router.noise.height as i32 - 1,
            block_z + 15,
        ),
    )
}

/// Apply the Beta surface pass to a generated chunk column.
///
pub fn beta_surface_rng(chunk_x: i32, chunk_z: i32) -> LegacyRandom {
    LegacyRandom::large_feature_with_salt(0, chunk_x, chunk_z, 0)
}

/// Ports replaceBlocksForBiome from back2beta with a single per-chunk LegacyRandom
/// that drives both the surface depth, beach conditions, and the bedrock Y 0-4
/// probabilistic check — all interleaved in back2beta's exact column iteration order.
///
/// The caller seeds `rng` once per chunk with [`beta_surface_rng`].
/// `rng` must be threaded across section calls so the stream is continuous.
pub fn apply_beta_surface(
    column: &ColumnBlocks,
    block_x: i32,
    block_z: i32,
    noise_router: &NoiseRouter,
    biome_source: &BiomeSource,
    blocks: &BlockDefinitions,
    rng: &mut LegacyRandom,
) {
    let Some(beta) = noise_router.beta_noises() else {
        return;
    };

    // back2beta's replaceBlocksForBiome reads biomes via getBiomeFromLookup (quantized).
    let BiomeSource::Beta {
        lookup: beta_lookup,
        ..
    } = biome_source
    else {
        panic!("the Beta surface reads a Beta biome source");
    };

    let sea_level = noise_router.sea_level;
    let default_fluid = noise_router.default_fluid_state;
    let stone = noise_router.default_block_state;
    let bedrock = VoxelId::from(blocks.default_state_of(Block::Bedrock.id()).0);
    let sandstone = VoxelId::from(blocks.default_state_of(Block::Sandstone.id()).0);
    let gravel = VoxelId::from(blocks.default_state_of(Block::Gravel.id()).0);
    let ice = VoxelId::from(blocks.default_state_of(Block::Ice.id()).0);
    let sand = VoxelId::from(blocks.default_state_of(Block::Sand.id()).0);

    const D0: f64 = 0.03125;

    // Three 16x16 grids, indexed geographically as x * 16 + z. Beta permutes the
    // axes: world Z goes to the noise's y argument and its own z is pinned to
    // zero, so the walk is x outer and world Z inner, which is the index order
    // the arrays want anyway.
    //
    //   r: `n.a(r, i*16, jj*16, 0.0, 16, 16, 1, d0, d0, 1.0)`       sand and gravel
    //   s: `n.a(s, i*16, 109.0134, jj*16, 16, 1, 16, d0, 1.0, d0)`  gravel override
    //   t: `o.a(t, i*16, jj*16, 0.0, 16, 16, 1, d0*2, d0*2, d0*2)`  surface depth
    //
    // r and t go through the grid fill because Beta's reuse of a lattice cell's
    // corner dots makes that fill depend on the grid and not only on the
    // position. `s` has ySize 1, so Beta takes the branch that drops the y
    // offset entirely — the 109.0134 never reaches the lattice — and that branch
    // is an ordinary function of x and z.
    let (mut r, mut t) = ([0.0f32; 256], [0.0f32; 256]);
    let offset = [block_x as f64, block_z as f64, 0.0];
    beta.beach
        .fill_legacy_grid(&mut r, offset, [16, 16, 1], [D0, D0, 1.0]);
    beta.surface
        .fill_legacy_grid(&mut t, offset, [16, 16, 1], [D0 * 2.0, D0 * 2.0, D0 * 2.0]);

    let mut s = [0.0f32; 256];
    for x in 0..16i32 {
        for z in 0..16i32 {
            let (wx, wz) = ((block_x + x) as f64, (block_z + z) as f64);
            s[(x * 16 + z) as usize] = beta.beach_flat.get(wx * D0, 0.0, wz * D0);
        }
    }

    let mut ws = Workspace::new();
    let (temperatures, humidities) =
        noise_router.sample_beta_climate_grids(&mut ws, block_x, block_z);

    // back2beta replaceBlocksForBiome: outer loop kk=0..16 is Z, inner ll=0..16 is X.
    // Noise arrays r/s/t are filled at index x*16+z (geographic) and read at ll*16+kk
    // = x*16+z — the same geographic index. Climate is sampled at geographic (wx, wz).
    for z_local in 0..16i32 {
        for x_local in 0..16i32 {
            let idx = (x_local * 16 + z_local) as usize;

            // Three RNG draws per column matching Java's Random.nextDouble() exactly.
            let flag = r[idx] as f64 + rng.next_f64() * 0.2 > 0.0;
            let flag1 = s[idx] as f64 + rng.next_f64() * 0.2 > 3.0;
            let i1 = (t[idx] as f64 / 3.0 + 3.0 + rng.next_f64() * 0.25) as i32;

            let (temp, humidity) = (temperatures[idx], humidities[idx]);
            let biome_land: BetaLandBiome = beta_biome_from_climate(beta_lookup, temp, humidity);
            let (top_block, filler_block) = beta_surface_blocks(biome_land, blocks);
            let (top_block, filler_block) =
                (VoxelId::from(top_block.0), VoxelId::from(filler_block.0));

            // j1 in back2beta: depth counter, -1 means "not yet in surface layer".
            let mut j1: i32 = -1;
            // Mutable top/filler for current Y zone (back2beta: b1, b2).
            let mut b1 = top_block;
            let mut b2 = filler_block;
            let air = VoxelId(0);

            // Sweep from world Y=127 down to 0 (back2beta: k1 = 127..=0).
            // Bedrock check is interleaved inside this loop.
            for k1 in (0i32..=127).rev() {
                // Bedrock check (back2beta: k1 <= 0 + this.j.nextInt(5)).
                if k1 <= rng.next_i32_bound(5) {
                    column.set(x_local, k1, z_local, bedrock);
                } else {
                    let current_id = column.get(x_local, k1, z_local);

                    let current_id = match current_id {
                        Some(id) => id,
                        None => continue, // section not present — skip
                    };

                    if current_id == air {
                        j1 = -1;
                    } else if current_id == stone {
                        if j1 == -1 {
                            if i1 <= 0 {
                                b1 = air;
                                b2 = stone;
                            } else if k1 >= sea_level - 4 && k1 <= sea_level + 1 {
                                b1 = top_block;
                                b2 = filler_block;
                                if flag1 {
                                    b1 = air;
                                }
                                if flag1 {
                                    b2 = gravel;
                                }
                                if flag {
                                    b1 = sand;
                                }
                                if flag {
                                    b2 = sand;
                                }
                            }

                            if k1 < sea_level && b1 == air {
                                b1 = default_fluid;
                            }

                            j1 = i1;
                            let place = if k1 >= sea_level - 1 { b1 } else { b2 };
                            column.set(x_local, k1, z_local, place);
                        } else if j1 > 0 {
                            j1 -= 1;
                            column.set(x_local, k1, z_local, b2);
                            if j1 == 0 && b2 == sand {
                                j1 = rng.next_i32_bound(4);
                                b2 = sandstone;
                            }
                        }
                    }
                }
            }

            // back2beta fillDensityTerrain: if d17 < 0.5 && Y == sea_level-1, replace
            // water with ice. No RNG consumed — must stay after all bedrock draws.
            if temp < 0.5 {
                let ice_y = sea_level - 1;
                if column.get(x_local, ice_y, z_local) == Some(default_fluid) {
                    column.set(x_local, ice_y, z_local, ice);
                }
            }
        }
    }
}

fn beta_surface_blocks(
    biome: BetaLandBiome,
    blocks: &BlockDefinitions,
) -> (BlockStateId, BlockStateId) {
    match biome {
        BetaLandBiome::Desert | BetaLandBiome::IceDesert => {
            let sand = blocks.default_state_of(Block::Sand.id());
            (sand, sand)
        }
        _ => {
            let grass = blocks.default_state_of(Block::GrassBlock.id());
            let dirt = blocks.default_state_of(Block::Dirt.id());
            (grass, dirt)
        }
    }
}

pub mod block_state;
pub mod column_blocks;
pub mod heightmap;
pub mod saved;
pub mod task;
pub use column_blocks::ColumnBlocks;
pub mod beta_caves;
pub use beta_caves::{BetaCaveBlockIds, apply_beta_carvers};
pub mod beta_ores;
pub(crate) mod biome_upscale;
pub mod feature_program;
pub mod features;
pub mod ids;
pub mod modern_carvers;
pub mod multi_noise_biomes;
pub mod routers;
pub mod stages;
pub mod staging;
pub(crate) mod stored_biomes;
pub mod structures;
pub mod surface;
pub mod trees;
pub use beta_ores::{BetaOreBlockIds, place_all_ores};
pub use surface::{SurfaceIds, SurfaceStates, apply_material_surface, spans_dimension};

#[cfg(any(test, feature = "test-support"))]
pub mod tests;
