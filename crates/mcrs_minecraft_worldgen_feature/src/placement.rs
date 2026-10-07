use mcrs_minecraft_block_predicate::predicate::HeightmapName;
use serde::{Deserialize, Serialize};

use mcrs_minecraft_block_predicate::predicate::BlockPredicate;
use mcrs_minecraft_block_predicate::provider::UnitFloat;
use mcrs_minecraft_core::codec::{Bounded, PositiveInt, default_true, is_default, non_empty};
use mcrs_minecraft_value_provider::{BoundedIntProvider, HeightProvider};
use std::ops::RangeInclusive;

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
    /// The horizontal block range, relative to the chunk's own corner, that
    /// a position placed through this modifier can reach when it is handed
    /// positions inside `domain`.
    pub fn xz_domain(&self, domain: RangeInclusive<i32>) -> RangeInclusive<i32> {
        use PlacementModifier::*;
        let (min, max) = domain.into_inner();
        match self {
            InSquare {} | CountOnEveryLayer { .. } => min..=max + 15,
            Cuboid { xz_size, .. } => min..=max + xz_size.bounds().1 - 1,
            Offset { x, z, .. } => {
                let (x, z) = (x.bounds(), z.bounds());
                min + x.0.min(z.0)..=max + x.1.max(z.1)
            }
            RandomlySelected { placements } => placements
                .iter()
                .map(|placement| placement.xz_domain(min..=max))
                .reduce(|a, b| *a.start().min(b.start())..=*a.end().max(b.end()))
                .expect("a random selection holds a placement"),
            FixedPlacement { positions } => {
                let (min_section, max_section) = (min >> 4 << 4, max >> 4 << 4);
                positions
                    .iter()
                    .map(|&[x, _, z]| (x & 15, z & 15))
                    .map(|(x, z)| (min_section + x.min(z), max_section + x.max(z)))
                    .reduce(|a, b| (a.0.min(b.0), a.1.max(b.1)))
                    .map(|(low, high)| low..=high)
                    .expect("a fixed placement holds a position")
            }
            BlockPredicateFilter { .. }
            | RarityFilter { .. }
            | RandomChance { .. }
            | SurfaceRelativeThresholdFilter { .. }
            | SurfaceWaterDepthFilter { .. }
            | Biome {}
            | Count { .. }
            | NoiseBasedCount { .. }
            | NoiseThresholdCount { .. }
            | EnvironmentScan { .. }
            | Heightmap { .. }
            | HeightRange { .. } => min..=max,
        }
    }

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

#[cfg(test)]
mod tests {
    use super::*;

    const SWING: &str = r#"{"type":"minecraft:uniform","min_inclusive":-16,"max_inclusive":16}"#;

    #[test]
    fn the_xz_domain_of_each_modifier_follows_the_reference() {
        let square = r#"{"type":"minecraft:in_square"}"#;
        let offset =
            |x: &str, z: &str| format!(r#"{{"type":"minecraft:offset","x":{x},"y":0,"z":{z}}}"#);
        let uniform = |low: i32, high: i32| {
            format!(
                r#"{{"type":"minecraft:uniform","min_inclusive":{low},"max_inclusive":{high}}}"#
            )
        };
        let cuboid = |xz_size: &str| {
            format!(r#"{{"type":"minecraft:cuboid","xz_size":{xz_size},"y_size":1}}"#)
        };
        let selected = |branches: &[&str]| {
            format!(
                r#"{{"type":"minecraft:randomly_selected","placements":[{}]}}"#,
                branches.join(",")
            )
        };
        let fixed = |positions: &str| {
            format!(r#"{{"type":"minecraft:fixed_placement","positions":{positions}}}"#)
        };
        let filters = [
            r#"{"type":"minecraft:rarity_filter","chance":4}"#,
            r#"{"type":"minecraft:count","count":3}"#,
            r#"{"type":"minecraft:heightmap","heightmap":"MOTION_BLOCKING"}"#,
            r#"{"type":"minecraft:biome"}"#,
            r#"{"type":"minecraft:surface_water_depth_filter","max_water_depth":0}"#,
        ];

        let cases: Vec<(String, (i32, i32))> = vec![
            (String::new(), (0, 0)),
            (square.into(), (0, 15)),
            (format!("{square},{square}"), (0, 30)),
            (
                r#"{"type":"minecraft:count_on_every_layer","count":1}"#.into(),
                (0, 15),
            ),
            (cuboid("1"), (0, 0)),
            (cuboid(&uniform(1, 16)), (0, 15)),
            (format!("{square},{}", cuboid("16")), (0, 30)),
            (offset(&uniform(-3, 2), &uniform(-5, 7)), (-5, 7)),
            (offset("-4", "-2"), (-4, -2)),
            (offset("3", "5"), (3, 5)),
            (
                format!("{square},{}", offset(&uniform(-5, 7), "0")),
                (-5, 22),
            ),
            (format!("{square},{}", offset(SWING, SWING)), (-16, 31)),
            (selected(&[square, &offset("-7", "-7")]), (-7, 15)),
            (
                format!("{square},{}", selected(&[square, &offset("-7", "-7")])),
                (-7, 30),
            ),
            (fixed("[[0,64,0],[17,64,5]]"), (0, 5)),
            (
                format!("{square},{square},{}", fixed("[[3,0,9],[-1,0,20]]")),
                (3, 31),
            ),
            (
                format!("{},{}", offset("-16", "-16"), fixed("[[1,0,2]]")),
                (-15, -14),
            ),
            (format!("{square},{}", filters.join(",")), (0, 15)),
        ];

        mcrs_minecraft_worldgen_testing::corpus_set().scope(|| {
            for (placement, expected) in cases {
                let chain: Vec<PlacementModifier> = serde_json::from_str(&format!("[{placement}]"))
                    .unwrap_or_else(|error| panic!("{placement}: {error}"));
                let domain = chain
                    .iter()
                    .fold(0..=0, |domain, modifier| modifier.xz_domain(domain));
                assert_eq!((*domain.start(), *domain.end()), expected, "{placement}");
            }
        });
    }
}
