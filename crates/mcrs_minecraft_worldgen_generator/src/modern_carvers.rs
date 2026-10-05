use crate::structures::index::CLIMATE_ROOTS;
use crate::{ColumnBlocks, beta_chunk_seed};
use bevy_math::IVec3;
use mcrs_minecraft_biome::climate::{ParameterList, ParameterPoint, TargetPoint};
use mcrs_minecraft_biome::parameter_list::Preset;
use mcrs_minecraft_biome::source::{BetaLandBiome, BiomeSource, beta_biome_from_climate};
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::value_provider::HeightContext;
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_keys::Block;
use mcrs_minecraft_keys::block_tags::UNCARVABLE;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_registry::{Entries, Registry, Tags};
use mcrs_minecraft_worldgen_carver::beta::carve_beta_caves;
use mcrs_minecraft_worldgen_carver::config::CarverConfig;
use mcrs_minecraft_worldgen_carver::mask::CarvingMask;
use mcrs_minecraft_worldgen_carver::modern::{SOURCE_RADIUS, carve_source_into};
use mcrs_minecraft_worldgen_carver::target::{Region, SingleColumn};
use mcrs_minecraft_worldgen_carver::water::WaterMask;
use mcrs_minecraft_worldgen_density::aquifer::{FluidField, point_barrier};
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_density::router::{NoiseRouter, TEMPERATURE, VEGETATION};
use mcrs_minecraft_worldgen_noise::sample_grid::SampleGrid;
use std::collections::HashMap;
use std::ops::Deref;
use std::sync::{Arc, Mutex, OnceLock};

/// The six climate roots at one quart position, which is where a carver source
/// takes its biome from.
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

/// The climate table of a dimension's biome source, with each entry resolved
/// to the carvers that biome runs.
///
/// Resolving at build time rather than per source means a source's lookup ends
/// at the carver list itself, with no name to hash on the way.
pub struct CarverBiomeTable {
    biomes: SourceBiomes,
    /// Which entry each source chunk resolves to, by tile of `TILE` x `TILE`
    /// chunks. Every column asks for the 17x17 sources around it, so the
    /// columns next to it would derive 272 of the same climates again; the
    /// reference memoises the answer on the source chunk itself.
    tiles: Mutex<SourceTiles>,
    /// Columns per side of the regions the carvers walk at once.
    width: i32,
    /// The masks of the regions built so far. Absent when a region is one
    /// column, which no other column shares, and for a Beta source, which
    /// carves from each column's own terrain.
    regions: Option<RegionCache>,
}

/// How a source chunk's biome is found, and the carvers each answer runs.
enum SourceBiomes {
    /// The six climate roots at the source, against a climate table.
    Climate(ParameterList<Arc<[CarverConfig]>>),
    /// Temperature and humidity at the source through Beta's lookup grid, which
    /// only ever answers a land biome: one carver list per land biome, in
    /// discriminant order.
    Beta {
        lookup: Box<[[BetaLandBiome; 64]; 64]>,
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

/// The seed is part of the key so a table reused across routers cannot answer
/// with the other's climate.
type TileKey = (u64, i32, i32);

/// The tiles resolved most recently, at most `KEPT` of them.
///
/// The working set is the tiles under the columns in flight: a column touches
/// up to four, and columns arrive in distance order, so the same few answer
/// column after column while a player stays, and the ones behind a moving
/// player go stale. Least recently used is that set, and the capacity bounds
/// the memory rather than the explored area: `KEPT` tiles are 65k source
/// chunks, some forty view distances of 32, in 128 KB.
///
/// A miss costs one strided fill of the tile, which is worth 256 columns of
/// hits; the capacity only matters once more tiles than that are in flight at
/// once, and then a column degrades to filling its own tiles rather than to
/// anything worse.
type SourceTiles = Lru<TileKey, Tile>;

const KEPT: usize = 256;

/// The values asked for most recently.
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
        // chisle: a linear scan, which holds while the capacity is tens of
        // entries; past a few hundred it wants a map keyed by `K`.
        let (_, used, value) = self.entries.iter_mut().find(|(at, ..)| at == key)?;
        *used = self.clock;
        Some(value.clone())
    }

