use std::cell::RefCell;
use std::sync::Arc;

use bevy_ecs::prelude::Resource;
use bevy_math::IVec3;
use fixedbitset::FixedBitSet;
use mcrs_minecraft_core::RegistrySnapshot;
use mcrs_minecraft_core::tag::registry::DynTagRegistry;
use mcrs_minecraft_decoration::block_entity::GeneratedBlockEntity;
use mcrs_minecraft_protocol::ColumnPos;
use mcrs_minecraft_random::Random;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_random::xoroshiro::XoroshiroRandom;
use mcrs_minecraft_world::biome::Biome;
use mcrs_minecraft_world::biome::source::BiomeSource;
use mcrs_minecraft_world::biome::zoom::{obfuscate_seed, quart_cell};
use mcrs_minecraft_world::block::Block as VanillaBlock;
use mcrs_minecraft_world::block::definition::BlockDefinitions;
use mcrs_minecraft_worldgen::feature::placement::HeightmapName;
use mcrs_minecraft_worldgen::feature::placer::{
    PlacerScratch, WorldGenVolume, WorldStates, decorate,
};
use mcrs_minecraft_worldgen::material::MaterialScratch;
use mcrs_minecraft_worldgen::program::Workspace;
use mcrs_minecraft_worldgen::router::NoiseRouter;
use mcrs_minecraft_worldgen::value_provider::HeightContext;
use mcrs_voxel_math::chunk_pos::BLOCKS;
use mcrs_voxel_storage::{Blocks, BlocksMut, Volume, VoxelId};
use rustc_hash::FxHashMap;
use tracing::{error, info_span};

use crate::world::chunk::{CancellationToken, ColumnSource};
use crate::world::format::anvil::{SavedColumns, column_sections, saved_block_entities};
use crate::world::generate::feature_program::{FeatureProgram, RunScratch};
use crate::world::generate::modern_carvers::{
    CarverBiomeTable, ModernCarverBlockIds, apply_modern_carvers,
};
use crate::world::generate::multi_noise_biomes::MultiNoiseBiomeTable;
use crate::world::generate::staging::{
    ColumnDelta, FilledSnapshot, RegionSnapshots, cell_index, rank, region_column, region_slot,
};
use crate::world::generate::structures::DimensionStructureTables;
use crate::world::generate::structures::index::{BiomeLookup, StructureIndex};
use crate::world::generate::{
    BetaCaveBlockIds, ColumnBlocks, SurfaceIds, apply_beta_carvers, apply_beta_surface,
    apply_material_surface, fill_column_dense_any, spans_dimension,
};
use crate::world::heightmap::{
    ColumnHeightmapSet, HeightmapPredicates, PreCarveHeightmaps, build_column_heightmaps,
    build_pre_carve_heightmaps,
};

/// Everything a column stage reads that is the same for every column of one
/// dimension, built once when the dimension spawns. Every field is a handle,
/// so a clone per dispatch is a refcount.
#[derive(Clone, Resource)]
pub struct FillContext {
    pub router: Arc<NoiseRouter>,
    pub blocks: Arc<BlockDefinitions>,
    /// The section list every column of the dimension is generated over: a
    /// delta names a cell by its index in it, so it is one list, not one per
    /// dispatch.
    pub y_sections: Arc<[i32]>,
    pub biome: Option<(Arc<BiomeSource>, Arc<RegistrySnapshot<Biome>>)>,
    pub predicates: Option<HeightmapPredicates>,
    /// Read only where `biome` names the registry the save is decoded against.
    pub saved: Option<SavedColumns>,
    pub program: ColumnProgram,
    pub structures: Option<Arc<StructureIndex>>,
}

/// What the fill and the run do to a column past the density fill.
#[derive(Clone)]
pub struct ColumnProgram {
    pub generator: ColumnGenerator,
    pub carvers: Option<Arc<CarverBiomeTable>>,
    pub features: Option<Arc<FeatureProgram>>,
}

/// Where Beta and the modern generator part, chosen by the dimension's biome
/// source: the surface, and what a carved block becomes.
#[derive(Clone)]
pub enum ColumnGenerator {
    Beta(Arc<BetaCaveBlockIds>),
    Modern {
        multi_noise: Option<Arc<MultiNoiseBiomeTable>>,
        surface: Option<Arc<SurfaceIds>>,
        carver_blocks: Arc<ModernCarverBlockIds>,
    },
}

impl ColumnProgram {
    #[cfg(test)]
    pub fn modern(features: Option<Arc<FeatureProgram>>) -> Self {
        ColumnProgram {
            generator: ColumnGenerator::Modern {
                multi_noise: None,
                surface: None,
                carver_blocks: Arc::new(ModernCarverBlockIds::for_test(Vec::new())),
            },
            carvers: None,
            features,
        }
    }
}

