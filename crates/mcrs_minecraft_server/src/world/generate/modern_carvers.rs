use std::collections::HashMap;
use std::sync::Arc;

use bevy_math::IVec3;
use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::tag::key::TagKey;
use mcrs_minecraft_core::tag::registry::DynTagRegistry;
use mcrs_minecraft_decoration::carver::canyon::carve_canyon;
use mcrs_minecraft_decoration::carver::mask::CarvingMask;
use mcrs_minecraft_decoration::carver::modern::{SOURCE_RADIUS, carve_caves, is_start_chunk};
use mcrs_minecraft_decoration::carver::water::WaterMask;
use mcrs_minecraft_random::legacy::LegacyRandom;
use mcrs_minecraft_world::biome::climate::{ParameterList, ParameterPoint, TargetPoint};
use mcrs_minecraft_world::biome::overworld_preset::{
    nether_parameter_list, overworld_parameter_list,
};
use mcrs_minecraft_world::block::Block as VanillaBlock;
use mcrs_minecraft_world::block::definition::BlockDefinitions;
use mcrs_minecraft_worldgen::carver::CarverConfig;
use mcrs_minecraft_worldgen::program::Workspace;
use mcrs_minecraft_worldgen::router::NoiseRouter;
use mcrs_minecraft_worldgen::value_provider::HeightContext;
use mcrs_minecraft_worldgen::volume::Volume;
use mcrs_voxel_storage::VoxelId;

use crate::world::generate::ColumnBlocks;

/// `WorldgenRandom.setLargeFeatureSeed`.
///
/// Beta's sibling adds odd-forced products where this one exclusive-ors plain
/// ones; the two are the same idea and not the same number.
pub fn large_feature_seed(world_seed: i64, chunk_x: i32, chunk_z: i32) -> i64 {
    let mut rng = LegacyRandom::new(world_seed as u64);
    let x_scale = rng.next_java_long();
    let z_scale = rng.next_java_long();
    (chunk_x as i64).wrapping_mul(x_scale) ^ (chunk_z as i64).wrapping_mul(z_scale) ^ world_seed
}

/// The climate at every source chunk that can reach one target chunk.
///
/// The density program fills a whole strided volume in one pass, so asking for
/// the 17x17 grid at once costs a fraction of 289 single-point evaluations.
/// Indexed by `(source_x - chunk_x + SOURCE_RADIUS, source_z - chunk_z +
/// SOURCE_RADIUS)`, row-major in x.
pub fn climate_targets_for_sources(
    router: &NoiseRouter,
    ws: &mut Workspace,
    chunk_x: i32,
    chunk_z: i32,
    out: &mut Vec<TargetPoint>,
) {
    let side = SOURCE_RADIUS * 2 + 1;
    let volume = Volume::new(
        IVec3::new(side, 1, side),
        IVec3::new(
            (chunk_x - SOURCE_RADIUS) * 16,
            0,
            (chunk_z - SOURCE_RADIUS) * 16,
        ),
        IVec3::new(16, 1, 16),
    );
    let roots = [
        router.temperature(),
        router.vegetation(),
        router.continents(),
        router.erosion(),
        router.depth(),
        router.ridges(),
    ];
    let points = volume.len();
    let mut values = vec![0.0f32; roots.len() * points];
    router.fill_roots(ws, &volume, &roots, &mut values);

    out.clear();
    for dx in 0..side {
        for dz in 0..side {
            let at = volume.index_unchecked(dx, 0, dz);
            out.push(TargetPoint::new(
                values[at],
                values[points + at],
                values[2 * points + at],
                values[3 * points + at],
                values[4 * points + at],
                values[5 * points + at],
            ));
        }
    }
}

/// The six climate roots at one quart position, which is where a carver source
/// takes its biome from.
pub fn climate_target_at(
    router: &NoiseRouter,
    ws: &mut Workspace,
    quart_x: i32,
    quart_y: i32,
    quart_z: i32,
) -> TargetPoint {
    let volume = Volume::new(
        IVec3::ONE,
        IVec3::new(quart_x * 4, quart_y * 4, quart_z * 4),
        IVec3::ONE,
    );
    let roots = [
        router.temperature(),
        router.vegetation(),
        router.continents(),
        router.erosion(),
        router.depth(),
        router.ridges(),
    ];
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
    table: ParameterList<Arc<[CarverConfig]>>,
}

