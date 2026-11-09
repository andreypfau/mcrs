use glam::DVec3;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::block_tag_keys;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_block::definition::BlockStateFlags;
use mcrs_minecraft_block::definition::schema::PropertyValue;
use mcrs_minecraft_block::fluid_keys::Fluid;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_physics::BlockFlags;
use mcrs_minecraft_physics::BlockMotion;
use mcrs_minecraft_physics::CollisionTable;
use mcrs_minecraft_physics::CollisionTableBuilder;
use mcrs_minecraft_physics::ContextShape;
use mcrs_minecraft_physics::StateSpec;
use mcrs_minecraft_registries::Tags;
use mcrs_minecraft_resource::TagKey;
use mcrs_voxel::BlockStateId;

use crate::inside_shape::inside_shape;
use crate::named_block::NamedBlock;
use crate::path_types::PathTypes;

/// The block tags that set a flag.
const TAG_FLAGS: [(TagKey<'static, Block>, BlockFlags); 12] = [
    (block_tag_keys::DOORS, BlockFlags::DOOR),
    (block_tag_keys::CLIMBABLE, BlockFlags::CLIMBABLE),
    (block_tag_keys::FENCES, BlockFlags::FENCE),
    (block_tag_keys::WALLS, BlockFlags::WALL),
    (block_tag_keys::FENCE_GATES, BlockFlags::FENCE_GATE),
    (
        block_tag_keys::SUPPRESSES_BOUNCE,
        BlockFlags::SUPPRESSES_BOUNCE,
    ),
    (
        block_tag_keys::FALL_DAMAGE_RESETTING,
        BlockFlags::FALL_DAMAGE_RESETTING,
    ),
    (
        block_tag_keys::CAN_GLIDE_THROUGH,
        BlockFlags::CAN_GLIDE_THROUGH,
    ),
    (block_tag_keys::SHULKER_BOXES, BlockFlags::COARSE_NEAR),
    (block_tag_keys::PRESSURE_PLATES, BlockFlags::INSIDE_ACTION),
    (block_tag_keys::BUTTONS, BlockFlags::INSIDE_ACTION),
    (block_tag_keys::FIRE, BlockFlags::INSIDE_ACTION),
];

/// The bounce restitution of each bed.
const BED_BOUNCE: f32 = 0.75;

/// The collision table of every state of the definitions, in the order of the state ids.
#[must_use]
pub fn collision_table(definitions: &BlockDefinitions, tags: &Tags<Block>) -> CollisionTable {
    let tag_flags: Vec<_> = TAG_FLAGS
        .iter()
        .filter_map(|(key, flag)| Some((tags.id(*key)?, *flag)))
        .collect();
    let beds = tags.id(block_tag_keys::BEDS);
    let causes_suffocation = tags.id(block_tag_keys::CAUSES_SUFFOCATION);
    let leaves = tags.id(block_tag_keys::LEAVES);
    let shulker_boxes = tags.id(block_tag_keys::SHULKER_BOXES);
    let path_types = PathTypes::new(definitions, tags);
    let mut builder = CollisionTableBuilder::new();
    let mut boxes = Vec::new();
    let cauldron: Vec<Aabb> = definitions
        .default_state("minecraft:cauldron")
        .map(|id| boxes_of(definitions, id).collect())
        .unwrap_or_default();
    let mut inside_boxes = Vec::new();
    for id in definitions.states() {
        let state = definitions.state(id);
        let owner = definitions.owner(id);
        let block = definitions.block_index(id);
        let name = owner.map_or("", |entry| entry.identifier.as_str());
        let path = name.strip_prefix("minecraft:").unwrap_or(name);
        let value_of = |property: &str| owner.and_then(|entry| entry.value_of(id, property));
        let named = NamedBlock::of(path);
        let in_tag = |tag| {
            block
                .zip(tag)
                .is_some_and(|(block, tag)| tags.contains(tag, block))
        };
        let mut flags = named.flags;
        for (tag, flag) in &tag_flags {
            if in_tag(Some(*tag)) {
                flags |= *flag;
            }
        }
        if flags.contains(BlockFlags::FALL_DAMAGE_RESETTING) {
            flags |= BlockFlags::INSIDE_ACTION;
        }
        if let Some(fluid) = state.fluid {
            flags |= BlockFlags::FLUID | BlockFlags::COARSE_NEAR;
            if fluid.fluid == Fluid::Water.id() {
                flags |= BlockFlags::WATER;
            }
        }
        if state.flags.contains(BlockStateFlags::IS_SOLID_RENDER) {
            flags |= BlockFlags::SOLID_RENDER;
        }
        if state.flags.contains(BlockStateFlags::LEGACY_SOLID) {
            flags |= BlockFlags::LEGACY_SOLID;
        }
        boxes.clear();
        boxes.extend(boxes_of(definitions, id));
        if boxes.iter().any(|bounds| {
            !bounds
                .min
                .to_array()
                .into_iter()
                .chain(bounds.max.to_array())
                .all(|value| (value * 16.0).fract() == 0.0)
        }) {
            flags |= BlockFlags::OFF_GRID;
        }
        let by_tag = || {
            !in_tag(leaves)
                && !in_tag(shulker_boxes)
                && in_tag(causes_suffocation)
                && is_full_cube(&boxes)
        };
        if suffocates(path, value_of("extended"), by_tag) {
            flags |= BlockFlags::SUFFOCATES;
        }
        let bounce = if in_tag(beds) {
            BED_BOUNCE
        } else {
            named.bounce
        };
        let motion = BlockMotion {
            friction: state.friction,
            speed_factor: named.speed_factor,
            jump_factor: named.jump_factor,
            bounce,
        };
        let _ = builder.push(StateSpec {
            boxes: &boxes,
            context: context_shape(path, value_of),
            flags,
            inside: named.inside,
            inside_shape: inside_shape(path, value_of, &cauldron, &mut inside_boxes),
            motion,
            path: path_types.of(definitions, id, &boxes),
        });
    }
    builder.build()
}

/// The collision boxes of a state, widened.
fn boxes_of(definitions: &BlockDefinitions, id: BlockStateId) -> impl Iterator<Item = Aabb> {
    definitions
        .shape(definitions.state(id).collision_shape)
        .iter()
        .map(|bounds| Aabb::new(bounds.min.as_dvec3(), bounds.max.as_dvec3()))
}

fn context_shape<'a>(
    path: &str,
    value_of: impl Fn(&str) -> Option<&'a PropertyValue>,
) -> Option<ContextShape> {
    match path {
        "scaffolding" => Some(ContextShape::Scaffolding {
            bottom: !matches!(value_of("distance"), Some(PropertyValue::Int(0)))
                && matches!(value_of("bottom"), Some(PropertyValue::Bool(true))),
        }),
        "powder_snow" => Some(ContextShape::PowderSnow),
        _ => None,
    }
}

/// The table of PHY-INP-021; `by_tag` decides for a block that the table does not name. A
/// shulker box never counts: the table does not know whether it is closed, and the box makes the
/// tick coarse.
fn suffocates(path: &str, extended: Option<&PropertyValue>, by_tag: impl FnOnce() -> bool) -> bool {
    match path {
        "mangrove_roots" | "glass" | "tinted_glass" | "moving_piston" => false,
        _ if path.ends_with("_stained_glass") || path.ends_with("copper_grate") => false,
        "farmland" | "soul_sand" | "dirt_path" | "mud" => true,
        "piston" | "sticky_piston" => !matches!(extended, Some(PropertyValue::Bool(true))),
        _ => by_tag(),
    }
}

fn is_full_cube(boxes: &[Aabb]) -> bool {
    matches!(boxes, [only] if only.min == DVec3::ZERO && only.max == DVec3::ONE)
}