impl FillContext {
    /// One context for a dimension, with every table its stages read resolved
    /// here and never again per column.
    #[allow(clippy::too_many_arguments)]
    pub fn build(
        router: Arc<NoiseRouter>,
        blocks: Arc<BlockDefinitions>,
        y_sections: Arc<[i32]>,
        biome: Option<(Arc<BiomeSource>, Arc<RegistrySnapshot<Biome>>)>,
        predicates: Option<HeightmapPredicates>,
        saved: Option<SavedColumns>,
        carver_biomes: Option<Arc<CarverBiomeTable>>,
        block_tags: Option<&DynTagRegistry<VanillaBlock>>,
        features: Option<Arc<FeatureProgram>>,
        structures: Option<Arc<DimensionStructureTables>>,
    ) -> Self {
        let multi_noise = biome.as_ref().and_then(|(source, registry)| {
            let BiomeSource::MultiNoise(multi) = source.as_ref() else {
                return None;
            };
            MultiNoiseBiomeTable::resolve(multi, |location| {
                // The palette stores a biome in a byte, so an id past
                // 255 would silently alias another biome.
                match registry.by_location(location).map(u8::try_from) {
                    Some(Ok(id)) => Some(id),
                    Some(Err(_)) => {
                        error!(
                            biome = location,
                            "biome id is past the 256 the palette can store"
                        );
                        None
                    }
                    None => {
                        error!(biome = location, "biome missing from the registry");
                        None
                    }
                }
            })
            .map(Arc::new)
        });
        let structures = structures.map(|tables| {
            let biome_lookup = match (&multi_noise, &biome) {
                (Some(table), _) => BiomeLookup::MultiNoise(Arc::clone(table)),
                (None, Some((source, registry))) => match source.as_ref() {
                    BiomeSource::Fixed { biome_id, .. } => registry
                        .by_location(biome_id.as_str())
                        .map_or(BiomeLookup::None, BiomeLookup::Fixed),
                    _ => BiomeLookup::None,
                },
                (None, None) => BiomeLookup::None,
            };
            let (min_y, height) = match y_sections.first() {
                Some(&first) => (
                    first << BLOCKS::BITS,
                    (y_sections.len() as i32) << BLOCKS::BITS,
                ),
                None => (router.noise.min_y, router.noise.height as i32),
            };
            Arc::new(StructureIndex::new(
                tables,
                router.world_seed as i64,
                Arc::clone(&router),
                biome_lookup,
                predicates.clone(),
                min_y,
                height,
            ))
        });
        let generator = match &biome {
            Some((source, _)) if matches!(source.as_ref(), BiomeSource::Beta { .. }) => {
                ColumnGenerator::Beta(Arc::new(BetaCaveBlockIds::resolve(&blocks)))
            }
            _ => ColumnGenerator::Modern {
                multi_noise,
                surface: biome
                    .as_ref()
                    .map(|(_, registry)| Arc::new(SurfaceIds::resolve(&blocks, registry))),
                carver_blocks: Arc::new(ModernCarverBlockIds::resolve(&blocks, block_tags)),
            },
        };
        let program = ColumnProgram {
            generator,
            carvers: carver_biomes.filter(|_| biome.is_some()),
            features,
        };
        FillContext {
            saved,
            router,
            blocks,
            y_sections,
            biome,
            predicates,
            program,
            structures,
        }
    }

    fn biome_context(&self) -> Option<(&BiomeSource, &RegistrySnapshot<Biome>)> {
        self.biome
            .as_ref()
            .map(|(src, reg)| (src.as_ref(), reg.as_ref()))
    }

    pub fn features(&self) -> Option<&FeatureProgram> {
        self.program.features.as_deref()
    }

    /// How many rungs of run-and-merge the dimension's columns climb. A
    /// dimension that decorates nothing climbs none and goes from filled
    /// straight to delivered.
    pub fn rungs(&self) -> usize {
        self.features().map_or(0, |program| program.rungs().len())
    }
}

/// One dense buffer per worker, reused column after column: a fresh one costs an
/// allocation the size of the whole column.
fn with_column_buffer<T>(f: impl FnOnce(&mut ColumnBlocks) -> T) -> T {
    thread_local! {
        static COLUMN: RefCell<ColumnBlocks> = RefCell::new(ColumnBlocks::new(&[]));
    }
    COLUMN.with_borrow_mut(f)
}

pub fn extent(router: &NoiseRouter) -> HeightContext {
    HeightContext {
        min_y: router.noise.min_y,
        depth: router.noise.height as i32,
        sea_level: router.sea_level,
    }
}

fn pre_carve(ctx: &FillContext, column: &ColumnBlocks) -> Option<PreCarveHeightmaps> {
    ctx.predicates.as_ref().and_then(|p| {
        let _span = info_span!("world::column_pre_carve").entered();
        build_pre_carve_heightmaps(column, p)
    })
}

