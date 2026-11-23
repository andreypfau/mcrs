#![expect(
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "the panics in this crate are not yet replaced by errors or by code that cannot panic"
)]
#![cfg_attr(
    test,
    expect(
        clippy::disallowed_macros,
        clippy::disallowed_types,
        reason = "test input is written as JSON"
    )
)]

use crate::biome_upscale::upscale_biomes;
use crate::multi_noise_biomes::{BiomeGrid, MultiNoiseBiomeTable};
use crate::probe_lattice::ProbeLattice;
use crate::structures::index::EndBiomes;
use crate::task::CancellationToken;
use glam::IVec3;
use mcrs_minecraft_biome::registry_keys::BIOME;
use mcrs_minecraft_biome::source::{BetaLandBiome, BiomeSource, beta_biome_from_climate};
use mcrs_minecraft_biome::zoom::{FiddleCache, obfuscate_seed};
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_block_predicate::predicate::HeightmapName;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_chunk::palette::{BiomePalette, BlockPalette};
use mcrs_minecraft_heightmap::{HeightmapKinds, HeightmapPredicates};
use mcrs_minecraft_noise::interval::Interval;
use mcrs_minecraft_noise::sample_grid::SampleGrid;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_resource::NarrowError;
use mcrs_minecraft_worldgen::beard::Beard;
use mcrs_minecraft_worldgen_density::aquifer::{FluidField, FluidStatus};
use mcrs_minecraft_worldgen_density::aquifer_cells::AquiferCells;
use mcrs_minecraft_worldgen_density::cell::{CELL_BOUNDS_SLACK, corner_bounds, sampled_bounds};
use mcrs_minecraft_worldgen_density::column_bound::ColumnBoundScratch;
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_density::router::{
    FINAL_DENSITY, NoiseRouter, TEMPERATURE, VEGETATION,
};
use mcrs_minecraft_worldgen_density::router_key::RouterKey;
use mcrs_minecraft_worldgen_density::surface_levels::SurfaceLevels;
use mcrs_voxel::{BlockStateId, ColumnPos};
use std::cell::RefCell;
use std::collections::HashMap;
use std::sync::Arc;

struct CellLattice {
    volume: SampleGrid,
    cell: IVec3,
    cell_rows: i32,
    values: Vec<f32>,
    width: usize,
}

#[derive(Default)]
struct LatticePlanes {
    owner: Option<RouterKey>,
    current: HashMap<i64, Box<[f32]>>,
    previous: HashMap<i64, Box<[f32]>>,
}

const KEPT_COLUMNS: usize = 2048;

