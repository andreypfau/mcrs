use mcrs_minecraft_client_ticks::Attribute;
use mcrs_minecraft_entity_attribute::AttributeModifier;
use mcrs_minecraft_entity_attribute::Modifier;
use mcrs_minecraft_entity_attribute::ModifierOperation;
use mcrs_minecraft_entity_attribute::ValueRange;
use mcrs_minecraft_entity_attribute::fold;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_voxel::BlockStateId;

use crate::MotionAttributes;
use crate::MotionEffects;
use crate::SprintSpeed;
use crate::TestBlocks;
use crate::movement_speeds::movement_speeds;

const MOVEMENT_SPEED_RANGE: ValueRange = ValueRange {
    min: 0.0,
    max: 1024.0,
};

pub fn set_attribute(
    attributes: &mut MotionAttributes,
    sprint_speed: &mut SprintSpeed,
    attribute: &Attribute,
) {
    let modifiers = attribute.modifiers.iter().filter_map(|modifier| {
        Some(AttributeModifier {
            id: ResourceLocation::read(&modifier.id).ok()?,
            amount: modifier.amount,
            operation: ModifierOperation::try_from(modifier.operation).ok()?,
        })
    });
    if attribute.name == "minecraft:movement_speed" {
        let speeds = movement_speeds(attribute.base, modifiers, MOVEMENT_SPEED_RANGE);
        attributes.movement_speed = speeds.walking;
        *sprint_speed = SprintSpeed(speeds.sprinting);
        return;
    }
    let value = fold(attribute.base, modifiers.map(Modifier::from));
    let field = match attribute.name.as_str() {
        "minecraft:gravity" => &mut attributes.gravity,
        "minecraft:step_height" => &mut attributes.step_height,
        "minecraft:jump_strength" => &mut attributes.jump_strength,
        "minecraft:scale" => &mut attributes.scale,
        "minecraft:sneaking_speed" => &mut attributes.sneaking_speed,
        "minecraft:movement_efficiency" => &mut attributes.movement_efficiency,
        "minecraft:bounciness" => &mut attributes.bounciness,
        "minecraft:friction_modifier" => &mut attributes.friction_modifier,
        "minecraft:air_drag_modifier" => &mut attributes.air_drag_modifier,
        _ => return,
    };
    *field = value;
}

pub fn set_effect(effects: &mut MotionEffects, name: &str, level: Option<u8>) {
    match name {
        "minecraft:slow_falling" => effects.slow_falling = level.is_some(),
        "minecraft:levitation" => effects.levitation = level,
        "minecraft:jump_boost" => effects.jump_boost = level,
        "minecraft:weaving" => effects.weaving = level.is_some(),
        "minecraft:blindness" => effects.blindness = level.is_some(),
        _ => {}
    }
}

pub fn test_state(blocks: &TestBlocks, state: &str) -> Result<BlockStateId, String> {
    let (block, properties) = state.split_once('[').unwrap_or((state, ""));
    let has = |property: &str| properties.contains(property);
    Ok(match block {
        "minecraft:air" => BlockStateId::AIR,
        "minecraft:stone" => blocks.stone,
        "minecraft:ice" => blocks.ice,
        "minecraft:slime_block" => blocks.slime,
        "minecraft:soul_sand" => blocks.soul_sand,
        "minecraft:honey_block" => blocks.honey,
        "minecraft:cobweb" => blocks.cobweb,
        "minecraft:sweet_berry_bush" => blocks.sweet_berry_bush,
        "minecraft:powder_snow" => blocks.powder_snow,
        "minecraft:water" => blocks.water,
        _ if block.ends_with("_slab") && has("type=bottom") => blocks.bottom_slab,
        _ if block.ends_with("_slab") && has("type=top") => blocks.top_slab,
        _ if block.ends_with("_carpet") => blocks.carpet,
        _ => return Err(format!("the test table has no state for {state}")),
    })
}