impl CarverBiomeTable {
    #[cfg(test)]
    pub fn entry_count(&self) -> usize {
        self.table.len()
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
            table: Self::map_values(named, lookup),
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
            table: ParameterList::new(values),
        })
    }

    fn map_values(
        named: ParameterList<&'static str>,
        lookup: impl Fn(&str) -> Arc<[CarverConfig]>,
    ) -> ParameterList<Arc<[CarverConfig]>> {
        let mut resolved: HashMap<&str, Arc<[CarverConfig]>> = HashMap::new();
        let values = named
            .values()
            .iter()
            .map(|(point, biome)| {
                let carvers = resolved
                    .entry(biome)
                    .or_insert_with(|| lookup(biome))
                    .clone();
                (*point, carvers)
            })
            .collect();
        ParameterList::new(values)
    }

    fn carvers_at(&self, target: TargetPoint) -> &[CarverConfig] {
        self.table.find_value(target)
    }

    #[cfg(test)]
    pub fn carvers_at_for_test(&self, target: TargetPoint) -> &[CarverConfig] {
        self.carvers_at(target)
    }
}

/// What the substance pass writes, and what it must leave alone.
pub struct ModernCarverBlockIds {
    pub air: VoxelId,
    pub fluid: VoxelId,
    pub sea_level: i32,
    /// Every state of every block in the `uncarvable` tag.
    uncarvable: Box<[VoxelId]>,
}

impl ModernCarverBlockIds {
    pub fn resolve(
        blocks: &BlockDefinitions,
        router: &NoiseRouter,
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
        ModernCarverBlockIds {
            air: blocks.default_state("minecraft:air").into(),
            fluid: router.default_fluid_state(),
            sea_level: router.sea_level(),
            uncarvable,
        }
    }

