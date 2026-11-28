use crate::structures::index::CLIMATE_ROOTS;
use crate::{ColumnBlocks, beta_chunk_seed};
use glam::IVec3;
use mcrs_minecraft_biome::Biome;
use mcrs_minecraft_biome::climate::{ParameterList, ParameterPoint, TargetPoint};
use mcrs_minecraft_biome::source::{BetaLandBiome, BiomeSource, beta_biome_from_climate};
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_noise::sample_grid::SampleGrid;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_resource::Id;
use mcrs_minecraft_value_provider::HeightContext;
use mcrs_minecraft_worldgen_carver::beta::carve_beta_caves;
use mcrs_minecraft_worldgen_carver::config::CarverConfig;
use mcrs_minecraft_worldgen_carver::mask::CarvingMask;
use mcrs_minecraft_worldgen_carver::modern::{SOURCE_RADIUS, carve_source_into};
use mcrs_minecraft_worldgen_carver::target::{Region, SingleColumn};
use mcrs_minecraft_worldgen_carver::water::WaterMask;
use mcrs_minecraft_worldgen_density::aquifer::{FluidField, point_barrier};
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_density::router::{NoiseRouter, TEMPERATURE, VEGETATION};
use std::collections::HashMap;
use std::hash::Hash;
use std::ops::Deref;
use std::sync::{Arc, Mutex, OnceLock};

pub fn climate_target_at(
    router: &NoiseRouter,
    ws: &mut Workspace,
    quart_x: i32,
    quart_y: i32,
    quart_z: i32,
) -> TargetPoint {
    let volume = SampleGrid::new(
        IVec3::ONE,
        IVec3::new(quart_x * 4, quart_y * 4, quart_z * 4),
        IVec3::ONE,
    );
    let mut values = [0.0f32; 6];
    router.fill_roots(ws, &volume, &CLIMATE_ROOTS, &mut values);
    TargetPoint::new(
        values[0], values[1], values[2], values[3], values[4], values[5],
    )
}

pub struct CarverBiomeTable {
    biomes: SourceBiomes,
    tiles: Mutex<SourceTiles>,
    width: i32,
    regions: Option<RegionCache>,
}

enum SourceBiomes {
    Climate(Box<ParameterList<Arc<[CarverConfig]>>>),
    Beta {
        lookup: Arc<[[BetaLandBiome; 64]; 64]>,
        land: Box<[Arc<[CarverConfig]>]>,
    },
}

impl SourceBiomes {
    fn carvers(&self, slot: u16) -> &[CarverConfig] {
        match self {
            SourceBiomes::Climate(table) => &table.values()[usize::from(slot)].1,
            SourceBiomes::Beta { land, .. } => &land[usize::from(slot)],
        }
    }

    fn lists(&self) -> Box<dyn Iterator<Item = &Arc<[CarverConfig]>> + '_> {
        match self {
            SourceBiomes::Climate(table) => {
                Box::new(table.values().iter().map(|(_, carvers)| carvers))
            }
            SourceBiomes::Beta { land, .. } => Box::new(land.iter()),
        }
    }
}

const TILE: i32 = 16;

type Tile = Arc<[u16; (TILE * TILE) as usize]>;

type TileKey = (u64, i32, i32);

type SourceTiles = Lru<TileKey, Tile>;

const KEPT: usize = 256;

struct Lru<K, V> {
    entries: Vec<(K, u64, V)>,
    clock: u64,
}

impl<K, V> Default for Lru<K, V> {
    fn default() -> Self {
        Lru {
            entries: Vec::new(),
            clock: 0,
        }
    }
}

impl<K: PartialEq, V: Clone> Lru<K, V> {
    fn get(&mut self, key: &K) -> Option<V> {
        self.clock += 1;
        let (_, used, value) = self.entries.iter_mut().find(|(at, ..)| at == key)?;
        *used = self.clock;
        Some(value.clone())
    }

