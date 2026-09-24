use crate::{ColumnBlocks, beta_chunk_seed};
use bevy_math::IVec3;
use mcrs_minecraft_assets::tag::registry::DynTagRegistry;
use mcrs_minecraft_biome::climate::{ParameterList, ParameterPoint, TargetPoint};
use mcrs_minecraft_biome::overworld_preset::{nether_parameter_list, overworld_parameter_list};
use mcrs_minecraft_biome::source::{BetaLandBiome, BiomeSource, beta_biome_from_climate};
use mcrs_minecraft_block::Block as VanillaBlock;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::tag_key::TagKey;
use mcrs_minecraft_core::value_provider::HeightContext;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_worldgen_carver::beta::carve_beta_caves;
use mcrs_minecraft_worldgen_carver::canyon::carve_canyon;
use mcrs_minecraft_worldgen_carver::config::CarverConfig;
use mcrs_minecraft_worldgen_carver::mask::CarvingMask;
use mcrs_minecraft_worldgen_carver::modern::{SOURCE_RADIUS, carve_caves, is_start_chunk};
use mcrs_minecraft_worldgen_carver::water::WaterMask;
use mcrs_minecraft_worldgen_density::aquifer::{FluidField, point_barrier};
use mcrs_minecraft_worldgen_density::program::Workspace;
use mcrs_minecraft_worldgen_density::router::{
    CONTINENTS, DEPTH, EROSION, NoiseRouter, RIDGES, TEMPERATURE, VEGETATION,
};
use mcrs_minecraft_worldgen_noise::sample_grid::SampleGrid;
use std::collections::HashMap;
use std::sync::{Arc, Mutex};

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
    let roots = [TEMPERATURE, VEGETATION, CONTINENTS, EROSION, DEPTH, RIDGES];
    let mut values = [0.0f32; 6];
    router.fill_roots(ws, &volume, &roots, &mut values);
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
#[derive(Default)]
struct SourceTiles {
    entries: Vec<(TileKey, u64, Tile)>,
    clock: u64,
}

const KEPT: usize = 256;

impl SourceTiles {
    fn get(&mut self, key: TileKey) -> Option<Tile> {
        self.clock += 1;
        let (_, used, tile) = self.entries.iter_mut().find(|(at, ..)| *at == key)?;
        *used = self.clock;
        Some(tile.clone())
    }

