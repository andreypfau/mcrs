use crate::world::chunk::CancellationToken;
use crate::world::generate::multi_noise_biomes::{BiomeGrid, MultiNoiseBiomeTable};
use bevy_math::IVec3;
use mcrs_minecraft_block::palette::{BiomePalette, BlockPalette};
use mcrs_minecraft_core::RegistrySnapshot;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_world::biome::Biome;
use mcrs_minecraft_world::biome::beta_surface::beta_surface_blocks;
use mcrs_minecraft_world::biome::climate::TargetPoint;
use mcrs_minecraft_world::biome::source::{
    BetaLandBiome, BiomeSource, beta_biome_from_climate, beta_get_biome,
};
use mcrs_minecraft_world::block::definition::BlockDefinitions;
use mcrs_minecraft_worldgen::aquifer::{FluidField, FluidStatus};
use mcrs_minecraft_worldgen::cell::{CELL_BOUNDS_SLACK, sampled_range};
use mcrs_minecraft_worldgen::interval::Interval;
use mcrs_minecraft_worldgen::program::Workspace;
use mcrs_minecraft_worldgen::router::{
    CONTINENTS, DEPTH, EROSION, FINAL_DENSITY, NoiseRouter, RIDGES, TEMPERATURE, VEGETATION,
};
use mcrs_minecraft_worldgen::volume::Volume;
use mcrs_voxel_storage::VoxelId;
use std::cell::RefCell;
use std::collections::HashMap;

