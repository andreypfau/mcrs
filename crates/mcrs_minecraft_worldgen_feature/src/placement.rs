use mcrs_minecraft_block_predicate::predicate::HeightmapName;
use serde::{Deserialize, Serialize};

use mcrs_minecraft_block_predicate::predicate::BlockPredicate;
use mcrs_minecraft_block_predicate::provider::{UnitFloat, non_empty};
use mcrs_minecraft_core::codec::{Bounded, PositiveInt, default_true, is_default};
use mcrs_minecraft_value_provider::{BoundedIntProvider, HeightProvider};

/// `Codec.INT.optionalFieldOf(name, DEFAULT)`.
pub type IntOr<const DEFAULT: i32> = Bounded<{ i32::MIN }, { i32::MAX }, DEFAULT>;

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
#[serde(remote = "Self")]
#[serde(deny_unknown_fields)]
#[serde(bound(deserialize = "P: Deserialize<'de>", serialize = "P: Serialize"))]
pub enum PlacementModifier<P = BlockPredicate> {
    BlockPredicateFilter {
        predicate: P,
    },
    RarityFilter {
        chance: PositiveInt,
    },
    RandomChance {
        chance: UnitFloat,
    },
    SurfaceRelativeThresholdFilter {
        heightmap: HeightmapName,
        #[serde(default, skip_serializing_if = "is_default")]
        min_inclusive: IntOr<{ i32::MIN }>,
        #[serde(default, skip_serializing_if = "is_default")]
        max_inclusive: IntOr<{ i32::MAX }>,
    },
    SurfaceWaterDepthFilter {
        max_water_depth: i32,
    },
    Biome {},
    Count {
        count: BoundedIntProvider<0, 4096>,
    },
    NoiseBasedCount {
        noise_to_count_ratio: i32,
        noise_factor: f64,
        #[serde(default, skip_serializing_if = "is_default")]
        noise_offset: f64,
    },
    NoiseThresholdCount {
        noise_level: f64,
        below_noise: i32,
        above_noise: i32,
    },
    CountOnEveryLayer {
        count: BoundedIntProvider<0, 256>,
    },
    Cuboid {
        xz_size: BoundedIntProvider<1, 16>,
        y_size: BoundedIntProvider<1, 16>,
        #[serde(default = "default_true", skip_serializing_if = "Clone::clone")]
        include_edges: bool,
        #[serde(default = "default_true", skip_serializing_if = "Clone::clone")]
        include_interior: bool,
    },
    EnvironmentScan {
        direction_of_search: VerticalDirection,
        target_condition: P,
        /// Absent is vanilla's `alwaysTrue` default, kept absent so the entry
        /// is written back the way it was read.
        #[serde(default, skip_serializing_if = "Option::is_none")]
        allowed_search_condition: Option<P>,
        max_steps: Bounded<1, 32>,
    },
    Heightmap {
        heightmap: HeightmapName,
    },
    HeightRange {
        height: HeightProvider,
    },
    InSquare {},
    Offset {
        x: BoundedIntProvider<-16, 16>,
        y: BoundedIntProvider<-16, 16>,
        z: BoundedIntProvider<-16, 16>,
    },
    RandomlySelected {
        #[serde(deserialize_with = "non_empty")]
        placements: Vec<PlacementModifier<P>>,
    },
    FixedPlacement {
        #[serde(deserialize_with = "non_empty")]
        positions: Vec<[i32; 3]>,
    },
}

mcrs_minecraft_registry::dispatch! {
    for<P> serialize { P: Serialize } deserialize { P: Deserialize<'de> }
    PlacementModifier<P>, key = "type", registry = crate::keys::PlacementModifierType,
    {
        BlockPredicateFilter => BlockPredicateFilter,
        RarityFilter => RarityFilter,
        RandomChance => RandomChance,
        SurfaceRelativeThresholdFilter => SurfaceRelativeThresholdFilter,
        SurfaceWaterDepthFilter => SurfaceWaterDepthFilter,
        Biome => Biome,
        Count => Count,
        NoiseBasedCount => NoiseBasedCount,
        NoiseThresholdCount => NoiseThresholdCount,
        CountOnEveryLayer => CountOnEveryLayer,
        Cuboid => Cuboid,
        EnvironmentScan => EnvironmentScan,
        Heightmap => Heightmap,
        HeightRange => HeightRange,
        InSquare => InSquare,
        Offset => Offset,
        RandomlySelected => RandomlySelected,
        FixedPlacement => FixedPlacement,
    }
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
// Declaration order is the step index; keep it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DecorationStep {
    RawGeneration,
    Lakes,
    LocalModifications,
    UndergroundStructures,
    SurfaceStructures,
    Strongholds,
    UndergroundOres,
    UndergroundDecoration,
    FluidSprings,
    VegetalDecoration,
    TopLayerModification,
}
