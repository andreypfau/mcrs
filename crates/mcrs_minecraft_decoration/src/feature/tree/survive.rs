use crate::feature::holds;
use bevy_math::IVec3;
use fixedbitset::FixedBitSet;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::BlockPos;
use mcrs_minecraft_worldgen::feature::block_predicate::Direction;

/// The block tags a rule is built from. The fluid tags of the same name are
/// separate registries and fold into the same masks: a fluid test is a question
/// about the block state that carries the fluid.
///
/// A block whose `canSurvive` is one tag read below carries that as the
/// `minecraft:placement_filter` component of its definition instead and has no
/// family here.
pub const SUPPORTS_VEGETATION: &str = "minecraft:supports_vegetation";
pub const OVERRIDES_MUSHROOM_LIGHT_REQUIREMENT: &str =
    "minecraft:overrides_mushroom_light_requirement";
pub const SUPPORTS_LILY_PAD: &str = "minecraft:supports_lily_pad";
pub const UNSTABLE_BOTTOM_CENTER: &str = "minecraft:unstable_bottom_center";
pub const SUPPORTS_SUGAR_CANE: &str = "minecraft:supports_sugar_cane";
pub const SUPPORTS_SUGAR_CANE_ADJACENTLY: &str = "minecraft:supports_sugar_cane_adjacently";
pub const SUPPORTS_CACTUS: &str = "minecraft:supports_cactus";
pub const CANNOT_SUPPORT_SEAGRASS: &str = "minecraft:cannot_support_seagrass";
pub const SUPPORTS_SMALL_DRIPLEAF: &str = "minecraft:supports_small_dripleaf";

/// Which shape a state's block takes. A block that answers `None` overrides
/// nothing and takes the default `canSurvive`, which is true.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurviveFamily {
    /// `MushroomBlock`: the light requirement is met wherever features run,
    /// because the light engine has not run yet and the raw brightness is 0.
    Mushroom,
    /// `CarpetBlock`, and `MossyCarpetBlock` in its base form.
    OnAnythingBelow,
    /// `LeafLitterBlock`, and `FireBlock` reduced to its sturdy-floor arm.
    OnSturdyFaceBelow,
    /// `SeaPickleBlock`: the block below only has to present some upward face.
    SeaPickle,
    /// `SporeBlossomBlock`: hangs from a ceiling that supports its centre, out
    /// of water.
    SporeBlossom,
    LilyPad,
    SugarCane,
    Cactus,
    /// `SeagrassBlock`: a sturdy face below outside `cannot_support_seagrass`.
    Seagrass,
    /// `TallSeagrassBlock`: the same, and its own square must be water.
    TallSeagrass,
    /// `SmallDripleafBlock`: `supports_small_dripleaf` below, or vegetation
    /// soil below when its own square is water.
    SmallDripleaf,
}

pub fn family_of(block: &str) -> Option<SurviveFamily> {
    use SurviveFamily::*;
    let name = block.strip_prefix("minecraft:").unwrap_or(block);
    Some(match name {
        "sugar_cane" => SugarCane,
        "cactus" => Cactus,
        "brown_mushroom" | "red_mushroom" => Mushroom,
        "moss_carpet" | "pale_moss_carpet" => OnAnythingBelow,
        "leaf_litter" | "fire" => OnSturdyFaceBelow,
        "sea_pickle" => SeaPickle,
        "spore_blossom" => SporeBlossom,
        "lily_pad" => LilyPad,
        "seagrass" => Seagrass,
        "tall_seagrass" => TallSeagrass,
        "small_dripleaf" => SmallDripleaf,
        _ => return None,
    })
}

