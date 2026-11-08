use mcrs_minecraft_physics::BlockFlags;
use mcrs_minecraft_physics::BlockMotion;
use mcrs_minecraft_physics::InsideEffect;

/// What the physics model states for a block by its name, apart from its tags and the corpus.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct NamedBlock {
    pub(crate) flags: BlockFlags,
    pub(crate) inside: InsideEffect,
    pub(crate) speed_factor: f32,
    pub(crate) jump_factor: f32,
    pub(crate) bounce: f32,
}

impl NamedBlock {
    /// The values of a block by its name without the namespace.
    pub(crate) fn of(path: &str) -> Self {
        let mut named = Self {
            flags: BlockFlags::empty(),
            inside: InsideEffect::None,
            speed_factor: BlockMotion::DEFAULT.speed_factor,
            jump_factor: BlockMotion::DEFAULT.jump_factor,
            bounce: BlockMotion::DEFAULT.bounce,
        };
        match path {
            "slime_block" => {
                named.bounce = 1.0;
                named.flags |= BlockFlags::SLIME;
            }
            "soul_sand" => named.speed_factor = 0.4,
            "honey_block" => {
                named.speed_factor = 0.4;
                named.jump_factor = 0.5;
                named.inside = InsideEffect::Honey;
            }
            "shelf_mushroom" => named.bounce = 0.75,
            "cobweb" => named.inside = InsideEffect::Cobweb,
            "sweet_berry_bush" => named.inside = InsideEffect::SweetBerryBush,
            "powder_snow" => named.inside = InsideEffect::PowderSnow,
            "water" => named.flags |= BlockFlags::OWN_SPEED_FACTOR,
            "bubble_column" => {
                named.flags |= BlockFlags::OWN_SPEED_FACTOR | BlockFlags::COARSE_BUBBLE_COLUMN;
            }
            "potent_sulfur" => named.flags |= BlockFlags::COARSE_POTENT_SULFUR,
            "moving_piston" => named.flags |= BlockFlags::MOVING_PISTON | BlockFlags::COARSE_PISTON,
            "piston" | "sticky_piston" | "piston_head" => named.flags |= BlockFlags::COARSE_PISTON,
            "nether_portal" | "end_portal" | "end_gateway" | "big_dripleaf" | "bamboo"
            | "pointed_dripstone" | "sulfur_spike" | "icicle" => {
                named.flags |= BlockFlags::COARSE_NEAR
            }
            _ => {}
        }
        if STEP_ACTIONS.contains(&path) {
            named.flags |= BlockFlags::STEP_ACTION;
        }
        if INSIDE_ACTIONS.contains(&path) || named.inside != InsideEffect::None {
            named.flags |= BlockFlags::INSIDE_ACTION;
        }
        named
    }
}

const STEP_ACTIONS: [&str; 9] = [
    "magma_block",
    "redstone_ore",
    "deepslate_redstone_ore",
    "turtle_egg",
    "sculk_sensor",
    "calibrated_sculk_sensor",
    "sculk_shrieker",
    "brittle_ice",
    "slime_block",
];

/// The blocks with an inside action that are not in a tag of [`crate::collision_table`] and have
/// no [`InsideEffect`].
const INSIDE_ACTIONS: [&str; 25] = [
    "tripwire",
    "detector_rail",
    "wheat",
    "carrots",
    "potatoes",
    "beetroots",
    "torchflower_crop",
    "pitcher_crop",
    "campfire",
    "soul_campfire",
    "water_cauldron",
    "powder_snow_cauldron",
    "lava_cauldron",
    "cactus",
    "bubble_column",
    "wither_rose",
    "open_eyeblossom",
    "closed_eyeblossom",
    "big_dripleaf",
    "lily_pad",
    "frogspawn",
    "hopper",
    "nether_portal",
    "end_portal",
    "end_gateway",
];