    fn get_or_insert_with(&mut self, key: K, capacity: usize, value: impl FnOnce() -> V) -> V {
        if let Some(held) = self.get(&key) {
            return held;
        }
        let value = value();
        let entry = (key, self.clock, value.clone());
        if self.entries.len() < capacity {
            self.entries.push(entry);
        } else {
            let stale = self
                .entries
                .iter()
                .enumerate()
                .min_by_key(|(_, (_, used, _))| *used)
                .map(|(index, _)| index)
                .expect("a full cache has entries");
            self.entries[stale] = entry;
        }
        value
    }
}

pub const REGION_WIDTH: i32 = 2;

pub const REGION_CAPACITY: usize = 256;

#[derive(Clone, Copy, PartialEq, Eq)]
struct RegionKey {
    world_seed: i64,
    router_seed: u64,
    min_y: i32,
    depth: i32,
    sea_level: i32,
    x: i32,
    z: i32,
}

type RegionCell = OnceLock<Arc<[CarvingMask]>>;

struct RegionCache {
    capacity: usize,
    list: Mutex<Lru<RegionKey, Arc<RegionCell>>>,
    #[cfg(test)]
    builds: std::sync::atomic::AtomicUsize,
}

impl RegionCache {
    fn new(capacity: usize) -> Self {
        RegionCache {
            capacity,
            list: Mutex::default(),
            #[cfg(test)]
            builds: Default::default(),
        }
    }

    fn get_or_build(
        &self,
        key: RegionKey,
        build: impl FnOnce() -> Arc<[CarvingMask]>,
    ) -> Arc<[CarvingMask]> {
        let cell = self
            .list
            .lock()
            .expect("carver regions")
            .get_or_insert_with(key, self.capacity, Arc::default);
        cell.get_or_init(|| {
            #[cfg(test)]
            self.builds
                .fetch_add(1, std::sync::atomic::Ordering::Relaxed);
            build()
        })
        .clone()
    }
}

pub struct RegionSlot {
    region: Arc<[CarvingMask]>,
    slot: usize,
}

impl Deref for RegionSlot {
    type Target = CarvingMask;

    #[inline]
    fn deref(&self) -> &CarvingMask {
        &self.region[self.slot]
    }
}

#[cfg(test)]
impl RegionSlot {
    pub fn same_region_as(&self, other: &RegionSlot) -> bool {
        Arc::ptr_eq(&self.region, &other.region)
    }
}

impl CarverBiomeTable {
    pub fn entry_count(&self) -> usize {
        self.biomes.lists().count()
    }

    pub fn runs(&self, kind: impl Fn(&CarverConfig) -> bool) -> bool {
        self.biomes.lists().any(|carvers| carvers.iter().any(&kind))
    }
}

impl CarverBiomeTable {
    pub fn from_climate<B: Copy + Eq + Hash>(
        climate: &ParameterList<B>,
        lookup: impl Fn(B) -> Arc<[CarverConfig]>,
    ) -> CarverBiomeTable {
        let mut resolved: HashMap<B, Arc<[CarverConfig]>> = HashMap::new();
        let values = climate.map_values(|&biome| {
            resolved
                .entry(biome)
                .or_insert_with(|| lookup(biome))
                .clone()
        });
        Self::with_biomes(SourceBiomes::Climate(Box::new(values)))
            .with_region(REGION_WIDTH, REGION_CAPACITY)
    }

    pub fn from_entries<B: Clone + Eq + Hash>(
        entries: Vec<(ParameterPoint, B)>,
        lookup: impl Fn(&B) -> Arc<[CarverConfig]>,
    ) -> Option<CarverBiomeTable> {
        if entries.is_empty() {
            return None;
        }
        let mut resolved = HashMap::new();
        let values = entries
            .into_iter()
            .map(|(point, biome)| {
                let carvers = resolved
                    .entry(biome.clone())
                    .or_insert_with(|| lookup(&biome))
                    .clone();
                (point, carvers)
            })
            .collect();
        Some(
            Self::with_biomes(SourceBiomes::Climate(Box::new(ParameterList::new(values))))
                .with_region(REGION_WIDTH, REGION_CAPACITY),
        )
    }

