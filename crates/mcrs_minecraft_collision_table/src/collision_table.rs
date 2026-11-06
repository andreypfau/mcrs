use glam::DVec3;
use mcrs_minecraft_block::block_keys::Block;
use mcrs_minecraft_block::block_tag_keys;
use mcrs_minecraft_block::definition::BlockDefinitions;
use mcrs_minecraft_block::definition::schema::PropertyValue;
use mcrs_minecraft_physics::Aabb;
use mcrs_minecraft_physics::BlockFlags;
use mcrs_minecraft_physics::BlockMotion;
use mcrs_minecraft_physics::CollisionTable;
use mcrs_minecraft_physics::CollisionTableBuilder;
use mcrs_minecraft_physics::ContextShape;
use mcrs_minecraft_physics::StateSpec;
use mcrs_minecraft_registries::Tags;
use mcrs_minecraft_resource::TagKey;

use crate::named_block::NamedBlock;
use crate::widen::on_pixel_grid;
use crate::widen::widen;

/// The block tags that set a flag.
const TAG_FLAGS: [(TagKey<'static, Block>, BlockFlags); 12] = [
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
    (block_tag_keys::CROPS, BlockFlags::INSIDE_ACTION),
    (block_tag_keys::CAULDRONS, BlockFlags::INSIDE_ACTION),
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
    let fire = tags.id(block_tag_keys::FIRE);
    let mut builder = CollisionTableBuilder::new();
    let mut boxes = Vec::new();
    for id in definitions.states() {
        let state = definitions.state(id);
        let owner = definitions.owner(id);
        let block = definitions.block_index(id);
        let named = owner.map_or(NamedBlock::of(""), |entry| {
            NamedBlock::of(entry.identifier.as_str())
        });
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
        if in_tag(fire) {
            flags |= BlockFlags::INSIDE_ACTION;
        }
        if state.fluid.is_some() {
            flags |= BlockFlags::FLUID | BlockFlags::COARSE_NEAR;
        }
        boxes.clear();
        boxes.extend(
            definitions
                .shape(state.collision_shape)
                .iter()
                .map(|bounds| {
                    Aabb::new(
                        DVec3::new(
                            widen(bounds.min.x),
                            widen(bounds.min.y),
                            widen(bounds.min.z),
                        ),
                        DVec3::new(
                            widen(bounds.max.x),
                            widen(bounds.max.y),
                            widen(bounds.max.z),
                        ),
                    )
                }),
        );
        if boxes.iter().any(|bounds| {
            !bounds
                .min
                .to_array()
                .into_iter()
                .chain(bounds.max.to_array())
                .all(on_pixel_grid)
        }) {
            flags |= BlockFlags::OFF_GRID;
        }
        let name = owner.map_or("", |entry| entry.identifier.as_str());
        let context = match name {
            "minecraft:scaffolding" => {
                let distance = owner.and_then(|entry| entry.value_of(id, "distance"));
                let bottom = owner.and_then(|entry| entry.value_of(id, "bottom"));
                Some(ContextShape::Scaffolding {
                    bottom: !matches!(distance, Some(PropertyValue::Int(0)))
                        && matches!(bottom, Some(PropertyValue::Bool(true))),
                })
            }
            "minecraft:powder_snow" => Some(ContextShape::PowderSnow),
            _ => None,
        };
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
            context,
            flags,
            inside: named.inside,
            motion,
        });
    }
    builder.build()
}