/// `Filled`: the terrain of one column, its packed palettes and its maps.
///
/// A column the save already holds returns here whole, with the block entities
/// it was saved with, and never runs its own program again; it is still a
/// legitimate input to its neighbours' runs. `None` means the column was
/// cancelled part-way.
pub fn fill_column(
    ctx: &FillContext,
    col: ColumnPos,
    column: &mut ColumnBlocks,
    cancel: &CancellationToken,
) -> Option<FilledSnapshot> {
    let _span = info_span!("world::column_load").entered();
    let router = ctx.router.as_ref();
    let y_sections = &ctx.y_sections;

    let loaded = ctx
        .saved
        .as_ref()
        .zip(ctx.biome.as_ref())
        .and_then(|(saved, (_, biomes))| {
            let chunk = {
                let _read = info_span!("world::column_read_saved").entered();
                saved.read(col.x, col.z)?
            };
            let _decode = info_span!("world::column_decode_saved").entered();
            let decoded = column_sections(&chunk, y_sections, &ctx.blocks, biomes)
                .map_err(|err| err.to_string())
                .and_then(|sections| {
                    saved_block_entities(&chunk)
                        .map(|entities| (sections, entities))
                        .map_err(|err| err.to_string())
                });
            match decoded {
                Ok(loaded) => Some(loaded),
                Err(err) => {
                    error!(%err, x = col.x, z = col.z, "decoding a saved column");
                    None
                }
            }
        });

    if let Some((sections, block_entities)) = loaded {
        let maps = ctx
            .predicates
            .as_ref()
            .and_then(|p| build_column_heightmaps(&sections, y_sections, p));
        // A saved column was carved before it was written, so the maps it
        // arrives with are the only answer there is to the pre-carve pair.
        let pre_carve = maps.as_ref().map(|maps| PreCarveHeightmaps {
            surface: maps.surface.0.clone(),
            solid: maps.solid.0.clone(),
        });
        return Some(FilledSnapshot {
            col,
            y_sections: y_sections.clone(),
            sections,
            pre_carve,
            maps,
            source: ColumnSource::Saved,
            block_entities,
        });
    }

    let mut filled = {
        let _gen = info_span!("world::column_gen").entered();
        let multi_noise = match &ctx.program.generator {
            ColumnGenerator::Modern { multi_noise, .. } => multi_noise.as_deref(),
            ColumnGenerator::Beta(_) => None,
        };
        fill_column_dense_any(
            column,
            col.x,
            col.z,
            y_sections,
            router,
            ctx.biome_context(),
            multi_noise,
            cancel,
        )?
    };

    // The surface runs between the fill it reads and the carvers, because
    // carved rock is not re-surfaced.
    match &ctx.program.generator {
        ColumnGenerator::Beta(_) => {
            let (src, _) = ctx
                .biome_context()
                .expect("a beta program has a biome source");
            let seed = (col.x as i64)
                .wrapping_mul(341873128712)
                .wrapping_add((col.z as i64).wrapping_mul(132897987541));
            let mut rng = LegacyRandom::new(seed as u64);
            apply_beta_surface(
                column,
                col.x * 16,
                col.z * 16,
                router,
                src,
                &ctx.blocks,
                &mut rng,
            );
        }
        ColumnGenerator::Modern {
            surface: Some(ids), ..
        } => {
            if let Some(grid) = filled.biome_grid.as_ref() {
                thread_local! {
                    static MATERIAL: RefCell<MaterialScratch> = RefCell::new(MaterialScratch::default());
                }
                debug_assert!(
                    spans_dimension(y_sections, router),
                    "the descent needs the whole column, not the sections this dispatch owes"
                );
                MATERIAL.with_borrow_mut(|scratch| {
                    apply_material_surface(
                        column,
                        col.x,
                        col.z,
                        &mut filled.tops,
                        grid,
                        router,
                        ids,
                        scratch,
                    );
                });
            }
        }
        ColumnGenerator::Modern { surface: None, .. } => {}
    }

    let pre_carve = pre_carve(ctx, column);

    if let Some(carvers) = &ctx.program.carvers {
        let world_seed = router.world_seed as i64;
        let mut ws = Workspace::new();
        // The carvers draw against the dimension, not against the slice of
        // sections a dispatch happens to carry.
        let height = extent(router);
        match &ctx.program.generator {
            ColumnGenerator::Beta(ids) => apply_beta_carvers(
                column, col.x, col.z, world_seed, router, &mut ws, carvers, height, ids,
            ),
            ColumnGenerator::Modern { carver_blocks, .. } => apply_modern_carvers(
                column,
                col.x,
                col.z,
                world_seed,
                router,
                &mut ws,
                carvers,
                height,
                carver_blocks,
                &mut filled.fluid,
            ),
        }
    }

    Some(pack(ctx, col, column, &filled.biomes, pre_carve))
}

