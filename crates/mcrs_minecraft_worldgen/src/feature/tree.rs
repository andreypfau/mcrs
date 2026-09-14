use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

use super::block_predicate::{BlockPredicate, Direction};
use crate::proto::{BlockState, NoiseParam};
use mcrs_minecraft_core::HolderSet;
use mcrs_minecraft_core::codec::{Bounded, is_default};
use mcrs_minecraft_core::value_provider::{IntProvider, Weighted};
use mcrs_minecraft_core::{codec::Validate, validated};

super::proto::bounded_float! {
    /// `Codec.floatRange(0.0F, 1.0F)`.
    UnitFloat as f32 in 0.0 ..= 1.0;
    /// `Codec.floatRange(-1.0F, 1.0F)`.
    SignedUnitFloat as f32 in -1.0 ..= 1.0;
}

/// `ExtraCodecs.POSITIVE_FLOAT`: the low bound is exclusive.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(try_from = "f64")]
pub struct PositiveFloat(pub f64);

impl TryFrom<f64> for PositiveFloat {
    type Error = String;

    fn try_from(value: f64) -> Result<Self, String> {
        let narrowed = value as f32;
        if narrowed.is_nan() || narrowed <= 0.0 {
            return Err(format!("Value must be positive: {value}"));
        }
        Ok(PositiveFloat(value))
    }
}

pub(crate) fn non_empty<'de, D, T>(deserializer: D) -> Result<Vec<T>, D::Error>
where
    D: Deserializer<'de>,
    T: Deserialize<'de>,
{
    let values = Vec::<T>::deserialize(deserializer)?;
    if values.is_empty() {
        return Err(D::Error::custom("List must have contents"));
    }
    Ok(values)
}

/// `RegistryCodecs.holderSet(Registries.BLOCK)`.
pub type BlockSet = HolderSet;

/// `ExtraCodecs.intervalCodec`: one point, a two-element array, or the named
/// pair. Vanilla re-encodes all three as the shortest form that fits, so the
/// three are kept apart to round-trip instead.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(untagged)]
pub enum IntRange {
    Point(i32),
    Pair([i32; 2]),
    Named {
        min_inclusive: i32,
        max_inclusive: i32,
    },
}

impl IntRange {
    pub fn min_inclusive(self) -> i32 {
        match self {
            IntRange::Point(value) => value,
            IntRange::Pair([min, _]) => min,
            IntRange::Named { min_inclusive, .. } => min_inclusive,
        }
    }

    pub fn max_inclusive(self) -> i32 {
        match self {
            IntRange::Point(value) => value,
            IntRange::Pair([_, max]) => max,
            IntRange::Named { max_inclusive, .. } => max_inclusive,
        }
    }
}

/// `UniformInt.MAP_CODEC` reached directly rather than through the int-provider
/// dispatch, so the object carries no `type`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(deny_unknown_fields, remote = "Self")]
pub struct UniformIntRange {
    pub min_inclusive: i32,
    pub max_inclusive: i32,
}

impl Validate for UniformIntRange {
    fn validate(&self) -> Result<(), String> {
        if self.max_inclusive < self.min_inclusive {
            return Err(format!(
                "Max must be at least min, min_inclusive: {}, max_inclusive: {}",
                self.min_inclusive, self.max_inclusive
            ));
        }
        Ok(())
    }
}

validated!(UniformIntRange);

fn variety_range<'de, D: Deserializer<'de>>(deserializer: D) -> Result<IntRange, D::Error> {
    let range = IntRange::deserialize(deserializer)?;
    if range.min_inclusive() < 1 {
        return Err(D::Error::custom(format!(
            "Range limit too low, expected at least 1 [{}-{}]",
            range.min_inclusive(),
            range.max_inclusive()
        )));
    }
    if range.max_inclusive() > 64 {
        return Err(D::Error::custom(format!(
            "Range limit too high, expected at most 64 [{}-{}]",
            range.min_inclusive(),
            range.max_inclusive()
        )));
    }
    Ok(range)
}

