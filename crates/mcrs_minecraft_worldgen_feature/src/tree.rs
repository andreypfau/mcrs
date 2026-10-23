use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_block_predicate::provider::{BlockStateProvider, UnitFloat};
use mcrs_minecraft_registries::{KeySet, Named, dispatch};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

use mcrs_minecraft_core::Direction;
use mcrs_minecraft_core::codec::{Bounded, is_default, non_empty};
use mcrs_minecraft_core::{codec::Validate, validated};
use mcrs_minecraft_value_provider::{BoundedIntProvider, IntProvider};

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

pub type BaseHeight = Bounded<0, 32>;
pub type HeightRand = Bounded<0, 24>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(transparent)]
pub struct TrunkWidth(pub BoundedIntProvider<1, { i32::MAX }>);

impl Default for TrunkWidth {
    fn default() -> Self {
        TrunkWidth(BoundedIntProvider(IntProvider::Constant(1)))
    }
}

#[dispatch(registry = crate::keys::TrunkPlacerType)]
#[derive(Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum TrunkPlacer {
    #[dispatch(StraightTrunkPlacer)]
    Straight {
        base_height: BaseHeight,
        height_rand_a: HeightRand,
        height_rand_b: HeightRand,
        #[serde(default, skip_serializing_if = "is_default")]
        trunk_width: TrunkWidth,
    },
    #[dispatch(ForkingTrunkPlacer)]
    Forking {
        base_height: BaseHeight,
        height_rand_a: HeightRand,
        height_rand_b: HeightRand,
    },
    #[dispatch(GiantTrunkPlacer)]
    Giant {
        base_height: BaseHeight,
        height_rand_a: HeightRand,
        height_rand_b: HeightRand,
    },
    #[dispatch(MegaJungleTrunkPlacer)]
    MegaJungle {
        base_height: BaseHeight,
        height_rand_a: HeightRand,
        height_rand_b: HeightRand,
    },
    #[dispatch(DarkOakTrunkPlacer)]
    DarkOak {
        base_height: BaseHeight,
        height_rand_a: HeightRand,
        height_rand_b: HeightRand,
    },
    #[dispatch(FancyTrunkPlacer)]
    Fancy {
        base_height: BaseHeight,
        height_rand_a: HeightRand,
        height_rand_b: HeightRand,
    },
    #[dispatch(BendingTrunkPlacer)]
    Bending {
        base_height: BaseHeight,
        height_rand_a: HeightRand,
        height_rand_b: HeightRand,
        #[serde(default, skip_serializing_if = "is_default")]
        min_height_for_leaves: Bounded<1, { i32::MAX }, 1>,
        bend_length: IntProvider,
    },
    #[dispatch(UpwardsBranchingTrunkPlacer)]
    UpwardsBranching {
        base_height: BaseHeight,
        height_rand_a: HeightRand,
        height_rand_b: HeightRand,
        extra_branch_steps: IntProvider,
        place_branch_per_log_probability: UnitFloat,
        extra_branch_length: IntProvider,
        can_grow_through: KeySet<Block>,
    },
    #[dispatch(CherryTrunkPlacer)]
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
    #[dispatch(PoplarTrunkPlacer)]
    Poplar {
        base_height: BaseHeight,
        height_rand_a: HeightRand,
        height_rand_b: HeightRand,
        trunk_height_above_branches: IntProvider,
        branch_amount: IntProvider,
    },
}

#[dispatch(registry = crate::keys::FoliagePlacerType)]
#[derive(Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum FoliagePlacer {
    #[dispatch(BlobFoliagePlacer)]
    Blob {
        radius: IntProvider,
        offset: IntProvider,
        height: Bounded<0, 16>,
    },
    #[dispatch(BushFoliagePlacer)]
    Bush {
        radius: IntProvider,
        offset: IntProvider,
        height: Bounded<0, 16>,
    },
    #[dispatch(FancyFoliagePlacer)]
    Fancy {
        radius: IntProvider,
        offset: IntProvider,
        height: Bounded<0, 16>,
    },
    #[dispatch(SpruceFoliagePlacer)]
    Spruce {
        radius: IntProvider,
        offset: IntProvider,
        trunk_height: IntProvider,
    },
    #[dispatch(PineFoliagePlacer)]
    Pine {
        radius: IntProvider,
        offset: IntProvider,
        height: IntProvider,
    },
    #[dispatch(AcaciaFoliagePlacer)]
    Acacia {
        radius: IntProvider,
        offset: IntProvider,
    },
    #[dispatch(DarkOakFoliagePlacer)]
    DarkOak {
        radius: IntProvider,
        offset: IntProvider,
    },
    #[dispatch(JungleFoliagePlacer)]
    MegaJungle {
        radius: IntProvider,
        offset: IntProvider,
        height: Bounded<0, 16>,
    },
    #[dispatch(MegaPineFoliagePlacer)]
    MegaPine {
        radius: IntProvider,
        offset: IntProvider,
        crown_height: IntProvider,
    },
    #[dispatch(RandomSpreadFoliagePlacer)]
    RandomSpread {
        radius: IntProvider,
        offset: IntProvider,
        foliage_height: IntProvider,
        leaf_placement_attempts: Bounded<0, 256>,
    },
    #[dispatch(CherryFoliagePlacer)]
    Cherry {
        radius: IntProvider,
        offset: IntProvider,
        height: IntProvider,
        wide_bottom_layer_hole_chance: UnitFloat,
        corner_hole_chance: UnitFloat,
        hanging_leaves_chance: UnitFloat,
        hanging_leaves_extension_chance: UnitFloat,
    },
    #[dispatch(PoplarFoliagePlacer)]
    Poplar {
        radius: IntProvider,
        offset: IntProvider,
        height: IntProvider,
        side_hole_chance: UnitFloat,
    },
}