impl LatticePlanes {
    fn retarget(&mut self, router: &NoiseRouter) {
        let owner = router.key();
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
    static SURFACE_LEVELS: RefCell<SurfaceLevels> = RefCell::new(SurfaceLevels::default());
}

fn plane_key(node_x: i32, node_z: i32) -> i64 {
    (i64::from(node_x) << 32) | i64::from(node_z as u32)
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum CellFill {
    Solid,
    Uniform(FluidStatus),
    Mixed,
}

impl CellLattice {
    fn fill(
        noise_router: &NoiseRouter,
        block_x: i32,
        block_z: i32,
        beard: Option<&Beard>,
        fill: &mut FillBuffers,
    ) -> Option<Self> {
        let cell = noise_router.cell_size()?;
        if 16 % cell.x != 0 || 16 % cell.y != 0 || 16 % cell.z != 0 {
            return None;
        }
        let height = noise_router.noise.height as i32;
        if noise_router.noise.min_y % 16 != 0 || height % 16 != 0 {
            return None;
        }
        let full = SampleGrid::new(
            IVec3::new(16 / cell.x + 1, height / cell.y + 1, 16 / cell.z + 1),
            IVec3::new(block_x, noise_router.noise.min_y, block_z),
            cell,
        );
        let cell_rows = full.size().y - 1;
        let width = noise_router.cell_inputs().len();
        let hint = LATTICE_PLANES.with_borrow_mut(|planes| {
            planes.retarget(noise_router);
            borrowed_columns(&full)
                .filter_map(|(x, z)| planes.get(plane_key(full.block_x(x), full.block_z(z))))
                .filter_map(|column| column.len().checked_div(width))
                .max()
                .map(|held| held as i32 - 1)
        });
        let first_air =
            noise_router.final_density_first_air_row(&mut fill.ws, &full, hint, &mut fill.columns);
        let bearded = beard.and_then(|beard| {
            (first_air..cell_rows).rev().find(|&cell_y| {
                (0..full.size().z - 1).any(|cell_z| {
                    (0..full.size().x - 1).any(|cell_x| {
                        let world = IVec3::new(
                            full.block_x(cell_x),
                            full.block_y(cell_y),
                            full.block_z(cell_z),
                        );
                        beard.intersects(&SampleGrid::dense(cell, world))
                    })
                })
            })
        });
        let rows = bearded.map_or(first_air, |cell_y| cell_y + 1) + 1;
        let mut lattice = Self {
            volume: SampleGrid::new(
                IVec3::new(full.size().x, rows.min(full.size().y), full.size().z),
                full.min_block(),
                cell,
            ),
            cell,
            cell_rows,
            values: Vec::new(),
            width,
        };
        lattice.compute(noise_router, fill);
        Some(lattice)
    }

    fn holds(&self, cell_y: i32) -> bool {
        cell_y + 1 < self.volume.size().y
    }

    fn extend(&mut self, noise_router: &NoiseRouter, fill: &mut FillBuffers) {
        self.volume = SampleGrid::new(
            IVec3::new(
                self.volume.size().x,
                self.cell_rows + 1,
                self.volume.size().z,
            ),
            self.volume.min_block(),
            self.cell,
        );
        self.compute(noise_router, fill);
    }

    fn compute(&mut self, noise_router: &NoiseRouter, fill: &mut FillBuffers) {
        let volume = self.volume;
        let FillBuffers {
            ws,
            columns,
            block_values,
            ..
        } = fill;
        let width = self.width;
        let rows = volume.size().y as usize;
        let (nx, nz) = (volume.size().x, volume.size().z);
        let values = &mut self.values;
        values.clear();
        values.resize(width * volume.len(), 0.0);

        let borrowed = borrowed_columns(&volume).collect::<Vec<_>>();
        let shared = LATTICE_PLANES.with_borrow_mut(|planes| {
            planes.retarget(noise_router);
            let held = borrowed.iter().try_fold(rows, |low, &(x, z)| {
                let column = planes.get(plane_key(volume.block_x(x), volume.block_z(z)))?;
                Some(low.min(column.len().checked_div(width).unwrap_or(rows)))
            })?;
            for &(x, z) in &borrowed {
                let column = planes.get(plane_key(volume.block_x(x), volume.block_z(z)))?;
                let stride = column.len().checked_div(width).unwrap_or(rows);
                for k in 0..width {
                    let at = k * volume.len() + volume.index_unchecked(x, 0, z);
                    let source = column.get(k * stride..k * stride + held)?;
                    values.get_mut(at..at + held)?.copy_from_slice(source);
                }
            }
            Some(held)
        });
        let filled = shared.and_then(|held| {
            let size_y = volume.size().y;
            let lattice = LatticeBlocks {
                noise_router,
                volume,
                width,
                columns,
            };
            lattice.fill(
                ws,
                block_values,
                IVec3::new(1, 0, 1),
                IVec3::new(nx - 1, size_y, nz - 1),
                values,
            )?;
            let held = held as i32;
            if held < size_y {
                let missing = size_y - held;
                lattice.fill(
                    ws,
                    block_values,
                    IVec3::new(0, held, 0),
                    IVec3::new(1, missing, nz),
                    values,
                )?;
                lattice.fill(
                    ws,
                    block_values,
                    IVec3::new(1, held, 0),
                    IVec3::new(nx - 1, missing, 1),
                    values,
                )?;
            }
            Some(())
        });
        if filled.is_none() {
            noise_router.fill_cell_inputs(ws, &volume, columns, values);
        }

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

        noise_router.pin_cell_lattice(ws, &volume, values);
    }

    fn classify(
        &self,
        noise_router: &NoiseRouter,
        at: IVec3,
        fluid: &mut FluidField<'_>,
        fill: &mut FillBuffers,
    ) -> CellFill {
        if !self.holds(at.y) {
            return self.settle(at, fluid);
        }
        let min = IVec3::new(
            self.volume.block_x(at.x),
            self.volume.block_y(at.y),
            self.volume.block_z(at.z),
        );
        let air = noise_router
            .final_density_chain_bound(&self.values, &self.volume, at, min, min + self.cell - 1)
            .filter(|chain| chain.max() < -CELL_BOUNDS_SLACK);
        if let Some(chain) = air {
            return self.verdict(chain, at, fluid);
        }
        corner_bounds(&self.values, &self.volume, at, &mut fill.corners);
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

    fn verdict(&self, bounds: Interval, at: IVec3, fluid: &mut FluidField<'_>) -> CellFill {
        if bounds.min() > CELL_BOUNDS_SLACK {
            return CellFill::Solid;
        }
        if bounds.max() < -CELL_BOUNDS_SLACK {
            return self.settle(at, fluid);
        }
        CellFill::Mixed
    }

    fn settle(&self, at: IVec3, fluid: &mut FluidField<'_>) -> CellFill {
        let min = IVec3::new(
            self.volume.block_x(at.x),
            self.volume.block_y(at.y),
            self.volume.block_z(at.z),
        );
        match fluid.settle(min, min + self.cell - 1) {
            Some(status) => CellFill::Uniform(status),
            None => CellFill::Mixed,
        }
    }
}

fn borrowed_columns(volume: &SampleGrid) -> impl Iterator<Item = (i32, i32)> {
    let (nx, nz) = (volume.size().x, volume.size().z);
    (0..nz).map(|z| (0, z)).chain((1..nx).map(|x| (x, 0)))
}

struct LatticeBlocks<'a> {
    noise_router: &'a NoiseRouter,
    volume: SampleGrid,
    width: usize,
    columns: &'a ColumnBoundScratch,
}

impl LatticeBlocks<'_> {
    fn fill(
        &self,
        ws: &mut Workspace,
        block_values: &mut Vec<f32>,
        origin: IVec3,
        size: IVec3,
        values: &mut [f32],
    ) -> Option<()> {
        let volume = &self.volume;
        let block = SampleGrid::new(
            size,
            IVec3::new(
                volume.block_x(origin.x),
                volume.block_y(origin.y),
                volume.block_z(origin.z),
            ),
            volume.step_block(),
        );
        block_values.clear();
        block_values.resize(self.width * block.len(), 0.0);
        self.noise_router
            .fill_cell_inputs(ws, &block, self.columns, block_values);
        let rows = size.y as usize;
        for z in 0..size.z {
            for x in 0..size.x {
                for k in 0..self.width {
                    let from = k * block.len() + block.index_unchecked(x, 0, z);
                    let to = k * volume.len()
                        + volume.index_unchecked(origin.x + x, origin.y, origin.z + z);
                    let source = block_values.get(from..from + rows)?;
                    values.get_mut(to..to + rows)?.copy_from_slice(source);
                }
            }
        }
        Some(())
    }
}

pub const NO_TOP: i32 = i32::MIN;

pub struct FilledColumn<'a> {
    pub biomes: Vec<BiomePalette>,
    pub tops: [i32; 256],
    pub material_surface: bool,
    pub fluid: FluidField<'a>,
}

