use serde::{Deserialize, Serialize};

use super::block_predicate::BlockPredicate;
use super::tree::{UnitFloat, non_empty};
use mcrs_minecraft_core::codec::{Bounded, PositiveInt, default_true, is_default};
use mcrs_minecraft_core::value_provider::{BoundedIntProvider, HeightProvider};

/// `Codec.INT.optionalFieldOf(name, DEFAULT)`.
pub type IntOr<const DEFAULT: i32> = Bounded<{ i32::MIN }, { i32::MAX }, DEFAULT>;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HeightmapName {
    #[serde(rename = "WORLD_SURFACE_WG")]
    WorldSurfaceWg,
    #[serde(rename = "WORLD_SURFACE")]
    WorldSurface,
    #[serde(rename = "OCEAN_FLOOR_WG")]
    OceanFloorWg,
    #[serde(rename = "OCEAN_FLOOR")]
    OceanFloor,
    #[serde(rename = "MOTION_BLOCKING")]
    MotionBlocking,
    #[serde(rename = "MOTION_BLOCKING_NO_LEAVES")]
    MotionBlockingNoLeaves,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum VerticalDirection {
    Up,
    Down,
}

impl From<VerticalDirection> for mcrs_minecraft_core::Direction {
    fn from(direction: VerticalDirection) -> Self {
        match direction {
            VerticalDirection::Up => mcrs_minecraft_core::Direction::Up,
            VerticalDirection::Down => mcrs_minecraft_core::Direction::Down,
        }
    }
}

/// One placement modifier. `P` is the predicate type: the asset's
/// `BlockPredicate` as loaded, the compiled `Predicate` once every set it names
/// is a mask.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type")]
#[serde(deny_unknown_fields)]
#[serde(bound(deserialize = "P: Deserialize<'de>", serialize = "P: Serialize"))]
pub enum PlacementModifier<P = BlockPredicate> {
    #[serde(rename = "minecraft:block_predicate_filter")]
    BlockPredicateFilter { predicate: P },
    #[serde(rename = "minecraft:rarity_filter")]
    RarityFilter { chance: PositiveInt },
    #[serde(rename = "minecraft:random_chance")]
    RandomChance { chance: UnitFloat },
    #[serde(rename = "minecraft:surface_relative_threshold_filter")]
    SurfaceRelativeThresholdFilter {
        heightmap: HeightmapName,
        #[serde(default, skip_serializing_if = "is_default")]
        min_inclusive: IntOr<{ i32::MIN }>,
        #[serde(default, skip_serializing_if = "is_default")]
        max_inclusive: IntOr<{ i32::MAX }>,
    },
    #[serde(rename = "minecraft:surface_water_depth_filter")]
    SurfaceWaterDepthFilter { max_water_depth: i32 },
    #[serde(rename = "minecraft:biome")]
    Biome {},
    #[serde(rename = "minecraft:count")]
    Count { count: BoundedIntProvider<0, 4096> },
    #[serde(rename = "minecraft:noise_based_count")]
    NoiseBasedCount {
        noise_to_count_ratio: i32,
        noise_factor: f64,
        #[serde(default, skip_serializing_if = "is_default")]
        noise_offset: f64,
    },
    #[serde(rename = "minecraft:noise_threshold_count")]
    NoiseThresholdCount {
        noise_level: f64,
        below_noise: i32,
        above_noise: i32,
    },
    #[serde(rename = "minecraft:count_on_every_layer")]
    CountOnEveryLayer { count: BoundedIntProvider<0, 256> },
    #[serde(rename = "minecraft:cuboid")]
    Cuboid {
        xz_size: BoundedIntProvider<1, 16>,
        y_size: BoundedIntProvider<1, 16>,
        #[serde(default = "default_true", skip_serializing_if = "Clone::clone")]
        include_edges: bool,
        #[serde(default = "default_true", skip_serializing_if = "Clone::clone")]
        include_interior: bool,
    },
    #[serde(rename = "minecraft:environment_scan")]
    EnvironmentScan {
        direction_of_search: VerticalDirection,
        target_condition: P,
        /// Absent is vanilla's `alwaysTrue` default, kept absent so the entry
        /// is written back the way it was read.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        allowed_search_condition: Option<P>,
        max_steps: Bounded<1, 32>,
    },
    #[serde(rename = "minecraft:heightmap")]
    Heightmap { heightmap: HeightmapName },
    #[serde(rename = "minecraft:height_range")]
    HeightRange { height: HeightProvider },
    #[serde(rename = "minecraft:in_square")]
    InSquare {},
    #[serde(rename = "minecraft:offset")]
    Offset {
        x: BoundedIntProvider<-16, 16>,
        y: BoundedIntProvider<-16, 16>,
        z: BoundedIntProvider<-16, 16>,
    },
    #[serde(rename = "minecraft:randomly_selected")]
    RandomlySelected {
        #[serde(deserialize_with = "non_empty")]
        placements: Vec<PlacementModifier<P>>,
    },
    #[serde(rename = "minecraft:fixed_placement")]
    FixedPlacement { positions: Vec<[i32; 3]> },
}

impl<P> PlacementModifier<P> {
    /// The same chain over another predicate type.
    pub fn try_map<Q, E>(
        &self,
        f: &mut impl FnMut(&P) -> Result<Q, E>,
    ) -> Result<PlacementModifier<Q>, E> {
        use PlacementModifier::*;
        Ok(match self {
            BlockPredicateFilter { predicate } => BlockPredicateFilter {
                predicate: f(predicate)?,
            },
            RarityFilter { chance } => RarityFilter { chance: *chance },
            RandomChance { chance } => RandomChance { chance: *chance },
            SurfaceRelativeThresholdFilter {
                heightmap,
                min_inclusive,
                max_inclusive,
            } => SurfaceRelativeThresholdFilter {
                heightmap: *heightmap,
                min_inclusive: *min_inclusive,
                max_inclusive: *max_inclusive,
            },
            SurfaceWaterDepthFilter { max_water_depth } => SurfaceWaterDepthFilter {
                max_water_depth: *max_water_depth,
            },
            Biome {} => Biome {},
            Count { count } => Count {
                count: count.clone(),
            },
            NoiseBasedCount {
                noise_to_count_ratio,
                noise_factor,
                noise_offset,
            } => NoiseBasedCount {
                noise_to_count_ratio: *noise_to_count_ratio,
                noise_factor: *noise_factor,
                noise_offset: *noise_offset,
            },
            NoiseThresholdCount {
                noise_level,
                below_noise,
                above_noise,
            } => NoiseThresholdCount {
                noise_level: *noise_level,
                below_noise: *below_noise,
                above_noise: *above_noise,
            },
            CountOnEveryLayer { count } => CountOnEveryLayer {
                count: count.clone(),
            },
            Cuboid {
                xz_size,
                y_size,
                include_edges,
                include_interior,
            } => Cuboid {
                xz_size: xz_size.clone(),
                y_size: y_size.clone(),
                include_edges: *include_edges,
                include_interior: *include_interior,
            },
            EnvironmentScan {
                direction_of_search,
                target_condition,
                allowed_search_condition,
                max_steps,
            } => EnvironmentScan {
                direction_of_search: *direction_of_search,
                target_condition: f(target_condition)?,
                allowed_search_condition: allowed_search_condition
                    .as_ref()
                    .map(&mut *f)
                    .transpose()?,
                max_steps: *max_steps,
            },
            Heightmap { heightmap } => Heightmap {
                heightmap: *heightmap,
            },
            HeightRange { height } => HeightRange { height: *height },
            InSquare {} => InSquare {},
            Offset { x, y, z } => Offset {
                x: x.clone(),
                y: y.clone(),
                z: z.clone(),
            },
            RandomlySelected { placements } => RandomlySelected {
                placements: placements
                    .iter()
                    .map(|m| m.try_map(f))
                    .collect::<Result<_, E>>()?,
            },
            FixedPlacement { positions } => FixedPlacement {
                positions: positions.clone(),
            },
        })
    }
}