fn pack(
    ctx: &FillContext,
    col: ColumnPos,
    column: &ColumnBlocks,
    biomes: &[mcrs_minecraft_block::palette::BiomePalette],
    pre_carve: Option<PreCarveHeightmaps>,
) -> FilledSnapshot {
    let sections = column.into_sections(biomes);
    let maps = ctx
        .predicates
        .as_ref()
        .and_then(|p| build_column_heightmaps(&sections, &ctx.y_sections, p));
    FilledSnapshot {
        col,
        y_sections: ctx.y_sections.clone(),
        sections,
        pre_carve,
        maps,
        source: ColumnSource::Generated,
        block_entities: Vec::new(),
    }
}

/// The 3×3 one unit's program reads and writes.
///
/// Reads answer, in order: the unit's own earlier write, then the filled block
/// of the snapshot that holds the position, then — outside the 3×3 — air. Writes
/// go to the dense centre buffer or to the target column's delta; a write past
/// the 3×3 is a data error and is dropped.
pub struct ColumnRegion<'a> {
    center: ColumnPos,
    snapshots: &'a RegionSnapshots,
    blocks: &'a ColumnBlocks,
    ctx: &'a FillContext,
    maps: Option<ColumnHeightmapSet>,
    zoom_seed: i64,
    own: Vec<(u32, VoxelId)>,
    ring: [FxHashMap<u32, VoxelId>; 9],
    /// What the generators asked the world to remember about a block they
    /// wrote; they travel with the writes, in the delta of the column that
    /// holds them.
    block_entities: Vec<GeneratedBlockEntity>,
}

impl<'a> ColumnRegion<'a> {
    /// `blocks` must already hold the centre snapshot unpacked.
    pub fn new(
        snapshots: &'a RegionSnapshots,
        blocks: &'a ColumnBlocks,
        ctx: &'a FillContext,
    ) -> Self {
        Self {
            center: snapshots[4].col,
            snapshots,
            blocks,
            ctx,
            maps: snapshots[4].maps.clone(),
            zoom_seed: obfuscate_seed(ctx.router.world_seed as i64),
            own: Vec::new(),
            ring: Default::default(),
            block_entities: Vec::new(),
        }
    }

    pub fn center(&self) -> ColumnPos {
        self.center
    }

    fn locate(&self, p: IVec3) -> Option<(usize, usize, usize)> {
        let slot = region_slot(
            self.center,
            ColumnPos::new(p.x.div_euclid(16), p.z.div_euclid(16)),
        )?;
        Some((
            slot,
            p.x.rem_euclid(16) as usize,
            p.z.rem_euclid(16) as usize,
        ))
    }

    /// The live centre map, or the ring column's frozen one; the two pre-carve
    /// generations are frozen for every column of the region.
    pub fn map_height(&self, kind: HeightmapName, x: i32, z: i32) -> Option<i32> {
        let (slot, lx, lz) = self.locate(IVec3::new(x, 0, z))?;
        let snapshot = &self.snapshots[slot];
        let maps = || {
            if slot == 4 {
                self.maps.as_ref()
            } else {
                snapshot.maps.as_ref()
            }
        };
        let heights = match kind {
            HeightmapName::WorldSurfaceWg => &snapshot.pre_carve.as_ref()?.surface,
            HeightmapName::OceanFloorWg => &snapshot.pre_carve.as_ref()?.solid,
            HeightmapName::WorldSurface => &maps()?.surface.0,
            HeightmapName::OceanFloor => &maps()?.solid.0,
            HeightmapName::MotionBlocking => &maps()?.motion.0,
            HeightmapName::MotionBlockingNoLeaves => &maps()?.no_leaves.0,
        };
        Some(heights.get(lx, lz))
    }

    /// The block-resolution biome, read through the zoom off the stored
    /// palettes: this stage holds the ring, so nothing re-evaluates climate.
    ///
    /// A quart cell the zoom picks outside the 3x3 falls back to the cell the
    /// position sits in, which is always inside it.
    fn biome_at(&self, p: IVec3) -> u32 {
        let (qx, qy, qz) = quart_cell(self.zoom_seed, p.x, p.y, p.z);
        self.quart_biome(qx, qy, qz)
            .or_else(|| self.quart_biome(p.x >> 2, p.y >> 2, p.z >> 2))
            .unwrap_or_default()
    }

    fn quart_biome(&self, qx: i32, qy: i32, qz: i32) -> Option<u32> {
        let slot = region_slot(self.center, ColumnPos::new(qx >> 2, qz >> 2))?;
        let snapshot = &self.snapshots[slot];
        let section = snapshot.slot(qy << 2)?;
        let (_, biomes) = snapshot.sections[section].as_ref()?;
        Some(
            biomes
                .0
                .get((qx & 3) as usize, (qy & 3) as usize, (qz & 3) as usize) as u32,
        )
    }