    pub fn beta(
        source: &BiomeSource,
        lookup: impl Fn(Id<Biome>) -> Arc<[CarverConfig]>,
    ) -> Option<CarverBiomeTable> {
        let BiomeSource::Beta {
            land_biomes,
            lookup: grid,
        } = source
        else {
            return None;
        };
        Some(Self::with_biomes(SourceBiomes::Beta {
            lookup: grid.clone(),
            land: land_biomes.iter().map(|id| lookup(*id)).collect(),
        }))
    }

    fn with_biomes(biomes: SourceBiomes) -> CarverBiomeTable {
        CarverBiomeTable {
            biomes,
            tiles: Mutex::default(),
            width: 1,
            regions: None,
        }
    }

    pub fn with_region(mut self, width: i32, capacity: usize) -> Self {
        assert!(
            matches!(self.biomes, SourceBiomes::Climate(_)),
            "a Beta table carves one column at a time"
        );
        assert!(
            (1..=Region::MAX_WIDTH).contains(&width),
            "a region is at most {} columns wide, not {width}",
            Region::MAX_WIDTH
        );
        assert!(
            width == 1 || capacity > 0,
            "a region of several columns needs room to be kept"
        );
        self.width = width;
        self.regions = (width > 1).then(|| RegionCache::new(capacity));
        self
    }

    pub fn region_bytes_bound(&self, height: HeightContext) -> usize {
        self.regions.as_ref().map_or(0, |regions| {
            regions.capacity * (self.width * self.width) as usize * column_mask_bytes(height)
        })
    }

    fn carvers_of_source<'a>(
        &'a self,
        router: &NoiseRouter,
        ws: &mut Workspace,
        source_x: i32,
        source_z: i32,
        held: &mut Vec<((i32, i32), Tile)>,
    ) -> &'a [CarverConfig] {
        let key = (source_x.div_euclid(TILE), source_z.div_euclid(TILE));
        let tile = match held.iter().position(|(at, _)| *at == key) {
            Some(index) => &held[index].1,
            None => {
                held.push((key, self.tile(router, ws, key)));
                &held.last().expect("just pushed").1
            }
        };
        let slot = tile[(source_x.rem_euclid(TILE) * TILE + source_z.rem_euclid(TILE)) as usize];
        self.biomes.carvers(slot)
    }

    fn tile(&self, router: &NoiseRouter, ws: &mut Workspace, (tile_x, tile_z): (i32, i32)) -> Tile {
        let key = (router.world_seed, tile_x, tile_z);
        if let Some(tile) = self.tiles.lock().expect("carver tiles").get(&key) {
            return tile;
        }
        let volume = SampleGrid::new(
            IVec3::new(TILE, 1, TILE),
            IVec3::new(tile_x * TILE * 16, 0, tile_z * TILE * 16),
            IVec3::new(16, 1, 16),
        );
        let points = volume.len();
        let mut slots = [0u16; (TILE * TILE) as usize];
        match &self.biomes {
            SourceBiomes::Climate(table) => {
                let mut values = vec![0.0f32; CLIMATE_ROOTS.len() * points];
                router.fill_roots(ws, &volume, &CLIMATE_ROOTS, &mut values);
                let mut cursor = table.cursor();
                for dx in 0..TILE {
                    for dz in 0..TILE {
                        let at = volume.index_unchecked(dx, 0, dz);
                        let target = TargetPoint::new(
                            values[at],
                            values[points + at],
                            values[2 * points + at],
                            values[3 * points + at],
                            values[4 * points + at],
                            values[5 * points + at],
                        );
                        if let Some(slot) = cursor.find_slot(target) {
                            slots[(dx * TILE + dz) as usize] =
                                u16::try_from(slot).expect("a climate table fits in u16 slots");
                        }
                    }
                }
            }
            SourceBiomes::Beta { lookup, .. } => {
                let roots = [TEMPERATURE, VEGETATION];
                let mut values = vec![0.0f32; roots.len() * points];
                router.fill_roots(ws, &volume, &roots, &mut values);
                for dx in 0..TILE {
                    for dz in 0..TILE {
                        let at = volume.index_unchecked(dx, 0, dz);
                        let biome =
                            beta_biome_from_climate(lookup, values[at], values[points + at]);
                        slots[(dx * TILE + dz) as usize] = biome as u16;
                    }
                }
            }
        }
        self.tiles
            .lock()
            .expect("carver tiles")
            .get_or_insert_with(key, KEPT, || Arc::new(slots))
    }

    #[cfg(test)]
    pub fn region_entries_for_test(&self) -> Option<usize> {
        self.regions
            .as_ref()
            .map(|regions| regions.list.lock().expect("carver regions").entries.len())
    }

    #[cfg(test)]
    pub fn region_builds_for_test(&self) -> usize {
        self.regions.as_ref().map_or(0, |regions| {
            regions.builds.load(std::sync::atomic::Ordering::Relaxed)
        })
    }

    #[cfg(test)]
    pub fn carvers_at_for_test(&self, target: TargetPoint) -> &[CarverConfig] {
        let SourceBiomes::Climate(table) = &self.biomes else {
            panic!("only a climate table answers a climate target");
        };
        table
            .find_value(target)
            .expect("a climate table answers every target")
    }

    #[cfg(test)]
    pub fn carvers_of_source_for_test(
        &self,
        router: &NoiseRouter,
        ws: &mut Workspace,
        source_x: i32,
        source_z: i32,
    ) -> &[CarverConfig] {
        self.carvers_of_source(router, ws, source_x, source_z, &mut Vec::new())
    }
}

