use mcrs_minecraft_client_ticks::Attribute;
use mcrs_minecraft_client_ticks::Event;
use mcrs_minecraft_client_ticks::Recording;
use mcrs_voxel::BlockStateId;

use crate::MotionAttributes;
use crate::MotionEffects;
use crate::TestBlocks;

/// The ids of the `movement_speed` modifiers that follow the sprint state of the client: each
/// update while the client sprints has them, and no update while it does not.
pub fn sprint_modifiers(recording: &Recording) -> Vec<String> {
    let mut sprinting = false;
    let mut while_sprinting: Option<Vec<String>> = None;
    let mut otherwise = Vec::new();
    for event in &recording.events {
        match event {
            Event::Sprinting(on) => sprinting = *on,
            Event::Attribute(attribute) if attribute.name == "minecraft:movement_speed" => {
                let ids: Vec<String> = attribute
                    .modifiers
                    .iter()
                    .map(|modifier| modifier.id.clone())
                    .collect();
                match &mut while_sprinting {
                    _ if !sprinting => otherwise.extend(ids),
                    Some(kept) => kept.retain(|id| ids.contains(id)),
                    None => while_sprinting = Some(ids),
                }
            }
            _ => {}
        }
    }
    let mut ids = while_sprinting.unwrap_or_default();
    ids.retain(|id| !otherwise.contains(id));
    ids
}

/// The value of an attribute without the sprint modifiers, which the kernel applies itself from
/// the sprint state: the base plus the additions, times the base multipliers, times each total
/// multiplier.
pub fn set_attribute(attributes: &mut MotionAttributes, attribute: &Attribute, sprint: &[String]) {
    let modifiers = || {
        attribute
            .modifiers
            .iter()
            .filter(|modifier| !sprint.contains(&modifier.id))
    };
    let sum = |operation| -> f64 {
        modifiers()
            .filter(|modifier| modifier.operation == operation)
            .map(|modifier| modifier.amount)
            .sum()
    };
    let total = modifiers()
        .filter(|modifier| modifier.operation == 2)
        .fold(1.0, |product, modifier| product * (1.0 + modifier.amount));
    let value = (attribute.base + sum(0)) * (1.0 + sum(1)) * total;
    let field = match attribute.name.as_str() {
        "minecraft:gravity" => &mut attributes.gravity,
        "minecraft:step_height" => &mut attributes.step_height,
        "minecraft:movement_speed" => &mut attributes.movement_speed,
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

/// Sets the effect that a recorded event names: `level` is `None` when the effect ends.
pub fn set_effect(effects: &mut MotionEffects, name: &str, level: Option<u8>) {
    match name {
        "minecraft:speed" => effects.speed = level,
        "minecraft:slowness" => effects.slowness = level,
        "minecraft:slow_falling" => effects.slow_falling = level.is_some(),
        "minecraft:levitation" => effects.levitation = level,
        "minecraft:jump_boost" => effects.jump_boost = level,
        "minecraft:weaving" => effects.weaving = level.is_some(),
        "minecraft:blindness" => effects.blindness = level.is_some(),
        _ => {}
    }
}

/// The state of the small test table that stands for a recorded block state.
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