    /// One delta per column this unit wrote into, its own included: the merge of
    /// each target applies them in ascending rank, last write standing.
    pub fn finish(self) -> Vec<(ColumnPos, ColumnDelta)> {
        let source_rank = rank(self.center);
        let mut writes: [Vec<(u32, VoxelId)>; 9] = self.ring.map(|w| w.into_iter().collect());
        writes[4] = self.own;
        let mut block_entities: [Vec<GeneratedBlockEntity>; 9] = Default::default();
        for entity in self.block_entities {
            let at = entity.position();
            let col = ColumnPos::new(at.x.div_euclid(16), at.z.div_euclid(16));
            match region_slot(self.center, col) {
                Some(slot) => block_entities[slot].push(entity),
                None => debug_assert!(
                    false,
                    "a block entity at {at} left the region of {:?}",
                    self.center
                ),
            }
        }
        writes
            .into_iter()
            .zip(block_entities)
            .enumerate()
            .filter(|(_, (writes, entities))| !writes.is_empty() || !entities.is_empty())
            .map(|(slot, (writes, block_entities))| {
                let delta = ColumnDelta {
                    source_rank,
                    writes,
                    block_entities,
                };
                (region_column(self.center, slot), delta)
            })
            .collect()
    }
}

impl Volume for ColumnRegion<'_> {
    fn min(&self) -> IVec3 {
        let first = self.snapshots[4].y_sections.first().copied().unwrap_or(0);
        IVec3::new(
            (self.center.x - 1) * 16,
            first * 16,
            (self.center.z - 1) * 16,
        )
    }

    fn max(&self) -> IVec3 {
        let last = self.snapshots[4].y_sections.last().copied().unwrap_or(-1);
        IVec3::new(
            (self.center.x + 2) * 16 - 1,
            (last + 1) * 16 - 1,
            (self.center.z + 2) * 16 - 1,
        )
    }
}

impl Blocks for ColumnRegion<'_> {
    fn get(&self, p: IVec3) -> VoxelId {
        let Some((slot, lx, lz)) = self.locate(p) else {
            debug_assert!(false, "a read at {p} left the region of {:?}", self.center);
            return VoxelId::default();
        };
        if slot == 4 {
            return self
                .blocks
                .get(lx as i32, p.y, lz as i32)
                .unwrap_or_default();
        }
        let snapshot = &self.snapshots[slot];
        let Some(section) = snapshot.slot(p.y) else {
            return VoxelId::default();
        };
        let local_y = (p.y & 0xF) as usize;
        match self.ring[slot].get(&cell_index(section, lx, local_y, lz)) {
            Some(written) => *written,
            None => snapshot.sections[section]
                .as_ref()
                .map_or(VoxelId::default(), |(blocks, _)| {
                    blocks.0.get(lx, local_y, lz)
                }),
        }
    }
}

impl BlocksMut for ColumnRegion<'_> {
    fn set(&mut self, p: IVec3, state: VoxelId) {
        let Some((slot, lx, lz)) = self.locate(p) else {
            debug_assert!(false, "a write at {p} left the region of {:?}", self.center);
            return;
        };
        let snapshot = &self.snapshots[slot];
        let Some(section) = snapshot.slot(p.y) else {
            return;
        };
        let cell = cell_index(section, lx, (p.y & 0xF) as usize, lz);
        if slot != 4 {
            self.ring[slot].insert(cell, state);
            return;
        }
        self.blocks.set(lx as i32, p.y, lz as i32, state);
        self.own.push((cell, state));
        if let (Some(predicates), Some(maps)) = (&self.ctx.predicates, self.maps.as_mut()) {
            let kinds = predicates.get(state);
            let read = |at: i32| {
                self.blocks
                    .get(lx as i32, at, lz as i32)
                    .unwrap_or_default()
            };
            maps.apply_write(lx, lz, p.y, kinds, predicates, &read);
        }
    }
}

impl WorldGenVolume for ColumnRegion<'_> {
    /// Only a generator asks, and a dimension without a program runs none; a
    /// default here would answer every state question with the empty set and
    /// place blocks that belong nowhere.
    fn world(&self) -> &WorldStates {
        &self
            .ctx
            .features()
            .expect("a region asked for the world states without a feature program")
            .world
    }

    fn height(&self, kind: HeightmapName, x: i32, z: i32) -> i32 {
        self.map_height(kind, x, z)
            .unwrap_or(self.ctx.router.noise.min_y)
    }

    fn biome(&self, p: IVec3) -> u32 {
        self.biome_at(p)
    }

    fn extent(&self) -> HeightContext {
        extent(&self.ctx.router)
    }

    fn would_survive(&self, state: VoxelId, p: IVec3) -> bool {
        let program = self
            .ctx
            .features()
            .expect("a region tested survival without a feature program");
        program.would_survive(program.world.block_of(state), p, |q| self.get(q))
    }
}