    /// The value held for `key`, or `value()`, which once `capacity` are held
    /// takes the place of the one asked for longest ago.
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

/// Columns per side of a region. A build walks (2 + 16)^2 = 324 sources for
/// four columns where each column alone walks 289, and one entry holds
/// 4 x 12,032 = 48,128 bytes of masks in the overworld.
pub const REGION_WIDTH: i32 = 2;

/// Regions kept, the least recently used leaving first: 256 x 48,128 =
/// 12,320,768 bytes in the overworld. A miss rebuilds the region in under a
/// millisecond and changes no block. 256 holds the regions of a view 32
/// columns in every direction twice over.
// chisle: sized for one view; several views at once evict each other's
// regions and pay rebuilds, not blocks.
pub const REGION_CAPACITY: usize = 256;

/// What a region's masks depend on besides the carvers of the table that holds
/// them: the seed the carver draws start from, the seed of the climate that
/// picks the carvers, the vertical range the carvers and the mask are resolved
/// against, and where the region is.
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

/// The regions built most recently, at most `capacity` of them, each built once
/// by whichever column asks first.
///
/// The list lock covers finding or inserting a cell and nothing else: the
/// build runs inside the cell, outside the lock, so a second asker of the same
/// region waits for the first and askers of other regions are not held up. A
/// region replaced while it is being built is built again by its next asker;
/// both are the same function of the key.
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

/// One column's mask: its slot of the region it was carved in.
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

    /// Whether any biome of the table runs a carver `kind` accepts.
    pub fn runs(&self, kind: impl Fn(&CarverConfig) -> bool) -> bool {
        self.biomes.lists().any(|carvers| carvers.iter().any(&kind))
    }
}

impl CarverBiomeTable {
    /// `lookup` answers what carvers a biome runs. It is called once per
    /// distinct biome in the preset, not once per entry.
    pub fn resolve(
        preset: Preset,
        lookup: impl Fn(&str) -> Arc<[CarverConfig]>,
    ) -> CarverBiomeTable {
        Self::with_biomes(SourceBiomes::Climate(Self::map_values(
            preset.parameter_list(),
            lookup,
        )))
        .with_region(REGION_WIDTH, REGION_CAPACITY)
    }