    #[cfg(test)]
    pub fn for_test(
        air: VoxelId,
        fluid: VoxelId,
        sea_level: i32,
        uncarvable: Vec<VoxelId>,
    ) -> Self {
        ModernCarverBlockIds {
            air,
            fluid,
            sea_level,
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

/// Every carver of every biome that reaches this chunk, then one pass to fill
/// what they freed.
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
) {
    let mut mask = CarvingMask::new(
        16,
        height.min_y + 1,
        height.min_y + height.depth - 1 - PROTECTED_BLOCKS_ON_TOP,
    );
    // Modern carvers have no water abort; the empty mask answers in one AND.
    let water = WaterMask::default();

    let mut targets = Vec::new();
    climate_targets_for_sources(router, ws, chunk_x, chunk_z, &mut targets);
    let side = SOURCE_RADIUS * 2 + 1;

    for source_x in (chunk_x - SOURCE_RADIUS)..=(chunk_x + SOURCE_RADIUS) {
        for source_z in (chunk_z - SOURCE_RADIUS)..=(chunk_z + SOURCE_RADIUS) {
            let slot =
                (source_x - chunk_x + SOURCE_RADIUS) * side + (source_z - chunk_z + SOURCE_RADIUS);
            let target = targets[slot as usize];
            for (index, config) in biomes.carvers_at(target).iter().enumerate() {
                let seed = world_seed.wrapping_add(index as i64);
                let mut rng =
                    LegacyRandom::new(large_feature_seed(seed, source_x, source_z) as u64);
                if !is_start_chunk(config, &mut rng) {
                    continue;
                }
                match config {
                    CarverConfig::Cave { .. } => carve_caves(
                        config, height, chunk_x, chunk_z, source_x, source_z, &water, &mut mask,
                        &mut rng,
                    ),
                    CarverConfig::Canyon { .. } => carve_canyon(
                        config, height, chunk_x, chunk_z, source_x, source_z, &water, &mut mask,
                        &mut rng,
                    ),
                }
            }
        }
    }

    apply_carver_substance(column, &mask, ids);
}

/// Fill the freed space, with the same rule the terrain fill uses: the
/// dimension's fluid below its sea level, air above it.
///
/// This is `Aquifer.createDisabled` over a global fluid picker. Neither this
/// pass nor the terrain fill implements the noise aquifer or the overworld's
/// separate lava level, and they agree because they use the one rule.
fn apply_carver_substance(column: &ColumnBlocks, mask: &CarvingMask, ids: &ModernCarverBlockIds) {
    mask.visit(|x, z, bottom_y, top_y| {
        for y in (bottom_y..=top_y).rev() {
            let Some(state) = column.get(x, y, z) else {
                continue;
            };
            if ids.is_uncarvable(state) {
                continue;
            }
            let substance = if y < ids.sea_level {
                ids.fluid
            } else {
                ids.air
            };
            column.set(x, y, z, substance);
        }
    });
}

/// The dimension's resolved carver table, built once in the host and cloned
/// into the sub-app the way the other immutable registries are.
#[derive(bevy_ecs::prelude::Resource, Clone)]
pub struct ModernCarverBiomes(pub Arc<CarverBiomeTable>);

pub struct ModernCarverPlugin;

impl bevy_app::Plugin for ModernCarverPlugin {
    fn build(&self, app: &mut bevy_app::App) {
        app.add_systems(
            bevy_state::prelude::OnEnter(mcrs_minecraft_core::AppState::WorldgenFreeze),
            bevy_ecs::prelude::IntoScheduleConfigs::before(
                build_modern_carver_biomes,
                mcrs_minecraft_world::transition_to_playing,
            ),
        );
    }
}

/// Resolve the active biome source's climate table into carver lists.
///
/// Both halves are loaded assets: which carvers a biome runs comes from the
/// biome JSON, and what each carver is comes from the carver JSON, so a
/// datapack that retunes either is picked up here.
fn build_modern_carver_biomes(
    mut commands: bevy_ecs::prelude::Commands,
    source: Option<
        bevy_ecs::prelude::Res<mcrs_minecraft_world::worldgen::beta_biome::ActiveBiomeSource>,
    >,
    biomes: bevy_ecs::prelude::Res<bevy_asset::Assets<mcrs_minecraft_world::biome::Biome>>,
    carvers: bevy_ecs::prelude::Res<
        bevy_asset::Assets<mcrs_minecraft_worldgen::bevy::CarverConfigAsset>,
    >,
    asset_server: bevy_ecs::prelude::Res<bevy_asset::AssetServer>,
) {
    use mcrs_minecraft_core::registry::snapshot::rl_from_asset_path;
    use mcrs_minecraft_world::biome::source::BiomeSource;

    let Some(source) = source else { return };
    let BiomeSource::MultiNoise(multi) = source.0.as_ref() else {
        return;
    };

    let mut config_by_location: HashMap<String, CarverConfig> = HashMap::new();
    for (asset_id, asset) in carvers.iter() {
        let Some(path) = asset_server.get_path(asset_id) else {
            continue;
        };
        let Some(location) = rl_from_asset_path(path.path()) else {
            continue;
        };
        config_by_location.insert(location.as_str().to_owned(), asset.config.clone());
    }

    let mut carvers_by_biome: HashMap<String, Vec<String>> = HashMap::new();
    for (asset_id, biome) in biomes.iter() {
        let Some(path) = asset_server.get_path(asset_id) else {
            continue;
        };
        let Some(location) = rl_from_asset_path(path.path()) else {
            continue;
        };
        carvers_by_biome.insert(
            location.as_str().to_owned(),
            biome
                .carvers
                .iter()
                .map(|carver| carver.as_str().to_owned())
                .collect(),
        );
    }

    let explicit = multi.biomes.as_ref().map(|entries| {
        entries
            .iter()
            .filter_map(|entry| {
                let path = asset_server.get_path(entry.biome.id())?;
                let location = rl_from_asset_path(path.path())?;
                Some((
                    ParameterPoint::from(&entry.parameters),
                    location.as_str().to_owned(),
                ))
            })
            .collect()
    });

    match resolve_carver_biomes(
        multi.preset.as_ref().map(|preset| preset.as_str()),
        explicit,
        &carvers_by_biome,
        &config_by_location,
    ) {
        Some(table) => {
            tracing::info!(entries = table.table.len(), "resolved the carver table");
            commands.insert_resource(ModernCarverBiomes(Arc::new(table)));
        }
        None => tracing::info!("no carver table for this biome source"),
    }
}

/// The resolution itself, with the asset lookups already reduced to two maps:
/// which carvers each biome runs, and what each carver is.
pub fn resolve_carver_biomes(
    preset: Option<&str>,
    explicit: Option<Vec<(ParameterPoint, String)>>,
    carvers_by_biome: &HashMap<String, Vec<String>>,
    config_by_location: &HashMap<String, CarverConfig>,
) -> Option<CarverBiomeTable> {
    let lookup = |biome: &str| -> Arc<[CarverConfig]> {
        carvers_by_biome
            .get(biome)
            .map(|names| {
                names
                    .iter()
                    .filter_map(|name| config_by_location.get(name).cloned())
                    .collect()
            })
            .unwrap_or_else(|| Arc::from(Vec::new()))
    };
    match preset {
        Some(preset) => CarverBiomeTable::resolve(preset, lookup),
        None => explicit.and_then(|entries| CarverBiomeTable::from_entries(entries, lookup)),
    }
}