pub struct ModernCarverBlockIds {
    uncarvable: Box<[VoxelId]>,
}

impl ModernCarverBlockIds {
    pub fn resolve(
        blocks: &BlockDefinitions,
        uncarvable: impl IntoIterator<Item = Id<Block>>,
    ) -> Self {
        let uncarvable = uncarvable
            .into_iter()
            .filter_map(|id| blocks.blocks().get(id.index()))
            .flat_map(|entry| {
                (0..entry.state_count)
                    .filter_map(move |offset| entry.state_at(offset))
                    .map(|state| VoxelId::from(state.number()))
            })
            .collect();
        ModernCarverBlockIds { uncarvable }
    }

    #[cfg(any(test, feature = "test-support"))]
    pub fn for_test(uncarvable: Vec<VoxelId>) -> Self {
        ModernCarverBlockIds {
            uncarvable: uncarvable.into_boxed_slice(),
        }
    }

    #[inline]
    fn is_uncarvable(&self, state: VoxelId) -> bool {
        self.uncarvable.contains(&state)
    }
}

const PROTECTED_BLOCKS_ON_TOP: i32 = 7;

pub(crate) fn carving_mask(height: HeightContext) -> CarvingMask {
    let (bottom, top) = mask_range(height);
    CarvingMask::new(bottom, top)
}

fn mask_range(height: HeightContext) -> (i32, i32) {
    (
        height.min_y + 1,
        height.min_y + height.depth - 1 - PROTECTED_BLOCKS_ON_TOP,
    )
}

fn column_mask_bytes(height: HeightContext) -> usize {
    let (bottom, top) = mask_range(height);
    (16 * 16 * (top - bottom + 1).max(0) as usize).div_ceil(8)
}