    /// Keeps `tile` unless another worker inserted the key meanwhile, in which
    /// case theirs is returned: both were computed from the same seed.
    fn insert(&mut self, key: TileKey, tile: Tile) -> Tile {
        if let Some(held) = self.get(key) {
            return held;
        }
        let entry = (key, self.clock, tile.clone());
        if self.entries.len() < KEPT {
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
        tile
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
        preset: &str,
        lookup: impl Fn(&str) -> Arc<[CarverConfig]>,
    ) -> Option<CarverBiomeTable> {
        let named = match preset {
            "minecraft:overworld" => overworld_parameter_list(),
            "minecraft:nether" => nether_parameter_list(),
            _ => return None,
        };
        Some(CarverBiomeTable {
            biomes: SourceBiomes::Climate(Self::map_values(named, lookup)),
            tiles: Mutex::default(),
        })
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
        Some(CarverBiomeTable {
            biomes: SourceBiomes::Climate(ParameterList::new(values)),
            tiles: Mutex::default(),
        })
    }

    /// A Beta source, whose biome is a function of temperature and humidity
    /// alone.
    pub fn beta(
        source: &BiomeSource,
        lookup: impl Fn(&str) -> Arc<[CarverConfig]>,
    ) -> Option<CarverBiomeTable> {
        let BiomeSource::Beta {
            land_biome_ids,
            lookup: grid,
            ..
        } = source
        else {
            return None;
        };
        Some(CarverBiomeTable {
            biomes: SourceBiomes::Beta {
                lookup: grid.clone(),
                land: land_biome_ids
                    .iter()
                    .map(|id| lookup(id.as_str()))
                    .collect(),
            },
            tiles: Mutex::default(),
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
        if let Some(tile) = self.tiles.lock().expect("carver tiles").get(key) {
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
                let roots = [TEMPERATURE, VEGETATION, CONTINENTS, EROSION, DEPTH, RIDGES];
                let mut values = vec![0.0f32; roots.len() * points];
                router.fill_roots(ws, &volume, &roots, &mut values);
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
        self.tiles
            .lock()
            .expect("carver tiles")
            .insert(key, Arc::new(slots))
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
    pub fn resolve(
        blocks: &BlockDefinitions,
        block_tags: Option<&DynTagRegistry<VanillaBlock>>,
    ) -> Self {
        let key: TagKey<VanillaBlock, Arc<str>> =
            TagKey::from_location(ResourceLocation::new_static("minecraft:uncarvable").to_arc());
        let uncarvable = block_tags
            .and_then(|tags| tags.get(&key))
            .into_iter()
            .flat_map(|members| members.iter())
            .filter_map(|index| blocks.blocks().get(index as usize))
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
    CarvingMask::new(
        16,
        height.min_y + 1,
        height.min_y + height.depth - 1 - PROTECTED_BLOCKS_ON_TOP,
    )
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
                let seed = match config {
                    CarverConfig::Cave { .. } | CarverConfig::Canyon { .. } => {
                        LegacyRandom::large_feature_seed(
                            world_seed.wrapping_add(index as i64),
                            source_x,
                            source_z,
                        )
                    }
                    CarverConfig::BetaCave => beta_chunk_seed(world_seed, source_x, source_z),
                };
                let mut rng = LegacyRandom::new(seed as u64);
                if !is_start_chunk(config, &mut rng) {
                    continue;
                }
                match config {
                    CarverConfig::Cave { .. } => carve_caves(
                        config, height, chunk_x, chunk_z, source_x, source_z, &no_water, &mut mask,
                        &mut rng,
                    ),
                    CarverConfig::Canyon { .. } => carve_canyon(
                        config, height, chunk_x, chunk_z, source_x, source_z, &no_water, &mut mask,
                        &mut rng,
                    ),
                    CarverConfig::BetaCave => carve_beta_caves(
                        chunk_x,
                        chunk_z,
                        source_x,
                        source_z,
                        beta_water.get_or_insert_with(&water),
                        &mut mask,
                        &mut rng,
                    ),
                }
            }
        }
    }
    mask
}

/// Every modern carver of every biome that reaches this chunk, marked into one
/// mask before any block of the column is decided.
#[allow(clippy::too_many_arguments)]
pub fn modern_carving_mask(
    chunk_x: i32,
    chunk_z: i32,
    world_seed: i64,
    router: &NoiseRouter,
    ws: &mut Workspace,
    biomes: &CarverBiomeTable,
    height: HeightContext,
) -> CarvingMask {
    carve_sources(
        || unreachable!("a Beta carver outside a Beta dimension is refused with the tables"),
        chunk_x,
        chunk_z,
        world_seed,
        router,
        ws,
        biomes,
        height,
    )
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

/// The resolution itself, with the asset lookups already reduced to two maps:
/// which carvers each biome runs, and what each carver is.
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
    preset: Option<&str>,
    explicit: Option<Vec<(ParameterPoint, String)>>,
    carvers_by_biome: &HashMap<String, Vec<String>>,
    config_by_location: &HashMap<String, CarverConfig>,
) -> Option<CarverBiomeTable> {
    let lookup = biome_carvers(carvers_by_biome, config_by_location);
    match preset {
        Some(preset) => CarverBiomeTable::resolve(preset, lookup),
        None => explicit.and_then(|entries| CarverBiomeTable::from_entries(entries, lookup)),
    }
}

/// [`resolve_carver_biomes`] for a Beta source.
pub fn resolve_beta_carver_biomes(
    source: &BiomeSource,
    carvers_by_biome: &HashMap<String, Vec<String>>,
    config_by_location: &HashMap<String, CarverConfig>,
) -> Option<CarverBiomeTable> {
    CarverBiomeTable::beta(source, biome_carvers(carvers_by_biome, config_by_location))
}

/// The carvers a biome runs, by the biome's name.
fn biome_carvers<'a>(
    carvers_by_biome: &'a HashMap<String, Vec<String>>,
    config_by_location: &'a HashMap<String, CarverConfig>,
) -> impl Fn(&str) -> Arc<[CarverConfig]> + 'a {
    move |biome: &str| -> Arc<[CarverConfig]> {
        if !carvers_by_biome.contains_key(biome) {
            // The table still resolves and reports success, so a biome absent
            // from the loaded assets carves nothing at all with no other
            // symptom — which for a single-biome source is the whole world.
            tracing::error!(biome, "biome has no loaded definition; it carves nothing");
        }
        carvers_by_biome
            .get(biome)
            .map(|names| {
                names
                    .iter()
                    .filter_map(|name| match config_by_location.get(name) {
                        Some(config) => Some(config.clone()),
                        None => {
                            // A biome naming a carver nobody loaded carves nothing
                            // at all, and does it without a symptom to notice.
                            tracing::error!(biome, carver = name, "carver not loaded");
                            None
                        }
                    })
                    .collect()
            })
            .unwrap_or_else(|| Arc::from(Vec::new()))
    }
}

/// The folder a carver asset loads from, under its namespace.
pub const CARVER_REGISTRY: &str = "worldgen/carver";

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
            tiles.insert((0, index as i32, 0), tile(index as u16));
        }
        assert_eq!(tiles.entries.len(), KEPT);

        // Touching the oldest entry makes the one after it the stalest.
        assert!(tiles.get((0, 0, 0)).is_some());
        tiles.insert((0, -1, 0), tile(u16::MAX));

        assert_eq!(tiles.entries.len(), KEPT);
        assert!(tiles.get((0, 0, 0)).is_some(), "the touched tile stays");
        assert!(
            tiles.get((0, 1, 0)).is_none(),
            "the stalest tile is evicted"
        );
        assert_eq!(tiles.get((0, -1, 0)).map(|t| t[0]), Some(u16::MAX));
    }

    #[test]
    fn a_racing_insert_keeps_the_first_tile() {
        let mut tiles = SourceTiles::default();
        let first = tiles.insert((7, 1, 2), tile(1));
        let second = tiles.insert((7, 1, 2), tile(2));
        assert!(Arc::ptr_eq(&first, &second));
        assert_eq!(tiles.entries.len(), 1);
    }
}