#[derive(Default)]
struct FillBuffers {
    ws: Workspace,
    density: Vec<f32>,
    barrier: Vec<f32>,
    corners: Vec<Interval>,
    cell_terms: Vec<Interval>,
    columns: ColumnBoundScratch,
    block_values: Vec<f32>,
}

#[expect(
    clippy::too_many_arguments,
    reason = "independent inputs of one column fill"
)]
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

    let Some(mut lattice) = CellLattice::fill(noise_router, block_x, block_z, beard, &mut fill)
    else {
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
            for cell_y in (0..lattice.cell_rows).rev() {
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
                let verdict = if beard.is_some_and(|beard| beard.intersects(&grid)) {
                    CellFill::Mixed
                } else {
                    lattice.classify(noise_router, at, fluid, &mut fill)
                };
                if verdict == CellFill::Mixed && !lattice.holds(cell_y) {
                    lattice.extend(noise_router, &mut fill);
                }
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

#[expect(
    clippy::too_many_arguments,
    reason = "independent inputs of one dense column fill"
)]
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

#[expect(
    clippy::too_many_arguments,
    reason = "independent inputs of one block fill"
)]
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
        .program()
        .fill(ws, volume, FINAL_DENSITY, density);
    if let Some(beard) = beard {
        beard.fill(volume, density);
    }
    let mut barrier_at = volume_barrier(noise_router, ws, volume, barrier);
    let min = volume.min_block();
    let settled = fluid.settle(min, min + volume.size() - 1);
    for z in 0..volume.size().z {
        for x in 0..volume.size().x {
            for y in (0..volume.size().y).rev() {
                let value = density[volume.index_unchecked(x, y, z)];
                let (px, py, pz) = (origin.x + x, origin.y + y, origin.z + z);
                let world = IVec3::new(volume.block_x(x), volume.block_y(y), volume.block_z(z));
                let state = match settled {
                    Some(status) if value <= 0.0 => status.at(world.y),
                    _ => column_block(
                        default_block,
                        value,
                        fluid,
                        world.x,
                        world.y,
                        world.z,
                        &mut barrier_at,
                    ),
                };
                if state != VoxelId(0) {
                    column.set_in_section(index, px, py, pz, state);
                    let slot = &mut tops[(pz * 16 + px) as usize];
                    *slot = (*slot).max(world.y);
                }
            }
        }
    }
}

