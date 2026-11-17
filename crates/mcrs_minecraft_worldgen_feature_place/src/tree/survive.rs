use crate::holds;
use fixedbitset::FixedBitSet;
use glam::IVec3;
use mcrs_minecraft_chunk::VoxelId;
use mcrs_minecraft_core::Direction;
use mcrs_voxel::BlockPos;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum SurviveFamily {
    Mushroom,
    SnowLayer,
    OnFacingFace,
    OnAnythingBelow,
    OnSturdyFaceBelow,
    SeaPickle,
    SporeBlossom,
    LilyPad,
    SugarCane,
    Cactus,
    Seagrass,
    TallSeagrass,
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
        "snow" => SnowLayer,
        "ice_crystal" => OnFacingFace,
        _ => return None,
    })
}

#[derive(Clone, Debug)]
pub enum SurviveRule {
    SupportedBy {
        offset_y: i32,
        supports: FixedBitSet,
    },
    SupportedByUnless {
        offset_y: i32,
        supports: FixedBitSet,
        blocked: FixedBitSet,
    },
    SugarCane {
        sugar_cane: FixedBitSet,
        supports: FixedBitSet,
        adjacent: FixedBitSet,
    },
    Cactus {
        supports: FixedBitSet,
        blocked: FixedBitSet,
        liquid: FixedBitSet,
    },
    SmallDripleaf {
        supports: FixedBitSet,
        wet_supports: FixedBitSet,
        water: FixedBitSet,
    },
    OnFacingFace {
        base: u16,
        facing: Vec<Direction>,
        sturdy: [FixedBitSet; 6],
    },
}

fn any_horizontal(p: BlockPos, test: impl Fn(BlockPos) -> bool) -> bool {
    Direction::HORIZONTAL.iter().any(|d| test(p + d.normal()))
}

impl SurviveRule {
    pub fn test(&self, state: VoxelId, p: BlockPos, get: impl Fn(BlockPos) -> VoxelId) -> bool {
        match self {
            SurviveRule::OnFacingFace {
                base,
                facing,
                sturdy,
            } => state
                .0
                .checked_sub(*base)
                .and_then(|index| facing.get(usize::from(index)))
                .is_some_and(|facing| {
                    sturdy
                        .get(facing.id())
                        .is_some_and(|mask| holds(mask, get(p - facing.normal())))
                }),
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
    use mcrs_minecraft_worldgen_feature::placer::mask_of;

    const AIR: VoxelId = VoxelId(0);
    const DIRT: VoxelId = VoxelId(1);
    const SAND: VoxelId = VoxelId(2);
    const CACTUS: VoxelId = VoxelId(3);
    const SUGAR_CANE: VoxelId = VoxelId(4);
    const WATER: VoxelId = VoxelId(5);
    const STONE: VoxelId = VoxelId(6);
    const LEAVES: VoxelId = VoxelId(7);

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
    #[expect(
        clippy::too_many_lines,
        reason = "one case per survive rule, each against the support the reference reads for it"
    )]
    fn each_rule_reads_its_support_where_the_reference_does() {
        let hanging_propagule = SurviveRule::SupportedBy {
            offset_y: 1,
            supports: mask_of([LEAVES]).as_ref().clone(),
        };
        let lily_pad = SurviveRule::SupportedByUnless {
            offset_y: -1,
            supports: mask_of([WATER]).as_ref().clone(),
            blocked: mask_of([WATER]).as_ref().clone(),
        };
        let sugar_cane = SurviveRule::SugarCane {
            sugar_cane: mask_of([SUGAR_CANE]).as_ref().clone(),
            supports: mask_of([SAND]).as_ref().clone(),
            adjacent: mask_of([WATER]).as_ref().clone(),
        };
        let cactus = SurviveRule::Cactus {
            supports: mask_of([SAND, CACTUS]).as_ref().clone(),
            blocked: mask_of([STONE, SAND]).as_ref().clone(),
            liquid: mask_of([WATER]).as_ref().clone(),
        };
        let small_dripleaf = SurviveRule::SmallDripleaf {
            supports: mask_of([SAND]).as_ref().clone(),
            wet_supports: mask_of([DIRT]).as_ref().clone(),
            water: mask_of([WATER]).as_ref().clone(),
        };
        let submerged = |p: BlockPos| if p.y <= 0 { WATER } else { AIR };
        let flooded = |p: BlockPos| {
            if p == BlockPos::new(0, 0, 0) {
                WATER
            } else {
                DIRT
            }
        };

        type World<'a> = &'a dyn Fn(BlockPos) -> VoxelId;
        let cases: &[(&str, &SurviveRule, World, bool)] = &[
            (
                "propagule under leaves",
                &hanging_propagule,
                &world(DIRT, LEAVES, AIR, AIR),
                true,
            ),
            (
                "propagule over leaves",
                &hanging_propagule,
                &world(LEAVES, AIR, AIR, AIR),
                false,
            ),
            (
                "lily pad on water",
                &lily_pad,
                &world(WATER, AIR, AIR, AIR),
                true,
            ),
            (
                "lily pad on dirt",
                &lily_pad,
                &world(DIRT, AIR, AIR, AIR),
                false,
            ),
            ("submerged lily pad", &lily_pad, &submerged, false),
            (
                "cane on cane",
                &sugar_cane,
                &world(SUGAR_CANE, AIR, AIR, AIR),
                true,
            ),
            (
                "cane beside water",
                &sugar_cane,
                &world(SAND, AIR, WATER, AIR),
                false,
            ),
            (
                "cane on sand beside water",
                &sugar_cane,
                &world(SAND, AIR, AIR, WATER),
                true,
            ),
            (
                "cane on stone beside water",
                &sugar_cane,
                &world(STONE, AIR, AIR, WATER),
                false,
            ),
            ("cactus on sand", &cactus, &world(SAND, AIR, AIR, AIR), true),
            (
                "cactus on cactus",
                &cactus,
                &world(CACTUS, AIR, AIR, AIR),
                true,
            ),
            (
                "cactus beside stone",
                &cactus,
                &world(SAND, AIR, STONE, AIR),
                false,
            ),
            (
                "cactus under water",
                &cactus,
                &world(SAND, WATER, AIR, AIR),
                false,
            ),
            (
                "cactus on dirt",
                &cactus,
                &world(DIRT, AIR, AIR, AIR),
                false,
            ),
            (
                "dripleaf on sand",
                &small_dripleaf,
                &world(SAND, AIR, AIR, AIR),
                true,
            ),
            (
                "dry dripleaf on dirt",
                &small_dripleaf,
                &world(DIRT, AIR, AIR, AIR),
                false,
            ),
            ("flooded dripleaf on dirt", &small_dripleaf, &flooded, true),
        ];
        for (name, rule, world, survives) in cases {
            assert_eq!(
                rule.test(AIR, BlockPos::new(0, 0, 0), world),
                *survives,
                "{name}"
            );
        }
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
}