/// `Run`: the steps of one rung of a column's own program against its 3×3
/// region, which holds every earlier rung of all nine columns already merged.
pub fn run_column(ctx: &FillContext, region: &mut ColumnRegion, rung: usize) {
    let col = region.center();
    let Some(program) = ctx.features() else {
        return;
    };
    let Some(steps) = program.rungs().get(rung).cloned() else {
        return;
    };

    // The region's biomes are the distinct palette entries of all nine
    // snapshots, and a biome the source cannot answer with has no slot, which
    // is the reference's intersection with `possibleBiomes`.
    let mut slots: Vec<usize> = Vec::new();
    for snapshot in region.snapshots {
        for section in snapshot.sections.iter().flatten() {
            section.1.for_each_distinct(|biome| {
                if let Some(slot) = program.slot_of(biome as u32)
                    && !slots.contains(&slot)
                {
                    slots.push(slot);
                }
            });
        }
    }
    let present = program.present(&slots);
    if present[steps.clone()].iter().all(FixedBitSet::is_clear) {
        return;
    }

    let origin = IVec3::new(col.x * 16, ctx.router.noise.min_y, col.z * 16);
    let seed = decoration_seed(ctx.router.world_seed as i64, origin.x, origin.z);
    thread_local! {
        static SCRATCH: RefCell<(PlacerScratch, RunScratch)> = RefCell::default();
    }
    SCRATCH.with_borrow_mut(|(scratch, pool)| {
        let mut run = program.run(std::mem::take(pool));
        decorate(
            &present,
            steps,
            &|step, index| program.chain(step, index),
            &|biome, step, index| program.carries(biome, step, index),
            region,
            scratch,
            origin,
            seed,
            &mut |(step, index), region, rng, at, carries| match program.generator_at(step, index) {
                Some(generator) => generator.place(&mut run, region, rng, at, carries),
                None => false,
            },
        );
        (region.block_entities, *pool) = run.finish();
    });
}

/// `setDecorationSeed`: the unit seed every object of one column is offset from.
pub(crate) fn decoration_seed(world_seed: i64, origin_x: i32, origin_z: i32) -> i64 {
    let mut rng = XoroshiroRandom::new(world_seed as u64);
    let a = Random::next_i64(&mut rng) | 1;
    let b = Random::next_i64(&mut rng) | 1;
    (origin_x as i64)
        .wrapping_mul(a)
        .wrapping_add((origin_z as i64).wrapping_mul(b))
        ^ world_seed
}

/// `Merged`: the column's own writes and the eight incoming deltas, applied in
/// the order given — ascending rank, as the store hands them out — then the
/// four maps rebuilt because a neighbour's write can raise them.
pub fn merge_column(
    snapshot: &FilledSnapshot,
    deltas: &[Arc<ColumnDelta>],
    predicates: Option<&HeightmapPredicates>,
) -> FilledSnapshot {
    let _span = info_span!("world::column_merge").entered();
    let mut merged = FilledSnapshot {
        col: snapshot.col,
        y_sections: snapshot.y_sections.clone(),
        sections: snapshot.sections.clone(),
        // The two `_WG` maps are the terrain's, and the reference stops
        // updating them once the terrain step is done; a rung reads them, so
        // they travel with the column rather than being dropped at the merge.
        pre_carve: snapshot.pre_carve.clone(),
        maps: snapshot.maps.clone(),
        source: snapshot.source,
        block_entities: snapshot
            .block_entities
            .iter()
            .chain(deltas.iter().flat_map(|delta| delta.block_entities.iter()))
            .cloned()
            .collect(),
    };
    if deltas.iter().all(|delta| delta.writes.is_empty()) {
        return merged;
    }
    for &(cell, state) in deltas.iter().flat_map(|delta| &delta.writes) {
        let (slot, index) = (
            cell as usize / ColumnBlocks::SECTION_VOLUME,
            cell as usize % ColumnBlocks::SECTION_VOLUME,
        );
        if let Some(Some((blocks, _))) = merged.sections.get_mut(slot) {
            blocks
                .0
                .set(index & 15, index >> 8, (index >> 4) & 15, state);
        }
    }
    merged.maps =
        predicates.and_then(|p| build_column_heightmaps(&merged.sections, &merged.y_sections, p));
    merged
}

/// [`fill_column`] over the worker's own reused buffer.
pub fn fill_pooled(
    ctx: &FillContext,
    col: ColumnPos,
    cancel: &CancellationToken,
) -> Option<FilledSnapshot> {
    with_column_buffer(|column| fill_column(ctx, col, column, cancel))
}