fn volume_barrier<'a>(
    noise_router: &'a NoiseRouter,
    ws: &'a mut Workspace,
    volume: &'a SampleGrid,
    barrier: &'a mut Vec<f32>,
) -> impl FnMut(i32, i32, i32) -> f64 + 'a {
    barrier.clear();
    let barrier_root = noise_router.aquifer().map(|aquifer| aquifer.barrier);
    move |bx: i32, by: i32, bz: i32| {
        if barrier.is_empty() {
            barrier.resize(volume.len(), 0.0);
            let root = barrier_root.expect("only a field with a barrier asks for it");
            noise_router.program().fill(ws, volume, root, barrier);
        }
        let at = volume
            .index_of_block(bx, by, bz)
            .expect("the barrier is read inside the box being filled");
        f64::from(barrier[at])
    }
}

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

pub fn first_free_kind(name: HeightmapName) -> HeightmapKinds {
    match name {
        HeightmapName::WorldSurfaceWg | HeightmapName::WorldSurface => HeightmapKinds::SURFACE,
        _ => HeightmapKinds::SOLID,
    }
}

#[expect(
    clippy::too_many_arguments,
    reason = "independent inputs of one height query"
)]
pub fn base_height(
    router: &NoiseRouter,
    ws: &mut Workspace,
    lattice: &mut ProbeLattice,
    predicates: &HeightmapPredicates,
    kind: HeightmapKinds,
    x: i32,
    z: i32,
    accessor_min_y: i32,
    accessor_height: i32,
) -> i32 {
    let (min_y, column) = base_column(router, ws, lattice, x, z, accessor_min_y, accessor_height);
    column
        .iter()
        .rposition(|state| predicates.get(*state).contains(kind))
        .map_or(min_y, |index| min_y + index as i32 + 1)
}