/// The `interpolated` wrapper inputs at every cell corner of a whole chunk
/// column, laid out one `volume`-shaped row per wrapper.
struct CellLattice {
    volume: Volume,
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
        let volume = Volume::new(
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
            let inner = Volume::new(
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

    /// The eight corner values of one cell, per wrapper.
    fn corner_bounds(&self, at: IVec3, out: &mut [Interval]) {
        let stride = self.volume.len();
        for (k, bound) in out.iter_mut().enumerate() {
            let row = &self.values[k * stride..(k + 1) * stride];
            let mut lo = f32::INFINITY;
            let mut hi = f32::NEG_INFINITY;
            for dz in 0..2 {
                for dx in 0..2 {
                    for dy in 0..2 {
                        let v = row[self.volume.index_unchecked(at.x + dx, at.y + dy, at.z + dz)];
                        lo = lo.min(v);
                        hi = hi.max(v);
                    }
                }
            }
            *bound = Interval::of(lo, hi);
        }
    }

    fn classify(
        &self,
        noise_router: &NoiseRouter,
        at: IVec3,
        fluid: &mut FluidField<'_>,
        fill: &mut FillBuffers,
    ) -> CellFill {
        self.corner_bounds(at, &mut fill.corners);
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
        self.sampled_bounds(at, &mut fill.corners);
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

    /// [`Self::corner_bounds`] over the range the blocks of the cell actually
    /// reach, rather than over the closed cell.
    fn sampled_bounds(&self, at: IVec3, out: &mut [Interval]) {
        let stride = self.volume.len();
        for (k, bound) in out.iter_mut().enumerate() {
            let row = &self.values[k * stride..(k + 1) * stride];
            *bound = sampled_range(self.cell, |dx, dy, dz| {
                row[self.volume.index_unchecked(at.x + dx, at.y + dy, at.z + dz)]
            });
        }
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
    /// `None` outside the multi-noise path, which is the only one that needs
    /// the zoom.
    pub biome_grid: Option<BiomeGrid>,
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
fn fill_column(
    column: &ColumnBlocks,
    block_x: i32,
    block_z: i32,
    noise_router: &NoiseRouter,
    tops: &mut [i32; 256],
    fluid: &mut FluidField<'_>,
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
                match lattice.classify(noise_router, at, fluid, &mut fill) {
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
                        &Volume::dense(cell, world),
                        base,
                        noise_router,
                        tops,
                        fluid,
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
        let volume = Volume::dense(
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
    volume: &Volume,
    origin: IVec3,
    noise_router: &NoiseRouter,
    tops: &mut [i32; 256],
    fluid: &mut FluidField<'_>,
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
    // The barrier noise over the whole box, sampled on the first block whose
    // pressure asks for it; the shell asks in runs, one point at a time would
    // pay the fill's setup per block.
    barrier.clear();
    let barrier_root = noise_router.aquifer.as_ref().map(|aquifer| aquifer.barrier);
    let mut barrier_at = |bx: i32, by: i32, bz: i32| {
        if barrier.is_empty() {
            barrier.resize(volume.len(), 0.0);
            let root = barrier_root.expect("only a field with a barrier asks for it");
            noise_router.program.fill(ws, volume, root, barrier);
        }
        let at = volume
            .index_of_block(bx, by, bz)
            .expect("the barrier is read inside the box being filled");
        f64::from(barrier[at])
    };
    for z in 0..volume.size().z {
        for x in 0..volume.size().x {
            for y in (0..volume.size().y).rev() {
                let value = density[volume.index_unchecked(x, y, z)];
                let (px, py, pz) = (origin.x + x, origin.y + y, origin.z + z);
                let world = IVec3::new(volume.block_x(x), volume.block_y(y), volume.block_z(z));
                let placed = if value > 0.0 {
                    Some(default_block)
                } else {
                    match fluid.substance_settled(
                        world.x,
                        world.y,
                        world.z,
                        f64::from(value),
                        &mut barrier_at,
                    ) {
                        None => Some(default_block),
                        Some(VoxelId(0)) => None,
                        state => state,
                    }
                };
                if let Some(state) = placed {
                    column.set_in_section(index, px, py, pz, state);
                    let slot = &mut tops[(pz * 16 + px) as usize];
                    *slot = (*slot).max(world.y);
                }
            }
        }
    }
}

/// The (temperature, humidity) pair at each of the sixteen biome-cell columns
/// of a chunk.
fn beta_climate_cells(noise_router: &NoiseRouter, block_x: i32, block_z: i32) -> [(f32, f32); 16] {
    let volume = Volume::new(
        IVec3::new(4, 1, 4),
        IVec3::new(block_x, 0, block_z),
        IVec3::new(4, 1, 4),
    );
    let mut values = vec![0.0f32; 2 * volume.len()];
    noise_router.fill_roots(
        &mut Workspace::new(),
        &volume,
        &[TEMPERATURE, VEGETATION],
        &mut values,
    );
    let mut cells = [(0.0f32, 0.0f32); 16];
    for cx in 0..4 {
        for cz in 0..4 {
            let source = volume.index_unchecked(cx, 0, cz);
            cells[(cx * 4 + cz) as usize] = (values[source], values[volume.len() + source]);
        }
    }
    cells
}

/// One `BiomePalette` per section of the column, empty when no source can
/// answer for the position.
///
/// The two sources answer at different resolutions: a Beta biome comes from
/// temperature and humidity at `(x, z)` alone, so one palette is cloned down
/// the column, while a multi-noise biome is sampled per 4x4x4 cell and every
/// section gets its own.
fn column_biome_palettes(
    noise_router: &NoiseRouter,
    biome_context: Option<(&BiomeSource, &RegistrySnapshot<Biome>)>,
    multi_noise: Option<&MultiNoiseBiomeTable>,
    block_x: i32,
    block_z: i32,
    y_sections: &[i32],
) -> (Vec<BiomePalette>, Option<BiomeGrid>) {
    if let Some((BiomeSource::MultiNoise(_), _)) = biome_context {
        return match multi_noise {
            Some(table) => multi_noise_palettes(noise_router, table, block_x, block_z, y_sections),
            None => (vec![BiomePalette::default(); y_sections.len()], None),
        };
    }
    if let Some((BiomeSource::Fixed { biome_id, .. }, registry)) = biome_context {
        return fixed_biome_palettes(biome_id.as_str(), registry, block_x, block_z, y_sections);
    }
    (
        vec![beta_biome_palette(noise_router, biome_context, block_x, block_z); y_sections.len()],
        None,
    )
}

/// The climate at every quart cell of the column and of the ring of cells
/// around it, resolved to a biome.
///
/// The density program fills a whole strided volume in one pass, so asking for
/// the column's cells at once costs a fraction of evaluating them one by one.
pub fn multi_noise_palettes(
    noise_router: &NoiseRouter,
    table: &MultiNoiseBiomeTable,
    block_x: i32,
    block_z: i32,
    y_sections: &[i32],
) -> (Vec<BiomePalette>, Option<BiomeGrid>) {
    let (Some(&first), Some(&last)) = (y_sections.first(), y_sections.last()) else {
        return (Vec::new(), None);
    };
    let volume = Volume::new(
        IVec3::new(6, (last - first + 1) * 4 + 2, 6),
        IVec3::new(block_x - 4, first * 16 - 4, block_z - 4),
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

    let palettes = y_sections
        .iter()
        .map(|&section_y| {
            let mut biomes = BiomePalette::default();
            let base_y = (section_y - first) * 4 + 1;
            for cx in 0..4 {
                for cy in 0..4 {
                    for cz in 0..4 {
                        let at = volume.index_unchecked(cx + 1, base_y + cy, cz + 1);
                        biomes.set_cell(cx as usize, cy as usize, cz as usize, ids[at]);
                    }
                }
            }
            biomes
        })
        .collect();

    (palettes, Some(BiomeGrid { volume, ids }))
}

/// The palettes and grid of a `minecraft:fixed` source, which answers the same
/// biome at every quart cell however it is asked.
///
/// The grid keeps the shape `multi_noise_palettes` produces — the column's
/// cells plus the ring around them — so the surface stage's zoom reads it
/// exactly as it reads a sampled one.
fn fixed_biome_palettes(
    biome_id: &str,
    registry: &RegistrySnapshot<Biome>,
    block_x: i32,
    block_z: i32,
    y_sections: &[i32],
) -> (Vec<BiomePalette>, Option<BiomeGrid>) {
    let (Some(&first), Some(&last)) = (y_sections.first(), y_sections.last()) else {
        return (Vec::new(), None);
    };
    let network_id = match registry
        .by_location(biome_id)
        .and_then(|id| u8::try_from(id).ok())
    {
        Some(id) => id,
        None => {
            // Returning no grid would skip the material stage entirely and hand
            // back a column of bare stone, so this degrades the way the Beta
            // palette degrades on the same failure rather than silently
            // dropping every rule the column owes.
            tracing::error!(
                biome = biome_id,
                "fixed biome is not in the registry snapshot"
            );
            debug_assert!(false, "unresolved fixed biome location");
            0
        }
    };

    let volume = Volume::new(
        IVec3::new(6, (last - first + 1) * 4 + 2, 6),
        IVec3::new(block_x - 4, first * 16 - 4, block_z - 4),
        IVec3::splat(4),
    );
    let ids = vec![network_id; volume.len()];

    let mut palette = BiomePalette::default();
    for cx in 0..4 {
        for cy in 0..4 {
            for cz in 0..4 {
                palette.set_cell(cx, cy, cz, network_id);
            }
        }
    }
    (
        vec![palette; y_sections.len()],
        Some(BiomeGrid { volume, ids }),
    )
}

/// The `BiomePalette` every section of a Beta column shares.
fn beta_biome_palette(
    noise_router: &NoiseRouter,
    biome_context: Option<(&BiomeSource, &RegistrySnapshot<Biome>)>,
    block_x: i32,
    block_z: i32,
) -> BiomePalette {
    let mut biomes = BiomePalette::default();
    let Some((biome_source, biome_registry)) =
        biome_context.filter(|(src, _)| matches!(src, BiomeSource::Beta { .. }))
    else {
        return biomes;
    };
    let climate = beta_climate_cells(noise_router, block_x, block_z);
    for cx in 0..4usize {
        for cz in 0..4usize {
            let (temp, humidity) = climate[cx * 4 + cz];
            let location = biome_source.beta_biome_location(temp, humidity, false);
            let network_id = match biome_registry.by_location(location.as_str()) {
                Some(id) => id as u8,
                None => {
                    // Falling back to id 0 renders a plausible-but-wrong biome, so a
                    // registry that cannot resolve a preset's own biome is loud.
                    tracing::error!(biome = %location.as_str(), "beta biome not present in registry snapshot");
                    debug_assert!(false, "unresolved beta biome location");
                    0
                }
            };
            for cy in 0..4usize {
                biomes.set_cell(cx, cy, cz, network_id);
            }
        }
    }
    biomes
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
    biome_context: Option<(&BiomeSource, &RegistrySnapshot<Biome>)>,
    multi_noise: Option<&MultiNoiseBiomeTable>,
    cancel: &CancellationToken,
) -> Vec<Option<(BlockPalette, BiomePalette)>> {
    let mut column = ColumnBlocks::new(y_sections);
    let Some(filled) = fill_column_dense_any(
        &mut column,
        section_x,
        section_z,
        y_sections,
        noise_router,
        biome_context,
        multi_noise,
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
pub fn fill_column_dense_any<'a>(
    column: &mut ColumnBlocks,
    section_x: i32,
    section_z: i32,
    y_sections: &[i32],
    noise_router: &'a NoiseRouter,
    biome_context: Option<(&BiomeSource, &RegistrySnapshot<Biome>)>,
    multi_noise: Option<&MultiNoiseBiomeTable>,
    cancel: &CancellationToken,
) -> Option<FilledColumn<'a>> {
    let block_x = section_x * 16;
    let block_z = section_z * 16;
    let (biomes, biome_grid) = column_biome_palettes(
        noise_router,
        biome_context,
        multi_noise,
        block_x,
        block_z,
        y_sections,
    );
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
        cancel,
    ) {
        return None;
    }
    Some(FilledColumn {
        biomes,
        tops,
        biome_grid,
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
/// Ports replaceBlocksForBiome from back2beta with a single per-chunk LegacyRandom
/// that drives both the surface depth, beach conditions, and the bedrock Y 0-4
/// probabilistic check — all interleaved in back2beta's exact column iteration order.
///
/// The caller seeds `rng` once per chunk with seed = chunkX*341873128712 + chunkZ*132897987541.
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

    // Extract the quantized biome lookup from the biome source.
    // back2beta's replaceBlocksForBiome reads biomes via getBiomeFromLookup (quantized).
    let beta_lookup = match biome_source {
        BiomeSource::Beta { lookup, .. } => Some(lookup.as_ref()),
        _ => None,
    };

    let sea_level = noise_router.sea_level;
    let default_fluid = noise_router.default_fluid_state;
    let stone = noise_router.default_block_state;
    let bedrock = VoxelId::from(blocks.default_state("minecraft:bedrock"));
    let sandstone = VoxelId::from(blocks.default_state("minecraft:sandstone"));
    let gravel = VoxelId::from(blocks.default_state("minecraft:gravel"));
    let ice = VoxelId::from(blocks.default_state("minecraft:ice"));
    let sand = VoxelId::from(blocks.default_state("minecraft:sand"));

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
            let flag = r[idx] as f64 + rng.next_java_double() * 0.2 > 0.0;
            let flag1 = s[idx] as f64 + rng.next_java_double() * 0.2 > 3.0;
            let i1 = (t[idx] as f64 / 3.0 + 3.0 + rng.next_java_double() * 0.25) as i32;

            let (temp, humidity) = (temperatures[idx], humidities[idx]);
            let biome_land: BetaLandBiome = if let Some(table) = beta_lookup {
                beta_biome_from_climate(table, temp, humidity)
            } else {
                beta_get_biome(temp, humidity)
            };
            let (top_block, filler_block) = beta_surface_blocks(biome_land, blocks);
            let (top_block, filler_block) = (VoxelId::from(top_block), VoxelId::from(filler_block));

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

pub mod column_blocks;
pub use column_blocks::ColumnBlocks;
pub mod beta_caves;
pub use beta_caves::{BetaCaveBlockIds, apply_beta_caves};
pub mod beta_ores;
pub mod modern_carvers;
pub mod multi_noise_biomes;
pub mod routers;
pub mod surface;
pub use beta_ores::{BetaOreBlockIds, apply_beta_ores, place_all_ores};
pub use routers::{DimensionBiomeSources, DimensionRouters};
pub use surface::{SurfaceIds, apply_material_surface, spans_dimension};

#[cfg(test)]
mod tests;