fn branch_start_offset<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<UniformIntRange, D::Error> {
    let range = UniformIntRange::deserialize(deserializer)?;
    if !(-16..=0).contains(&range.min_inclusive) || !(-16..=0).contains(&range.max_inclusive) {
        return Err(D::Error::custom(format!(
            "Value provider outside [-16;0]: [{}-{}]",
            range.min_inclusive, range.max_inclusive
        )));
    }
    if range.max_inclusive - range.min_inclusive < 1 {
        return Err(D::Error::custom(
            "Need at least 2 blocks variation for the branch starts to fit both branches",
        ));
    }
    Ok(range)
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum BlockStateProvider {
    #[serde(rename = "minecraft:simple_state_provider")]
    Simple { state: BlockState },
    #[serde(rename = "minecraft:weighted_state_provider")]
    Weighted {
        #[serde(deserialize_with = "non_empty")]
        entries: Vec<Weighted<BlockState>>,
    },
    #[serde(rename = "minecraft:rule_based_state_provider")]
    RuleBased {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        fallback: Option<Box<BlockStateProvider>>,
        rules: Vec<StateRule>,
    },
    #[serde(rename = "minecraft:randomized_int_state_provider")]
    RandomizedInt {
        source: Box<BlockStateProvider>,
        property: String,
        values: IntProvider,
    },
    #[serde(rename = "minecraft:rotated_block_provider")]
    Rotated {
        state: Box<BlockStateProvider>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        direction: Option<Direction>,
    },
    #[serde(rename = "minecraft:random_block_provider")]
    RandomBlock { blocks: BlockSet },
    #[serde(rename = "minecraft:copy_properties_provider")]
    CopyProperties { source: Box<BlockStateProvider> },
    #[serde(rename = "minecraft:noise_provider")]
    Noise {
        seed: i64,
        noise: NoiseParam,
        scale: PositiveFloat,
        #[serde(deserialize_with = "non_empty")]
        states: Vec<BlockState>,
    },
    #[serde(rename = "minecraft:noise_threshold_provider")]
    NoiseThreshold {
        seed: i64,
        noise: NoiseParam,
        scale: PositiveFloat,
        threshold: SignedUnitFloat,
        high_chance: UnitFloat,
        default_state: BlockState,
        #[serde(deserialize_with = "non_empty")]
        low_states: Vec<BlockState>,
        #[serde(deserialize_with = "non_empty")]
        high_states: Vec<BlockState>,
    },
    #[serde(rename = "minecraft:dual_noise_provider")]
    DualNoise {
        #[serde(deserialize_with = "variety_range")]
        variety: IntRange,
        slow_noise: NoiseParam,
        slow_scale: PositiveFloat,
        seed: i64,
        noise: NoiseParam,
        scale: PositiveFloat,
        #[serde(deserialize_with = "non_empty")]
        states: Vec<BlockState>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct StateRule {
    pub if_true: BlockPredicate,
    pub then: BlockStateProvider,
}

pub type BaseHeight = Bounded<0, 32>;
pub type HeightRand = Bounded<0, 24>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum TrunkPlacer {
    #[serde(rename = "minecraft:straight_trunk_placer")]
    Straight {
        base_height: BaseHeight,
        height_rand_a: HeightRand,
        height_rand_b: HeightRand,
    },
    #[serde(rename = "minecraft:forking_trunk_placer")]
    Forking {
        base_height: BaseHeight,
        height_rand_a: HeightRand,
        height_rand_b: HeightRand,
    },
    #[serde(rename = "minecraft:giant_trunk_placer")]
    Giant {
        base_height: BaseHeight,
        height_rand_a: HeightRand,
        height_rand_b: HeightRand,
    },
    #[serde(rename = "minecraft:mega_jungle_trunk_placer")]
    MegaJungle {
        base_height: BaseHeight,
        height_rand_a: HeightRand,
        height_rand_b: HeightRand,
    },
    #[serde(rename = "minecraft:dark_oak_trunk_placer")]
    DarkOak {
        base_height: BaseHeight,
        height_rand_a: HeightRand,
        height_rand_b: HeightRand,
    },
    #[serde(rename = "minecraft:fancy_trunk_placer")]
    Fancy {
        base_height: BaseHeight,
        height_rand_a: HeightRand,
        height_rand_b: HeightRand,
    },
    #[serde(rename = "minecraft:bending_trunk_placer")]
    Bending {
        base_height: BaseHeight,
        height_rand_a: HeightRand,
        height_rand_b: HeightRand,
        #[serde(default, skip_serializing_if = "is_default")]
        min_height_for_leaves: Bounded<1, { i32::MAX }, 1>,
        bend_length: IntProvider,
    },
    #[serde(rename = "minecraft:upwards_branching_trunk_placer")]
    UpwardsBranching {
        base_height: BaseHeight,
        height_rand_a: HeightRand,
        height_rand_b: HeightRand,
        extra_branch_steps: IntProvider,
        place_branch_per_log_probability: UnitFloat,
        extra_branch_length: IntProvider,
        can_grow_through: BlockSet,
    },
    #[serde(rename = "minecraft:cherry_trunk_placer")]
    Cherry {
        base_height: BaseHeight,
        height_rand_a: HeightRand,
        height_rand_b: HeightRand,
        branch_count: IntProvider,
        branch_horizontal_length: IntProvider,
        #[serde(deserialize_with = "branch_start_offset")]
        branch_start_offset_from_top: UniformIntRange,
        branch_end_offset_from_top: IntProvider,
    },
    #[serde(rename = "minecraft:poplar_trunk_placer")]
    Poplar {
        base_height: BaseHeight,
        height_rand_a: HeightRand,
        height_rand_b: HeightRand,
        trunk_height_above_branches: IntProvider,
        branch_amount: IntProvider,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum FoliagePlacer {
    #[serde(rename = "minecraft:blob_foliage_placer")]
    Blob {
        radius: IntProvider,
        offset: IntProvider,
        height: Bounded<0, 16>,
    },
    #[serde(rename = "minecraft:bush_foliage_placer")]
    Bush {
        radius: IntProvider,
        offset: IntProvider,
        height: Bounded<0, 16>,
    },
    #[serde(rename = "minecraft:fancy_foliage_placer")]
    Fancy {
        radius: IntProvider,
        offset: IntProvider,
        height: Bounded<0, 16>,
    },
    #[serde(rename = "minecraft:spruce_foliage_placer")]
    Spruce {
        radius: IntProvider,
        offset: IntProvider,
        trunk_height: IntProvider,
    },
    #[serde(rename = "minecraft:pine_foliage_placer")]
    Pine {
        radius: IntProvider,
        offset: IntProvider,
        height: IntProvider,
    },
    #[serde(rename = "minecraft:acacia_foliage_placer")]
    Acacia {
        radius: IntProvider,
        offset: IntProvider,
    },
    #[serde(rename = "minecraft:dark_oak_foliage_placer")]
    DarkOak {
        radius: IntProvider,
        offset: IntProvider,
    },
    #[serde(rename = "minecraft:jungle_foliage_placer")]
    MegaJungle {
        radius: IntProvider,
        offset: IntProvider,
        height: Bounded<0, 16>,
    },
    #[serde(rename = "minecraft:mega_pine_foliage_placer")]
    MegaPine {
        radius: IntProvider,
        offset: IntProvider,
        crown_height: IntProvider,
    },
    #[serde(rename = "minecraft:random_spread_foliage_placer")]
    RandomSpread {
        radius: IntProvider,
        offset: IntProvider,
        foliage_height: IntProvider,
        leaf_placement_attempts: Bounded<0, 256>,
    },
    #[serde(rename = "minecraft:cherry_foliage_placer")]
    Cherry {
        radius: IntProvider,
        offset: IntProvider,
        height: IntProvider,
        wide_bottom_layer_hole_chance: UnitFloat,
        corner_hole_chance: UnitFloat,
        hanging_leaves_chance: UnitFloat,
        hanging_leaves_extension_chance: UnitFloat,
    },
    #[serde(rename = "minecraft:poplar_foliage_placer")]
    Poplar {
        radius: IntProvider,
        offset: IntProvider,
        height: IntProvider,
        side_hole_chance: UnitFloat,
    },
}

/// `P` is the block state provider four decorators carry: the datapack's
/// [`BlockStateProvider`] as loaded, or whatever a freeze resolves it into.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum TreeDecorator<P = BlockStateProvider> {
    #[serde(rename = "minecraft:trunk_vine")]
    TrunkVine {},
    #[serde(rename = "minecraft:leave_vine")]
    LeaveVine { probability: UnitFloat },
    #[serde(rename = "minecraft:pale_moss")]
    PaleMoss {
        leaves_probability: UnitFloat,
        trunk_probability: UnitFloat,
        ground_probability: UnitFloat,
    },
    #[serde(rename = "minecraft:creaking_heart")]
    CreakingHeart { probability: UnitFloat },
    #[serde(rename = "minecraft:cocoa")]
    Cocoa { probability: UnitFloat },
    #[serde(rename = "minecraft:shelf_mushroom")]
    ShelfMushroom { probability: UnitFloat },
    #[serde(rename = "minecraft:beehive")]
    Beehive { probability: UnitFloat },
    #[serde(rename = "minecraft:alter_ground")]
    AlterGround { provider: P },
    #[serde(rename = "minecraft:attached_to_leaves")]
    AttachedToLeaves {
        probability: UnitFloat,
        exclusion_radius_xz: Bounded<0, 16>,
        exclusion_radius_y: Bounded<0, 16>,
        block_provider: P,
        required_empty_blocks: Bounded<1, 16>,
        #[serde(deserialize_with = "non_empty")]
        directions: Vec<Direction>,
    },
    #[serde(rename = "minecraft:place_on_ground")]
    PlaceOnGround {
        #[serde(default, skip_serializing_if = "is_default")]
        tries: Bounded<1, { i32::MAX }, 128>,
        #[serde(default, skip_serializing_if = "is_default")]
        radius: Bounded<0, { i32::MAX }, 2>,
        #[serde(default, skip_serializing_if = "is_default")]
        height: Bounded<0, { i32::MAX }, 1>,
        block_state_provider: P,
    },
    #[serde(rename = "minecraft:attached_to_logs")]
    AttachedToLogs {
        probability: UnitFloat,
        block_provider: P,
        #[serde(deserialize_with = "non_empty")]
        directions: Vec<Direction>,
    },
}

impl<P> TreeDecorator<P> {
    /// The same decorator with its provider, if it carries one, resolved by `f`.
    pub fn map_provider<Q, E>(
        &self,
        mut f: impl FnMut(&P) -> Result<Q, E>,
    ) -> Result<TreeDecorator<Q>, E> {
        use TreeDecorator::*;
        Ok(match self {
            TrunkVine {} => TrunkVine {},
            LeaveVine { probability } => LeaveVine {
                probability: *probability,
            },
            PaleMoss {
                leaves_probability,
                trunk_probability,
                ground_probability,
            } => PaleMoss {
                leaves_probability: *leaves_probability,
                trunk_probability: *trunk_probability,
                ground_probability: *ground_probability,
            },
            CreakingHeart { probability } => CreakingHeart {
                probability: *probability,
            },
            Cocoa { probability } => Cocoa {
                probability: *probability,
            },
            ShelfMushroom { probability } => ShelfMushroom {
                probability: *probability,
            },
            Beehive { probability } => Beehive {
                probability: *probability,
            },
            AlterGround { provider } => AlterGround {
                provider: f(provider)?,
            },
            AttachedToLeaves {
                probability,
                exclusion_radius_xz,
                exclusion_radius_y,
                block_provider,
                required_empty_blocks,
                directions,
            } => AttachedToLeaves {
                probability: *probability,
                exclusion_radius_xz: *exclusion_radius_xz,
                exclusion_radius_y: *exclusion_radius_y,
                block_provider: f(block_provider)?,
                required_empty_blocks: *required_empty_blocks,
                directions: directions.clone(),
            },
            PlaceOnGround {
                tries,
                radius,
                height,
                block_state_provider,
            } => PlaceOnGround {
                tries: *tries,
                radius: *radius,
                height: *height,
                block_state_provider: f(block_state_provider)?,
            },
            AttachedToLogs {
                probability,
                block_provider,
                directions,
            } => AttachedToLogs {
                probability: *probability,
                block_provider: f(block_provider)?,
                directions: directions.clone(),
            },
        })
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum FeatureSize {
    #[serde(rename = "minecraft:two_layers_feature_size")]
    TwoLayers {
        #[serde(default, skip_serializing_if = "is_default")]
        limit: Bounded<0, 81, 1>,
        #[serde(default, skip_serializing_if = "is_default")]
        lower_size: Bounded<0, 16>,
        #[serde(default, skip_serializing_if = "is_default")]
        upper_size: Bounded<0, 16, 1>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min_clipped_height: Option<Bounded<0, 80>>,
    },
    #[serde(rename = "minecraft:three_layers_feature_size")]
    ThreeLayers {
        #[serde(default, skip_serializing_if = "is_default")]
        limit: Bounded<0, 80, 1>,
        #[serde(default, skip_serializing_if = "is_default")]
        upper_limit: Bounded<0, 80, 1>,
        #[serde(default, skip_serializing_if = "is_default")]
        lower_size: Bounded<0, 16>,
        #[serde(default, skip_serializing_if = "is_default")]
        middle_size: Bounded<0, 16, 1>,
        #[serde(default, skip_serializing_if = "is_default")]
        upper_size: Bounded<0, 16, 1>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        min_clipped_height: Option<Bounded<0, 80>>,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum RootPlacer {
    #[serde(rename = "minecraft:mangrove_root_placer")]
    Mangrove {
        trunk_offset_y: IntProvider,
        root_provider: BlockStateProvider,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        above_root_placement: Option<AboveRootPlacement>,
        mangrove_root_placement: MangroveRootPlacement,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AboveRootPlacement {
    pub above_root_provider: BlockStateProvider,
    pub above_root_placement_chance: UnitFloat,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MangroveRootPlacement {
    pub can_grow_through: BlockSet,
    pub muddy_roots_in: BlockSet,
    pub muddy_roots_provider: BlockStateProvider,
    pub max_root_width: Bounded<1, 12>,
    pub max_root_length: Bounded<1, 64>,
    pub random_skew_chance: UnitFloat,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TreeConfig {
    pub trunk_provider: BlockStateProvider,
    pub trunk_placer: TrunkPlacer,
    pub foliage_provider: BlockStateProvider,
    pub foliage_placer: FoliagePlacer,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_placer: Option<RootPlacer>,
    pub minimum_size: FeatureSize,
    pub decorators: Vec<TreeDecorator>,
    /// `fieldOf(…).orElse(false)`, which unlike an optional field is written
    /// back even when it holds the default.
    #[serde(default)]
    pub ignore_vines: bool,
    pub below_trunk_provider: BlockStateProvider,
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Neither shape occurs in the shipped corpus, so nothing else covers them.
    #[test]
    fn a_copied_provider_and_a_clipped_size_round_trip() {
        let copied = r#"{"type":"minecraft:copy_properties_provider","source":{"type":"minecraft:simple_state_provider","state":"minecraft:oak_log"}}"#;
        let provider: BlockStateProvider = serde_json::from_str(copied).unwrap();
        assert_eq!(serde_json::to_string(&provider).unwrap(), copied);

        let clipped = r#"{"type":"minecraft:two_layers_feature_size","min_clipped_height":4}"#;
        let size: FeatureSize = serde_json::from_str(clipped).unwrap();
        assert_eq!(serde_json::to_string(&size).unwrap(), clipped);

        let error = serde_json::from_str::<FeatureSize>(
            r#"{"type":"minecraft:two_layers_feature_size","upper_size":17}"#,
        )
        .unwrap_err()
        .to_string();
        assert!(error.contains("[0;16]: 17"), "{error}");
    }
}