pub fn base_column(
    router: &NoiseRouter,
    ws: &mut Workspace,
    lattice: &mut ProbeLattice,
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
    lattice.pin(router, ws, x, z);
    router
        .program()
        .fill(ws, &volume, FINAL_DENSITY, &mut density);
    let (min, max) = (
        IVec3::new(x, min_y, z),
        IVec3::new(x, min_y + height - 1, z),
    );
    let mut cells = AquiferCells::default();
    let mut fluid = SURFACE_LEVELS
        .with_borrow_mut(|levels| FluidField::new(router, levels, &mut cells, min, max));
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
    SURFACE_LEVELS.with_borrow_mut(|levels| fluid.keep_surface_levels(levels));
    (min_y, column)
}

#[derive(Clone, Default)]
pub enum ColumnBiomes {
    #[default]
    None,
    MultiNoise(Arc<MultiNoiseBiomeTable>),
    Fixed(u8),
    Beta(BetaBiomes),
    TheEnd(EndBiomes<u8>),
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
    pub fn new(
        source: Option<&BiomeSource>,
        multi_noise: Option<Arc<MultiNoiseBiomeTable>>,
        end: Option<EndBiomes>,
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
            Some(BiomeSource::TheEnd) => match end {
                Some(end) => Self::TheEnd(end.try_map(narrow_biome_number)?),
                None => Self::None,
            },
            _ => Self::None,
        })
    }
}

fn narrow_biome_number(number: u16) -> Result<u8, NarrowError> {
    u8::try_from(number).map_err(|_| NarrowError {
        registry: BIOME.location().to_string(),
        id: number,
        bits: u8::BITS,
    })
}

fn column_biome_palettes(
    noise_router: &NoiseRouter,
    biomes: &ColumnBiomes,
    climate: Option<&ClimateBox>,
    block_x: i32,
    block_z: i32,
    y_sections: &[i32],
) -> (Vec<BiomePalette>, bool) {
    match biomes {
        ColumnBiomes::MultiNoise(table) => {
            let palettes =
                multi_noise_palettes(noise_router, table, climate, block_x, block_z, y_sections);
            (palettes, true)
        }
        ColumnBiomes::Fixed(biome) => (
            vec![BiomePalette::homogeneous(*biome); y_sections.len()],
            true,
        ),
        ColumnBiomes::Beta(beta) => {
            let grid = beta_biome_grid(noise_router, beta, block_x, block_z);
            (
                upscale_column(grid.window(), noise_router, y_sections),
                false,
            )
        }
        ColumnBiomes::TheEnd(end) => {
            let palettes = end_biome_grid(noise_router, end, block_x, block_z, y_sections)
                .map_or_else(Vec::new, |grid| {
                    upscale_column(grid.window(), noise_router, y_sections)
                });
            (palettes, true)
        }
        ColumnBiomes::None => (vec![BiomePalette::default(); y_sections.len()], false),
    }
}

fn upscale_column(
    grid: BiomeWindow<'_>,
    noise_router: &NoiseRouter,
    y_sections: &[i32],
) -> Vec<BiomePalette> {
    thread_local! {
        static FIDDLE: RefCell<FiddleCache> = RefCell::new(FiddleCache::default());
    }
    let zoom_seed = obfuscate_seed(noise_router.world_seed as i64);
    FIDDLE.with_borrow_mut(|fiddle| upscale_biomes(grid, zoom_seed, y_sections, fiddle))
}

pub fn multi_noise_palettes(
    noise_router: &NoiseRouter,
    table: &MultiNoiseBiomeTable,
    climate: Option<&ClimateBox>,
    block_x: i32,
    block_z: i32,
    y_sections: &[i32],
) -> Vec<BiomePalette> {
    let upscale = |window: BiomeWindow<'_>| upscale_column(window, noise_router, y_sections);
    with_column_window(
        noise_router,
        table,
        climate,
        block_x,
        block_z,
        y_sections,
        upscale,
    )
    .unwrap_or_default()
}

pub fn multi_noise_grid(
    noise_router: &NoiseRouter,
    table: &MultiNoiseBiomeTable,
    climate: Option<&ClimateBox>,
    block_x: i32,
    block_z: i32,
    y_sections: &[i32],
) -> Option<BiomeGrid> {
    let copy = |window: BiomeWindow<'_>| window.to_grid();
    with_column_window(
        noise_router,
        table,
        climate,
        block_x,
        block_z,
        y_sections,
        copy,
    )
}