    /// A source that lists its biomes rather than naming a preset.
    pub fn from_entries(
        entries: Vec<(ParameterPoint, String)>,
        lookup: impl Fn(&str) -> Arc<[CarverConfig]>,
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
            Self::with_biomes(SourceBiomes::Climate(ParameterList::new(values)))
                .with_region(REGION_WIDTH, REGION_CAPACITY),
        )
    }

    /// A Beta source, whose biome is a function of temperature and humidity
    /// alone.
    pub fn beta(
        source: &BiomeSource,
        biomes: &Registry<keys::Biome>,
        lookup: impl Fn(&str) -> Arc<[CarverConfig]>,
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
            land: land_biomes
                .iter()
                .map(|id| {
                    let name = biomes.key(*id).unwrap_or_else(|| {
                        panic!("the biome registry holds no entry numbered {}", id.index())
                    });
                    lookup(name.as_str())
                })
                .collect(),
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

    /// The table carving `width` x `width` columns per region, keeping up to
    /// `capacity` regions.
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

    /// The most bytes the kept regions can hold for masks of `height`.
    pub fn region_bytes_bound(&self, height: HeightContext) -> usize {
        self.regions.as_ref().map_or(0, |regions| {
            regions.capacity * (self.width * self.width) as usize * column_mask_bytes(height)
        })
    }

    fn map_values(
        named: &ParameterList<&'static str>,
        lookup: impl Fn(&str) -> Arc<[CarverConfig]>,
    ) -> ParameterList<Arc<[CarverConfig]>> {
        let mut resolved: HashMap<&str, Arc<[CarverConfig]>> = HashMap::new();
        named.map_values(|biome| {
            resolved
                .entry(biome)
                .or_insert_with(|| lookup(biome))
                .clone()
        })
    }

    /// The carvers the source chunk at `(source_x, source_z)` runs.
    ///
    /// `held` keeps the tiles one column's region has touched, so the lock is
    /// taken at most four times per column.
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

    /// One tile's sources, evaluated as a single strided fill the first time
    /// any column needs one of them.
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
                let mut last = None;
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
                        let slot = table.find_slot_from(target, &mut last);
                        slots[(dx * TILE + dz) as usize] =
                            u16::try_from(slot).expect("a climate table fits in u16 slots");
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
        // Another worker may have resolved the tile meanwhile; theirs is kept,
        // both being computed from the same seed.
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
        table.find_value(target)
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

/// What the substance pass must leave alone.
pub struct ModernCarverBlockIds {
    /// Every state of every block in the `uncarvable` tag.
    uncarvable: Box<[VoxelId]>,
}

impl ModernCarverBlockIds {
    pub fn resolve(blocks: &BlockDefinitions, tags: Option<&Tags<Block>>) -> Self {
        let uncarvable = tags
            .and_then(|tags| Some((tags, tags.get(&UNCARVABLE)?)))
            .into_iter()
            .flat_map(|(tags, tag)| tags.members(tag))
            .filter_map(|id| blocks.blocks().get(id.index()))
            .flat_map(|entry| {
                (0..entry.state_count)
                    .map(move |offset| VoxelId::from(entry.base_state_id.0 + offset))
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

/// The reference protects the top seven blocks of the dimension from carving.
const PROTECTED_BLOCKS_ON_TOP: i32 = 7;

/// The mask one column's carvers mark, between the floor and the top seven
/// blocks the reference protects.
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

/// The bits one column's mask holds, one per block of the 16 x 16 column,
/// without the few words of header.
fn column_mask_bytes(height: HeightContext) -> usize {
    let (bottom, top) = mask_range(height);
    (16 * 16 * (top - bottom + 1).max(0) as usize).div_ceil(8)
}

/// Every carver of every biome that reaches this chunk, marked into one mask.
///
/// `water` answers Beta's abort. It is built the first time a Beta carver
/// starts in the column, off the blocks as filled: nothing writes to the column
/// until the whole mask is marked.
#[allow(clippy::too_many_arguments)]
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
    // Modern carvers have no water abort; the empty mask answers in one AND.
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

/// Every modern carver of every source within reach of any column of the
/// square at `origin`, walked once into the masks of all its columns.
#[allow(clippy::too_many_arguments)]
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

/// The mask of the carvers of every source that reaches this chunk, which is
/// its slot of the region the chunk falls in: marked before any block of the
/// column is decided.
#[allow(clippy::too_many_arguments)]
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

/// What the carvers make of the solid terrain of one column: the mask they
/// marked, and the fluid field the fill used, asked again with zero density,
/// so a positive barrier alone keeps a carved tunnel through a lake shore
/// walled.
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

    /// The air or fluid a solid block becomes in place of `block`, the one the
    /// material rules chose for it. `None` leaves `block` standing: the
    /// position is not carved, `block` is uncarvable, or the barrier keeps it
    /// solid.
    pub fn substance(&mut self, x: i32, y: i32, z: i32, block: Option<VoxelId>) -> Option<VoxelId> {
        if !self.is_carved(x, y, z) || block.is_some_and(|block| self.ids.is_uncarvable(block)) {
            return None;
        }
        let mut barrier = point_barrier(self.router, &mut self.ws);
        self.fluid
            .substance_settled(self.block_x + x, y, self.block_z + z, 0.0, &mut barrier)
    }
}

/// Every carver of every biome that reaches this chunk, applied to a column no
/// material rule decides.
#[cfg(test)]
#[allow(clippy::too_many_arguments)]
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

/// Each carved solid block of a column no material rule decides, asked as it
/// stands what it becomes. Air and fluid the fill placed are never carved.
pub fn carve_unsurfaced(column: &ColumnBlocks, carving: &mut TerrainCarving<'_, '_>) {
    let router = carving.router;
    let open = [
        VoxelId::default(),
        router.default_fluid_state,
        router.water_state,
        router.lava_state,
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

/// A climate point spanning every parameter, for a source with one biome.
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

pub fn resolve_carver_biomes(
    preset: Option<Preset>,
    explicit: Option<Vec<(ParameterPoint, String)>>,
    biomes: &Registry<keys::Biome>,
    carvers: &Entries<keys::Biome, Arc<[CarverConfig]>>,
) -> Option<CarverBiomeTable> {
    let lookup = biome_carvers(biomes, carvers);
    match preset {
        Some(preset) => Some(CarverBiomeTable::resolve(preset, lookup)),
        None => explicit.and_then(|entries| CarverBiomeTable::from_entries(entries, lookup)),
    }
}

/// [`resolve_carver_biomes`] for a Beta source.
pub fn resolve_beta_carver_biomes(
    source: &BiomeSource,
    biomes: &Registry<keys::Biome>,
    carvers: &Entries<keys::Biome, Arc<[CarverConfig]>>,
) -> Option<CarverBiomeTable> {
    CarverBiomeTable::beta(source, biomes, biome_carvers(biomes, carvers))
}

fn biome_carvers<'a>(
    biomes: &'a Registry<keys::Biome>,
    carvers: &'a Entries<keys::Biome, Arc<[CarverConfig]>>,
) -> impl Fn(&str) -> Arc<[CarverConfig]> + 'a {
    move |name: &str| match biomes.get(name) {
        Some(id) => carvers[id].clone(),
        None => {
            // The table still resolves and reports success, so a biome absent
            // from the registry carves nothing at all with no other symptom,
            // which for a single-biome source is the whole world.
            tracing::error!(
                biome = name,
                "biome has no loaded definition; it carves nothing"
            );
            Arc::from([])
        }
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

        // Touching the oldest entry makes the one after it the stalest.
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