#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::too_many_arguments,
        reason = "independent inputs of one chunk's carving"
    )
)]
pub(crate) fn carve_sources(
    water: impl Fn() -> WaterMask,
    chunk_x: i32,
    chunk_z: i32,
    world_seed: i64,
    router: &NoiseRouter,
    ws: &mut Workspace,
    biomes: &CarverBiomeTable,
    height: HeightContext,
) -> CarvingMask {
    let mut mask = carving_mask(height);
    let no_water = WaterMask::default();
    let mut beta_water = None;

    let mut held = Vec::with_capacity(4);

    for source_x in (chunk_x - SOURCE_RADIUS)..=(chunk_x + SOURCE_RADIUS) {
        for source_z in (chunk_z - SOURCE_RADIUS)..=(chunk_z + SOURCE_RADIUS) {
            let carvers = biomes.carvers_of_source(router, ws, source_x, source_z, &mut held);
            for (index, config) in carvers.iter().enumerate() {
                if matches!(config, CarverConfig::BetaCave) {
                    let seed = beta_chunk_seed(world_seed, source_x, source_z);
                    carve_beta_caves(
                        chunk_x,
                        chunk_z,
                        source_x,
                        source_z,
                        beta_water.get_or_insert_with(&water),
                        &mut mask,
                        &mut LegacyRandom::new(seed as u64),
                    );
                } else {
                    carve_source_into(
                        config,
                        index,
                        world_seed,
                        height,
                        &mut SingleColumn::new(chunk_x, chunk_z, &no_water, &mut mask),
                        source_x,
                        source_z,
                    );
                }
            }
        }
    }
    mask
}

fn carve_region(
    origin: (i32, i32),
    width: i32,
    world_seed: i64,
    router: &NoiseRouter,
    ws: &mut Workspace,
    biomes: &CarverBiomeTable,
    height: HeightContext,
) -> Arc<[CarvingMask]> {
    let mut masks: Arc<[CarvingMask]> = (0..width * width).map(|_| carving_mask(height)).collect();
    let slots = Arc::get_mut(&mut masks).expect("a region nobody else holds yet");
    let mut region = Region::new(origin.0, origin.1, width, slots);
    let mut held = Vec::with_capacity(4);

    for source_x in (origin.0 - SOURCE_RADIUS)..=(origin.0 + width - 1 + SOURCE_RADIUS) {
        for source_z in (origin.1 - SOURCE_RADIUS)..=(origin.1 + width - 1 + SOURCE_RADIUS) {
            let carvers = biomes.carvers_of_source(router, ws, source_x, source_z, &mut held);
            for (index, config) in carvers.iter().enumerate() {
                carve_source_into(
                    config,
                    index,
                    world_seed,
                    height,
                    &mut region,
                    source_x,
                    source_z,
                );
            }
        }
    }
    masks
}

pub fn modern_carving_mask(
    chunk_x: i32,
    chunk_z: i32,
    world_seed: i64,
    router: &NoiseRouter,
    ws: &mut Workspace,
    biomes: &CarverBiomeTable,
    height: HeightContext,
) -> RegionSlot {
    let width = biomes.width;
    let (region_x, region_z) = (chunk_x.div_euclid(width), chunk_z.div_euclid(width));
    let slot = (chunk_x.rem_euclid(width) * width + chunk_z.rem_euclid(width)) as usize;
    let mut build = || {
        carve_region(
            (region_x * width, region_z * width),
            width,
            world_seed,
            router,
            ws,
            biomes,
            height,
        )
    };
    let region = match &biomes.regions {
        Some(regions) => regions.get_or_build(
            RegionKey {
                world_seed,
                router_seed: router.world_seed,
                min_y: height.min_y,
                depth: height.depth,
                sea_level: height.sea_level,
                x: region_x,
                z: region_z,
            },
            build,
        ),
        None => build(),
    };
    RegionSlot { region, slot }
}

pub struct TerrainCarving<'a, 'f> {
    pub mask: &'a CarvingMask,
    pub ids: &'a ModernCarverBlockIds,
    pub fluid: &'a mut FluidField<'f>,
    pub router: &'a NoiseRouter,
    pub ws: Workspace,
    pub block_x: i32,
    pub block_z: i32,
}

impl<'a, 'f> TerrainCarving<'a, 'f> {
    pub fn new(
        mask: &'a CarvingMask,
        ids: &'a ModernCarverBlockIds,
        fluid: &'a mut FluidField<'f>,
        router: &'a NoiseRouter,
        chunk_x: i32,
        chunk_z: i32,
    ) -> Self {
        Self {
            mask,
            ids,
            fluid,
            router,
            ws: Workspace::new(),
            block_x: chunk_x * 16,
            block_z: chunk_z * 16,
        }
    }

    #[inline]
    pub fn is_carved(&self, x: i32, y: i32, z: i32) -> bool {
        self.mask.contains(x, y, z)
    }