fn with_column_window<R>(
    noise_router: &NoiseRouter,
    table: &MultiNoiseBiomeTable,
    climate: Option<&ClimateBox>,
    block_x: i32,
    block_z: i32,
    y_sections: &[i32],
    lend: impl FnOnce(BiomeWindow<'_>) -> R,
) -> Option<R> {
    if (block_x | block_z) & 15 != 0 {
        return None;
    }
    let col = ColumnPos::new(block_x >> 4, block_z >> 4);
    let (&first, &last) = (y_sections.first()?, y_sections.last()?);
    let spans_sections = |window: &BiomeWindow<'_>| {
        let volume = window.volume();
        volume.min_block().y == first * 16 && volume.size().y == (last - first + 1) * 4
    };
    let shared = climate.and_then(|climate| climate.window(col));
    if let Some(window) = shared.filter(spans_sections) {
        return Some(lend(window));
    }
    let alone = ClimateBox::sample(noise_router, table, [col], y_sections)?;
    Some(lend(alone.window(col)?))
}

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

pub fn beta_chunk_seed(world_seed: i64, chunk_x: i32, chunk_z: i32) -> i64 {
    let mut rng = LegacyRandom::new(world_seed as u64);
    let x_multiplier = rng.next_java_long() / 2 * 2 + 1;
    let z_multiplier = rng.next_java_long() / 2 * 2 + 1;
    (chunk_x as i64)
        .wrapping_mul(x_multiplier)
        .wrapping_add((chunk_z as i64).wrapping_mul(z_multiplier))
        ^ world_seed
}

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
    let mut cells = AquiferCells::default();
    let Some(filled) = fill_column_dense_any(
        &mut column,
        section_x,
        section_z,
        y_sections,
        noise_router,
        &mut cells,
        biomes,
        None,
        None,
        cancel,
    ) else {
        return vec![None; y_sections.len()];
    };

    column.into_sections(&filled.biomes)
}

#[expect(
    clippy::too_many_arguments,
    reason = "independent inputs of one dense fill"
)]
pub fn fill_column_dense_any<'a>(
    column: &mut ColumnBlocks,
    section_x: i32,
    section_z: i32,
    y_sections: &[i32],
    noise_router: &'a NoiseRouter,
    cells: &'a mut AquiferCells,
    biomes: &ColumnBiomes,
    climate: Option<&ClimateBox>,
    beard: Option<&Beard>,
    cancel: &CancellationToken,
) -> Option<FilledColumn<'a>> {
    let block_x = section_x * 16;
    let block_z = section_z * 16;
    let (biomes, material_surface) =
        column_biome_palettes(noise_router, biomes, climate, block_x, block_z, y_sections);
    column.reset(y_sections);

    let mut tops = [NO_TOP; 256];
    let mut fluid = column_fluid_field(noise_router, cells, block_x, block_z);
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
    SURFACE_LEVELS.with_borrow_mut(|levels| fluid.keep_surface_levels(levels));
    Some(FilledColumn {
        biomes,
        tops,
        material_surface,
        fluid,
    })
}

pub fn column_fluid_field<'a>(
    noise_router: &'a NoiseRouter,
    cells: &'a mut AquiferCells,
    block_x: i32,
    block_z: i32,
) -> FluidField<'a> {
    let (min, max) = fill_bounds(noise_router, block_x, block_z, block_x, block_z);
    SURFACE_LEVELS.with_borrow_mut(|levels| FluidField::new(noise_router, levels, cells, min, max))
}