/// `P` is the block state provider four decorators carry: the datapack's
/// [`BlockStateProvider`] as loaded, or whatever a freeze resolves it into.
#[dispatch(registry = crate::keys::TreeDecoratorType)]
#[derive(Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum TreeDecorator<P = BlockStateProvider<Named>> {
    TrunkVine {},
    LeaveVine {
        probability: UnitFloat,
    },
    PaleMoss {
        leaves_probability: UnitFloat,
        trunk_probability: UnitFloat,
        ground_probability: UnitFloat,
    },
    CreakingHeart {
        probability: UnitFloat,
    },
    Cocoa {
        probability: UnitFloat,
    },
    ShelfMushroom {
        probability: UnitFloat,
    },
    Beehive {
        probability: UnitFloat,
    },
    AlterGround {
        provider: P,
    },
    AttachedToLeaves {
        probability: UnitFloat,
        exclusion_radius_xz: Bounded<0, 16>,
        exclusion_radius_y: Bounded<0, 16>,
        block_provider: P,
        required_empty_blocks: Bounded<1, 16>,
        #[serde(deserialize_with = "non_empty")]
        directions: Vec<Direction>,
    },
    PlaceOnGround {
        #[serde(default, skip_serializing_if = "is_default")]
        tries: Bounded<1, { i32::MAX }, 128>,
        #[serde(default, skip_serializing_if = "is_default")]
        radius: Bounded<0, { i32::MAX }, 2>,
        #[serde(default, skip_serializing_if = "is_default")]
        height: Bounded<0, { i32::MAX }, 1>,
        block_state_provider: P,
    },
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

#[dispatch(registry = crate::keys::FeatureSizeType)]
#[derive(Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum FeatureSize {
    #[dispatch(TwoLayersFeatureSize)]
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
    #[dispatch(ThreeLayersFeatureSize)]
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

#[dispatch(registry = crate::keys::RootPlacerType)]
#[derive(Debug, Clone, PartialEq)]
#[serde(deny_unknown_fields)]
pub enum RootPlacer {
    #[dispatch(MangroveRootPlacer)]
    Mangrove {
        trunk_offset_y: IntProvider,
        root_provider: BlockStateProvider<Named>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        above_root_placement: Option<AboveRootPlacement>,
        mangrove_root_placement: MangroveRootPlacement,
    },
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AboveRootPlacement {
    pub above_root_provider: BlockStateProvider<Named>,
    pub above_root_placement_chance: UnitFloat,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MangroveRootPlacement {
    pub can_grow_through: KeySet<Block>,
    pub muddy_roots_in: KeySet<Block>,
    pub muddy_roots_provider: BlockStateProvider<Named>,
    pub max_root_width: Bounded<1, 12>,
    pub max_root_length: Bounded<1, 64>,
    pub random_skew_chance: UnitFloat,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct TreeConfig {
    pub trunk_provider: BlockStateProvider<Named>,
    pub trunk_placer: TrunkPlacer,
    pub foliage_provider: BlockStateProvider<Named>,
    pub foliage_placer: FoliagePlacer,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_placer: Option<RootPlacer>,
    pub minimum_size: FeatureSize,
    pub decorators: Vec<TreeDecorator>,
    /// `fieldOf(…).orElse(false)`, which unlike an optional field is written
    /// back even when it holds the default.
    #[serde(default)]
    pub ignore_vines: bool,
    pub below_trunk_provider: BlockStateProvider<Named>,
}