    #[inline]
    #[must_use]
    pub fn highest_carved(&self, x: i32, z: i32, bottom: i32, top: i32) -> Option<i32> {
        self.mask.highest_carved(x, z, bottom, top)
    }

    pub fn substance(&mut self, x: i32, y: i32, z: i32, block: Option<VoxelId>) -> Option<VoxelId> {
        if !self.is_carved(x, y, z) || block.is_some_and(|block| self.ids.is_uncarvable(block)) {
            return None;
        }
        let mut barrier = point_barrier(self.router, &mut self.ws);
        self.fluid
            .substance_settled(self.block_x + x, y, self.block_z + z, 0.0, &mut barrier)
    }
}

#[cfg(test)]
#[cfg_attr(
    mcrs_lint,
    expect(
        mcrs::too_many_arguments,
        reason = "independent inputs of one chunk's carving"
    )
)]
pub fn apply_modern_carvers(
    column: &ColumnBlocks,
    chunk_x: i32,
    chunk_z: i32,
    world_seed: i64,
    router: &NoiseRouter,
    ws: &mut Workspace,
    biomes: &CarverBiomeTable,
    height: HeightContext,
    ids: &ModernCarverBlockIds,
    fluid: &mut FluidField<'_>,
) {
    let mask = modern_carving_mask(chunk_x, chunk_z, world_seed, router, ws, biomes, height);
    carve_unsurfaced(
        column,
        &mut TerrainCarving::new(&mask, ids, fluid, router, chunk_x, chunk_z),
    );
}

pub fn carve_unsurfaced(column: &ColumnBlocks, carving: &mut TerrainCarving<'_, '_>) {
    let router = carving.router;
    let open = [
        VoxelId::default(),
        router.default_fluid_state,
        router.water_state,
        router.lava_state(),
    ];
    let mask = carving.mask;
    mask.visit(|x, z, bottom_y, top_y| {
        for y in (bottom_y..=top_y).rev() {
            let Some(state) = column.get(x, y, z) else {
                continue;
            };
            if open.contains(&state) {
                continue;
            }
            if let Some(substance) = carving.substance(x, y, z, Some(state)) {
                column.set(x, y, z, substance);
            }
        }
    });
}

pub fn whole_climate_space() -> ParameterPoint {
    let full = mcrs_minecraft_biome::climate::Parameter::span(-2.0, 2.0);
    ParameterPoint {
        temperature: full,
        humidity: full,
        continentalness: full,
        erosion: full,
        depth: full,
        weirdness: full,
        offset: 0,
    }
}

#[cfg(test)]
mod source_tiles {
    use super::*;

    fn tile(fill: u16) -> Tile {
        Arc::new([fill; (TILE * TILE) as usize])
    }

    #[test]
    fn keeps_the_recently_used_tiles_and_drops_the_stalest() {
        let mut tiles = SourceTiles::default();
        for index in 0..KEPT {
            tiles.get_or_insert_with((0, index as i32, 0), KEPT, || tile(index as u16));
        }
        assert_eq!(tiles.entries.len(), KEPT);

        assert!(tiles.get(&(0, 0, 0)).is_some());
        tiles.get_or_insert_with((0, -1, 0), KEPT, || tile(u16::MAX));

        assert_eq!(tiles.entries.len(), KEPT);
        assert!(tiles.get(&(0, 0, 0)).is_some(), "the touched tile stays");
        assert!(
            tiles.get(&(0, 1, 0)).is_none(),
            "the stalest tile is evicted"
        );
        assert_eq!(tiles.get(&(0, -1, 0)).map(|t| t[0]), Some(u16::MAX));
    }

    #[test]
    fn a_racing_insert_keeps_the_first_tile() {
        let mut tiles = SourceTiles::default();
        let first = tiles.get_or_insert_with((7, 1, 2), KEPT, || tile(1));
        let second = tiles.get_or_insert_with((7, 1, 2), KEPT, || tile(2));
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(tiles.entries.len(), 1);
    }
}