pub fn cover_fill_group(
    noise_router: &NoiseRouter,
    columns: impl IntoIterator<Item = ColumnPos>,
    cells: &mut AquiferCells,
) {
    let mut columns = columns.into_iter();
    let Some(first) = columns.next() else {
        return;
    };
    let (low, high) = columns.fold((first, first), |(low, high), col| {
        (
            ColumnPos::new(low.x.min(col.x), low.z.min(col.z)),
            ColumnPos::new(high.x.max(col.x), high.z.max(col.z)),
        )
    });
    let (min, max) = fill_bounds(
        noise_router,
        low.x * 16,
        low.z * 16,
        high.x * 16,
        high.z * 16,
    );
    cells.cover(noise_router, min, max);
}

fn fill_bounds(
    noise_router: &NoiseRouter,
    low_x: i32,
    low_z: i32,
    high_x: i32,
    high_z: i32,
) -> (IVec3, IVec3) {
    let min_y = noise_router.noise.min_y;
    (
        IVec3::new(low_x, min_y, low_z),
        IVec3::new(
            high_x + 15,
            min_y + noise_router.noise.height as i32 - 1,
            high_z + 15,
        ),
    )
}

pub fn beta_surface_rng(chunk_x: i32, chunk_z: i32) -> LegacyRandom {
    LegacyRandom::large_feature_with_salt(0, chunk_x, chunk_z, 0)
}

#[expect(
    clippy::too_many_lines,
    reason = "one pass keeps the draws on one random stream in order"
)]
#[expect(
    clippy::excessive_nesting,
    reason = "the depth cases share one random stream in one pass"
)]
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
    let bedrock = VoxelId::from(blocks.default_state_of(Block::Bedrock.id()).number());
    let sandstone = VoxelId::from(blocks.default_state_of(Block::Sandstone.id()).number());
    let gravel = VoxelId::from(blocks.default_state_of(Block::Gravel.id()).number());
    let ice = VoxelId::from(blocks.default_state_of(Block::Ice.id()).number());
    let sand = VoxelId::from(blocks.default_state_of(Block::Sand.id()).number());

    const D0: f64 = 0.03125;

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

    for z_local in 0..16i32 {
        for x_local in 0..16i32 {
            let idx = (x_local * 16 + z_local) as usize;

            let flag = r[idx] as f64 + rng.next_f64() * 0.2 > 0.0;
            let flag1 = s[idx] as f64 + rng.next_f64() * 0.2 > 3.0;
            let i1 = (t[idx] as f64 / 3.0 + 3.0 + rng.next_f64() * 0.25) as i32;

            let (temp, humidity) = (temperatures[idx], humidities[idx]);
            let biome_land: BetaLandBiome = beta_biome_from_climate(beta_lookup, temp, humidity);
            let (top_block, filler_block) = beta_surface_blocks(biome_land, blocks);
            let (top_block, filler_block) = (
                VoxelId::from(top_block.number()),
                VoxelId::from(filler_block.number()),
            );

            let mut j1: i32 = -1;
            let mut b1 = top_block;
            let mut b2 = filler_block;
            let air = VoxelId(0);

            for k1 in (0i32..=127).rev() {
                if k1 <= rng.next_i32_bound(5) {
                    column.set(x_local, k1, z_local, bedrock);
                } else {
                    let current_id = column.get(x_local, k1, z_local);

                    let current_id = match current_id {
                        Some(id) => id,
                        None => continue,
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
pub mod dimension_plan;
mod end_biome_grid;
mod flat_world;
pub mod plan_error;
pub mod world_tables;
pub mod world_tables_error;
use end_biome_grid::end_biome_grid;
pub mod column_blocks;
pub mod task;
pub mod template_names;
pub use column_blocks::ColumnBlocks;
mod generate_area;
pub use generate_area::generate_area;
pub mod beta_caves;
pub use beta_caves::{BetaCaveBlockIds, apply_beta_carvers};
pub mod beta_ores;
pub(crate) mod biome_upscale;
mod biome_window;
pub use biome_window::BiomeWindow;
mod climate_box;
pub use climate_box::ClimateBox;
pub mod feature_program;
pub mod features;
pub mod flat;
pub mod ids;
pub mod modern_carvers;
pub mod multi_noise_biomes;
pub mod probe_lattice;
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