/// One family's rule with every set it reads reduced to a mask over state ids.
#[derive(Clone, Debug)]
pub enum SurviveRule {
    /// One neighbour on the vertical against one mask. `offset_y` is -1 below,
    /// +1 above.
    SupportedBy {
        offset_y: i32,
        supports: FixedBitSet,
    },
    /// The same, and the state at the position itself must stay out of
    /// `blocked`: the lily pad's own square must hold no fluid, the spore
    /// blossom's no water.
    SupportedByUnless {
        offset_y: i32,
        supports: FixedBitSet,
        blocked: FixedBitSet,
    },
    SugarCane {
        sugar_cane: FixedBitSet,
        supports: FixedBitSet,
        /// The block tag and the fluid tag of `supports_sugar_cane_adjacently`
        /// together: the reference reads both at each neighbour and ors them.
        adjacent: FixedBitSet,
    },
    Cactus {
        /// Below: cactus itself, or `supports_cactus`.
        supports: FixedBitSet,
        /// A horizontal neighbour that refuses the cactus: solid, or lava.
        blocked: FixedBitSet,
        /// Above, as `BlockState.liquid`.
        liquid: FixedBitSet,
    },
    SmallDripleaf {
        supports: FixedBitSet,
        /// Below, only when the position itself holds water.
        wet_supports: FixedBitSet,
        water: FixedBitSet,
    },
}

fn any_horizontal(p: BlockPos, test: impl Fn(BlockPos) -> bool) -> bool {
    Direction::HORIZONTAL.iter().any(|d| test(p + d.normal()))
}