/// Unpack the centre snapshot into a worker's dense buffer and run one rung.
pub fn run_region(
    ctx: &FillContext,
    snapshots: &RegionSnapshots,
    rung: usize,
) -> Vec<(ColumnPos, ColumnDelta)> {
    let centre = &snapshots[4];
    let _span = info_span!("world::column_run").entered();
    with_column_buffer(|column| {
        column.reset(&centre.y_sections);
        column.unpack(&centre.sections);
        let mut region = ColumnRegion::new(snapshots, column, ctx);
        run_column(ctx, &mut region, rung);
        region.finish()
    })
}

/// The section list every column of one dimension is generated over.
///
/// One list for the whole dimension, not one per dispatch: a delta names a cell
/// by its index in this list, so two columns that disagreed on it would write
/// past each other, and the three stages of one column would not line up
/// section for section.
pub fn dimension_y_sections(router: &NoiseRouter, min_y: i32, section_count: u32) -> Arc<[i32]> {
    let config_bottom = min_y >> BLOCKS::BITS;
    let noise_bottom = router.noise.min_y >> BLOCKS::BITS;
    let bottom = config_bottom.min(noise_bottom);
    let top = (config_bottom + section_count as i32)
        .max(noise_bottom + (router.noise.height as i32 >> BLOCKS::BITS));
    (bottom..top).collect()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::format::anvil::SectionData;
    use crate::world::generate::staging::StagingStore;
    use crate::world::generate::tests::{bare_fill_context, build_beta_router, generate_region};

    fn flat_snapshot(col: ColumnPos, y_sections: &Arc<[i32]>, fill: VoxelId) -> FilledSnapshot {
        crate::world::generate::tests::flat_snapshot(col, y_sections, |_| Some(fill), None)
    }

    fn region_of(center: ColumnPos, y_sections: &Arc<[i32]>, fill: VoxelId) -> RegionSnapshots {
        crate::world::generate::tests::region_of(center, |col| flat_snapshot(col, y_sections, fill))
    }

    #[test]
    fn the_writer_keeps_the_centre_and_routes_the_ring() {
        let y_sections: Arc<[i32]> = Arc::from(vec![0i32, 1]);
        let center = ColumnPos::new(-1, 2);
        let snapshots = region_of(center, &y_sections, VoxelId(1));
        let column = ColumnBlocks::new(&y_sections);
        column.unpack(&snapshots[4].sections);
        let ctx = bare_fill_context(build_beta_router());
        let mut region = ColumnRegion::new(&snapshots, &column, &ctx);

        let (cx, cz) = (center.x * 16 + 3, center.z * 16 + 4);
        region.set(IVec3::new(cx, 20, cz), VoxelId(7));
        region.set(IVec3::new(cx - 16, 20, cz), VoxelId(8));
        region.set(IVec3::new(cx, 20, cz + 16), VoxelId(9));
        region.set(IVec3::new(cx, 300, cz), VoxelId(10));

        assert_eq!(
            region.get(IVec3::new(cx, 20, cz)),
            VoxelId(7),
            "own centre write"
        );
        assert_eq!(
            region.get(IVec3::new(cx - 16, 20, cz)),
            VoxelId(8),
            "own ring write"
        );
        assert_eq!(
            region.get(IVec3::new(cx, 21, cz)),
            VoxelId(1),
            "the filled block"
        );
        assert_eq!(
            region.get(IVec3::new(cx, 300, cz)),
            VoxelId::default(),
            "above the column"
        );

        let deltas = region.finish();
        assert_eq!(deltas.len(), 3, "centre plus two ring columns");
        for (target, delta) in &deltas {
            assert_eq!(delta.source_rank, rank(center));
            assert_eq!(delta.writes.len(), 1, "{target:?} took one write");
        }
        assert!(deltas.iter().any(|(target, _)| *target == center));
    }

    /// Within one rung the nine columns of a 3×3 run at once, so a neighbour's
    /// write reaches a column only at the merge that closes the rung. What the
    /// reference gets from decorating into shared chunks, the ladder gets
    /// between rungs instead: the next rung reads the merge.
    #[test]
    fn a_run_reads_the_neighbour_as_the_rung_below_left_it() {
        let y_sections: Arc<[i32]> = Arc::from(vec![0i32, 1]);
        let left = ColumnPos::new(0, 0);
        let right = ColumnPos::new(1, 0);
        let ctx = bare_fill_context(build_beta_router());
        let shared = IVec3::new(right.x * 16 + 2, 20, right.z * 16 + 2);

        // `left` decorates into `right`.
        let snapshots = region_of(left, &y_sections, VoxelId(1));
        let column = ColumnBlocks::new(&y_sections);
        column.unpack(&snapshots[4].sections);
        let mut region = ColumnRegion::new(&snapshots, &column, &ctx);
        region.set(shared, VoxelId(7));
        let deltas = region.finish();
        assert!(
            deltas.iter().any(|(target, _)| *target == right),
            "the write is routed to the neighbour"
        );

        // `right` runs from its own filled snapshot and does not see it.
        let snapshots = region_of(right, &y_sections, VoxelId(1));
        let column = ColumnBlocks::new(&y_sections);
        column.unpack(&snapshots[4].sections);
        let region = ColumnRegion::new(&snapshots, &column, &ctx);
        assert_eq!(
            region.get(shared),
            VoxelId(1),
            "a neighbour's write reaches the column at the merge, not during its run"
        );
    }

    #[test]
    fn a_merge_applies_deltas_in_ascending_rank() {
        let y_sections: Arc<[i32]> = Arc::from(vec![0i32]);
        let col = ColumnPos::new(0, 0);
        let snapshot = flat_snapshot(col, &y_sections, VoxelId(1));
        let cell = cell_index(0, 5, 6, 7);
        let mut store = StagingStore::default();
        // Ranks 7, 2 and 5 of the 3×3 around the origin, pushed in an order
        // that is neither their rank nor their position.
        for source in [
            ColumnPos::new(-1, 1),
            ColumnPos::new(0, -1),
            ColumnPos::new(1, -1),
        ] {
            let rank = rank(source);
            store.push_delta(
                source,
                0,
                col,
                ColumnDelta {
                    source_rank: rank,
                    writes: vec![(cell, VoxelId(rank as u16 * 10))],
                    block_entities: Vec::new(),
                },
            );
        }
        let merged = merge_column(&snapshot, &store.deltas(col, 0), None);
        let (palette, _) = merged.sections[0].as_ref().expect("the section survives");
        assert_eq!(
            palette.0.get(5, 6, 7),
            VoxelId(70),
            "the highest rank stands whatever order the deltas arrived in"
        );
        assert_eq!(palette.0.get(0, 0, 0), VoxelId(1), "the fill is untouched");
    }

    /// A saved column merges against no deltas at all, so anything the save
    /// held has only the snapshot to arrive in.
    #[test]
    fn a_merge_carries_the_block_entities_the_snapshot_arrived_with() {
        let y_sections: Arc<[i32]> = Arc::from(vec![0i32]);
        let col = ColumnPos::new(1, -1);
        let mut snapshot = flat_snapshot(col, &y_sections, VoxelId(1));
        snapshot.source = ColumnSource::Saved;
        snapshot.block_entities = vec![GeneratedBlockEntity::Beehive {
            x: 20,
            y: 70,
            z: -10,
            bees: Vec::new(),
        }];

        let merged = merge_column(&snapshot, &[], None);
        assert_eq!(
            merged.block_entities, snapshot.block_entities,
            "a saved column's block entities reach its delivery"
        );

        let delta = Arc::new(ColumnDelta {
            source_rank: rank(col),
            writes: Vec::new(),
            block_entities: vec![GeneratedBlockEntity::Chest {
                x: 21,
                y: 71,
                z: -11,
                loot_table: "minecraft:chests/simple_dungeon".to_owned(),
                loot_table_seed: 0,
            }],
        });
        let merged = merge_column(&snapshot, std::slice::from_ref(&delta), None);
        assert_eq!(
            merged.block_entities.len(),
            2,
            "and the ones a neighbour's run grew join them"
        );
    }

    #[test]
    fn a_merge_with_no_writes_is_the_filled_column() {
        let ctx = bare_fill_context(build_beta_router());
        let mut buffer = ColumnBlocks::new(&ctx.y_sections);
        let snapshot = fill_column(
            &ctx,
            ColumnPos::new(2, -3),
            &mut buffer,
            &CancellationToken::new(),
        )
        .expect("the fill was not cancelled");

        let merged = merge_column(&snapshot, &[], None);
        assert_same_sections(&merged.sections, &snapshot.sections);
    }

    fn assert_same_sections(got: &[Option<SectionData>], want: &[Option<SectionData>]) {
        assert_eq!(got.len(), want.len());
        for (index, (got, want)) in got.iter().zip(want).enumerate() {
            let (got, _) = got.as_ref().expect("a merged section");
            let (want, _) = want.as_ref().expect("a filled section");
            for cell in 0..ColumnBlocks::SECTION_VOLUME {
                let (x, y, z) = (cell & 15, cell >> 8, (cell >> 4) & 15);
                assert_eq!(
                    got.0.get(x, y, z),
                    want.0.get(x, y, z),
                    "section {index} diverged at ({x}, {y}, {z})"
                );
            }
        }
    }

    #[test]
    fn the_oracle_merges_the_column_the_fill_produced() {
        let ctx = bare_fill_context(build_beta_router());
        let col = ColumnPos::new(0, 0);
        let region = generate_region(&ctx, col, col);
        let merged = region.get(&col).expect("the oracle merged the column");

        let mut buffer = ColumnBlocks::new(&ctx.y_sections);
        let alone = fill_column(&ctx, col, &mut buffer, &CancellationToken::new())
            .expect("the fill was not cancelled");
        assert_same_sections(&merged.sections, &alone.sections);
    }
}