impl SurviveRule {
    pub fn test(&self, p: BlockPos, get: impl Fn(BlockPos) -> VoxelId) -> bool {
        match self {
            SurviveRule::SupportedBy { offset_y, supports } => {
                holds(supports, get(p + IVec3::Y * *offset_y))
            }
            SurviveRule::SupportedByUnless {
                offset_y,
                supports,
                blocked,
            } => holds(supports, get(p + IVec3::Y * *offset_y)) && !holds(blocked, get(p)),
            SurviveRule::SugarCane {
                sugar_cane,
                supports,
                adjacent,
            } => {
                let below = get(p - IVec3::Y);
                if holds(sugar_cane, below) {
                    return true;
                }
                holds(supports, below) && any_horizontal(p - IVec3::Y, |q| holds(adjacent, get(q)))
            }
            SurviveRule::Cactus {
                supports,
                blocked,
                liquid,
            } => {
                if any_horizontal(p, |q| holds(blocked, get(q))) {
                    return false;
                }
                holds(supports, get(p - IVec3::Y)) && !holds(liquid, get(p + IVec3::Y))
            }
            SurviveRule::SmallDripleaf {
                supports,
                wet_supports,
                water,
            } => {
                let below = get(p - IVec3::Y);
                holds(supports, below) || (holds(water, get(p)) && holds(wet_supports, below))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use mcrs_minecraft_worldgen::feature::placer::mask_of;

    const AIR: VoxelId = VoxelId(0);
    const DIRT: VoxelId = VoxelId(1);
    const SAND: VoxelId = VoxelId(2);
    const CACTUS: VoxelId = VoxelId(3);
    const SUGAR_CANE: VoxelId = VoxelId(4);
    const WATER: VoxelId = VoxelId(5);
    const STONE: VoxelId = VoxelId(6);
    const LEAVES: VoxelId = VoxelId(7);

    /// A one-column world: `below` under the origin, `air` everywhere else,
    /// with the four horizontal neighbours of the origin and of `below`
    /// answered from the two rings.
    fn world(
        below: VoxelId,
        above: VoxelId,
        ring: VoxelId,
        under_ring: VoxelId,
    ) -> impl Fn(BlockPos) -> VoxelId {
        move |p| match (p.x == 0 && p.z == 0, p.y) {
            (true, -1) => below,
            (true, 1) => above,
            (true, _) => AIR,
            (false, 0) => ring,
            (false, -1) => under_ring,
            (false, _) => AIR,
        }
    }

    #[test]
    fn vegetation_reads_one_block_below() {
        let rule = SurviveRule::SupportedBy {
            offset_y: -1,
            supports: mask_of([DIRT]).as_ref().clone(),
        };
        assert!(rule.test(BlockPos::new(0, 0, 0), world(DIRT, AIR, AIR, AIR)));
        assert!(!rule.test(BlockPos::new(0, 0, 0), world(STONE, AIR, AIR, AIR)));
    }

    #[test]
    fn a_hanging_propagule_reads_above_instead() {
        let rule = SurviveRule::SupportedBy {
            offset_y: 1,
            supports: mask_of([LEAVES]).as_ref().clone(),
        };
        assert!(rule.test(BlockPos::new(0, 0, 0), world(DIRT, LEAVES, AIR, AIR)));
        assert!(!rule.test(BlockPos::new(0, 0, 0), world(LEAVES, AIR, AIR, AIR)));
    }

    #[test]
    fn a_lily_pad_wants_water_below_and_nothing_in_its_own_square() {
        let rule = SurviveRule::SupportedByUnless {
            offset_y: -1,
            supports: mask_of([WATER]).as_ref().clone(),
            blocked: mask_of([WATER]).as_ref().clone(),
        };
        assert!(rule.test(BlockPos::new(0, 0, 0), world(WATER, AIR, AIR, AIR)));
        assert!(!rule.test(BlockPos::new(0, 0, 0), world(DIRT, AIR, AIR, AIR)));
        let submerged = |p: BlockPos| if p.y <= 0 { WATER } else { AIR };
        assert!(!rule.test(BlockPos::new(0, 0, 0), submerged));
    }

    #[test]
    fn sugar_cane_stacks_on_itself_without_asking_for_water() {
        let rule = SurviveRule::SugarCane {
            sugar_cane: mask_of([SUGAR_CANE]).as_ref().clone(),
            supports: mask_of([SAND]).as_ref().clone(),
            adjacent: mask_of([WATER]).as_ref().clone(),
        };
        assert!(rule.test(BlockPos::new(0, 0, 0), world(SUGAR_CANE, AIR, AIR, AIR)));
        assert!(
            !rule.test(BlockPos::new(0, 0, 0), world(SAND, AIR, WATER, AIR)),
            "the adjacency is read beside the block below, not beside the cane"
        );
        assert!(rule.test(BlockPos::new(0, 0, 0), world(SAND, AIR, AIR, WATER)));
        assert!(!rule.test(BlockPos::new(0, 0, 0), world(STONE, AIR, AIR, WATER)));
    }

    #[test]
    fn cactus_refuses_a_solid_neighbour_and_a_liquid_above() {
        let rule = SurviveRule::Cactus {
            supports: mask_of([SAND, CACTUS]).as_ref().clone(),
            blocked: mask_of([STONE, SAND]).as_ref().clone(),
            liquid: mask_of([WATER]).as_ref().clone(),
        };
        assert!(rule.test(BlockPos::new(0, 0, 0), world(SAND, AIR, AIR, AIR)));
        assert!(rule.test(BlockPos::new(0, 0, 0), world(CACTUS, AIR, AIR, AIR)));
        assert!(!rule.test(BlockPos::new(0, 0, 0), world(SAND, AIR, STONE, AIR)));
        assert!(!rule.test(BlockPos::new(0, 0, 0), world(SAND, WATER, AIR, AIR)));
        assert!(!rule.test(BlockPos::new(0, 0, 0), world(DIRT, AIR, AIR, AIR)));
    }

    #[test]
    fn a_small_dripleaf_takes_plain_soil_only_under_water() {
        let rule = SurviveRule::SmallDripleaf {
            supports: mask_of([SAND]).as_ref().clone(),
            wet_supports: mask_of([DIRT]).as_ref().clone(),
            water: mask_of([WATER]).as_ref().clone(),
        };
        assert!(rule.test(BlockPos::new(0, 0, 0), world(SAND, AIR, AIR, AIR)));
        assert!(!rule.test(BlockPos::new(0, 0, 0), world(DIRT, AIR, AIR, AIR)));
        let flooded = |p: BlockPos| {
            if p == BlockPos::new(0, 0, 0) {
                WATER
            } else {
                DIRT
            }
        };
        assert!(rule.test(BlockPos::new(0, 0, 0), flooded));
    }

    #[test]
    fn a_block_whose_rule_is_one_tag_below_has_no_family_and_reads_its_definition() {
        for block in [
            "minecraft:oak_sapling",
            "minecraft:rose_bush",
            "minecraft:crimson_roots",
        ] {
            assert_eq!(family_of(block), None, "{block}");
        }
        assert_eq!(
            family_of("minecraft:sugar_cane"),
            Some(SurviveFamily::SugarCane)
        );
        assert_eq!(family_of("minecraft:cactus"), Some(SurviveFamily::Cactus));
    }

    #[test]
    fn a_block_that_overrides_nothing_has_no_family_and_takes_the_default() {
        for block in ["minecraft:stone", "minecraft:melon", "minecraft:pumpkin"] {
            assert_eq!(family_of(block), None, "{block}");
        }
    }
}
