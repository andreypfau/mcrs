use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::predicate::{BlockPredicate, LootCondition, dispatched_map};
use super::value::{FloatProvider, HolderSet, LevelBasedValue};

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConditionalEffect<T> {
    pub effect: T,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirements: Option<LootCondition>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EnchantmentTarget {
    Attacker,
    DamagingEntity,
    Victim,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct TargetedConditionalEffect<T> {
    pub enchanted: EnchantmentTarget,
    /// `equipment_drops` fixes the affected target at `victim` and leaves the
    /// field unwritten; every other component states both.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub affected: Option<EnchantmentTarget>,
    pub effect: T,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirements: Option<LootCondition>,
}

/// Java's `Unit`: the component's presence is the whole statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Unit {}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum EnchantmentValueEffect {
    #[serde(rename = "minecraft:set")]
    Set { value: LevelBasedValue },
    #[serde(rename = "minecraft:add")]
    Add { value: LevelBasedValue },
    #[serde(rename = "minecraft:multiply")]
    Multiply { factor: LevelBasedValue },
    #[serde(rename = "minecraft:remove_binomial")]
    RemoveBinomial { chance: LevelBasedValue },
    #[serde(rename = "minecraft:all_of")]
    AllOf {
        effects: Vec<EnchantmentValueEffect>,
    },
}

impl EnchantmentValueEffect {
    /// Java's `EnchantmentValueEffect.process`. `binomial` draws the removals
    /// `RemoveBinomial` needs; every other variant ignores it.
    pub fn process(
        &self,
        level: i32,
        input: f32,
        binomial: &mut dyn FnMut(f32, f32) -> f32,
    ) -> f32 {
        match self {
            EnchantmentValueEffect::Set { value } => value.calculate(level),
            EnchantmentValueEffect::Add { value } => input + value.calculate(level),
            EnchantmentValueEffect::Multiply { factor } => input * factor.calculate(level),
            EnchantmentValueEffect::RemoveBinomial { chance } => {
                binomial(input, chance.calculate(level))
            }
            EnchantmentValueEffect::AllOf { effects } => {
                let mut value = input;
                for effect in effects {
                    value = effect.process(level, value, binomial);
                }
                value
            }
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum ExplosionInteraction {
    None,
    Block,
    Mob,
    Tnt,
    Trigger,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ParticleOptions {
    #[serde(rename = "type")]
    pub particle_type: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum PositionSourceType {
    EntityPosition,
    InBoundingBox,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PositionSource {
    #[serde(rename = "type")]
    pub source: PositionSourceType,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub offset: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub scale: Option<f32>,
}

#[derive(Debug, Clone, Copy, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct VelocitySource {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub movement_scale: Option<f32>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub base: Option<FloatProvider>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum BlockStateProvider {
    #[serde(rename = "minecraft:simple_state_provider")]
    Simple { state: String },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum AttributeOperation {
    AddValue,
    AddMultipliedBase,
    AddMultipliedTotal,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnchantmentAttributeEffect {
    pub id: String,
    pub attribute: String,
    pub amount: LevelBasedValue,
    pub operation: AttributeOperation,
}

/// Java keeps two registries here, one for entity effects and one for
/// location-based effects; they differ only in that the location one also
/// accepts `attribute`. One enum carries both.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum EnchantmentEntityEffect {
    #[serde(rename = "minecraft:all_of")]
    AllOf {
        effects: Vec<EnchantmentEntityEffect>,
    },
    #[serde(rename = "minecraft:attribute")]
    Attribute {
        id: String,
        attribute: String,
        amount: LevelBasedValue,
        operation: AttributeOperation,
    },
    #[serde(rename = "minecraft:apply_mob_effect")]
    ApplyMobEffect {
        to_apply: HolderSet,
        min_duration: LevelBasedValue,
        max_duration: LevelBasedValue,
        min_amplifier: LevelBasedValue,
        max_amplifier: LevelBasedValue,
    },
    #[serde(rename = "minecraft:change_item_damage")]
    ChangeItemDamage { amount: LevelBasedValue },
    #[serde(rename = "minecraft:damage_entity")]
    DamageEntity {
        min_damage: LevelBasedValue,
        max_damage: LevelBasedValue,
        damage_type: String,
    },
    #[serde(rename = "minecraft:explode")]
    Explode {
        #[serde(default, skip_serializing_if = "Option::is_none")]
        attribute_to_user: Option<bool>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        damage_type: Option<String>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        knockback_multiplier: Option<LevelBasedValue>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        immune_blocks: Option<HolderSet>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        offset: Option<[f64; 3]>,
        radius: LevelBasedValue,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        create_fire: Option<bool>,
        block_interaction: ExplosionInteraction,
        small_particle: ParticleOptions,
        large_particle: ParticleOptions,
        sound: String,
    },
    #[serde(rename = "minecraft:ignite")]
    Ignite { duration: LevelBasedValue },
    #[serde(rename = "minecraft:apply_impulse")]
    ApplyImpulse {
        direction: [f64; 3],
        coordinate_scale: [f64; 3],
        magnitude: LevelBasedValue,
    },
    #[serde(rename = "minecraft:apply_exhaustion")]
    ApplyExhaustion { amount: LevelBasedValue },
    #[serde(rename = "minecraft:play_sound")]
    PlaySound {
        sound: HolderSet,
        volume: FloatProvider,
        pitch: FloatProvider,
    },
    #[serde(rename = "minecraft:replace_disk")]
    ReplaceDisk {
        radius: LevelBasedValue,
        height: LevelBasedValue,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        offset: Option<[i32; 3]>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        predicate: Option<BlockPredicate>,
        block_state: BlockStateProvider,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        trigger_game_event: Option<String>,
    },
    #[serde(rename = "minecraft:spawn_particles")]
    SpawnParticles {
        particle: ParticleOptions,
        horizontal_position: PositionSource,
        vertical_position: PositionSource,
        horizontal_velocity: VelocitySource,
        vertical_velocity: VelocitySource,
        speed: FloatProvider,
    },
    #[serde(rename = "minecraft:summon_entity")]
    SummonEntity {
        entity: HolderSet,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        join_team: Option<bool>,
    },
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ChargingSounds {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub start: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub mid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub end: Option<String>,
}

dispatched_map! {
    /// Java's `EnchantmentEffectComponents`: the key names the component and so
    /// chooses the type of its value.
    EnchantmentEffects {
        "minecraft:damage_protection" => damage_protection: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        "minecraft:damage_immunity" => damage_immunity: Vec<ConditionalEffect<Unit>>,
        "minecraft:damage" => damage: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        "minecraft:smash_damage_per_fallen_block" => smash_damage_per_fallen_block: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        "minecraft:knockback" => knockback: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        "minecraft:armor_effectiveness" => armor_effectiveness: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        "minecraft:post_attack" => post_attack: Vec<TargetedConditionalEffect<EnchantmentEntityEffect>>,
        "minecraft:post_piercing_attack" => post_piercing_attack: Vec<ConditionalEffect<EnchantmentEntityEffect>>,
        "minecraft:hit_block" => hit_block: Vec<ConditionalEffect<EnchantmentEntityEffect>>,
        "minecraft:item_damage" => item_damage: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        "minecraft:equipment_drops" => equipment_drops: Vec<TargetedConditionalEffect<EnchantmentValueEffect>>,
        "minecraft:location_changed" => location_changed: Vec<ConditionalEffect<EnchantmentEntityEffect>>,
        "minecraft:tick" => tick: Vec<ConditionalEffect<EnchantmentEntityEffect>>,
        "minecraft:ammo_use" => ammo_use: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        "minecraft:projectile_piercing" => projectile_piercing: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        "minecraft:projectile_spawned" => projectile_spawned: Vec<ConditionalEffect<EnchantmentEntityEffect>>,
        "minecraft:projectile_spread" => projectile_spread: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        "minecraft:projectile_count" => projectile_count: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        "minecraft:trident_return_acceleration" => trident_return_acceleration: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        "minecraft:fishing_time_reduction" => fishing_time_reduction: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        "minecraft:fishing_luck_bonus" => fishing_luck_bonus: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        "minecraft:block_experience" => block_experience: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        "minecraft:mob_experience" => mob_experience: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        "minecraft:repair_with_xp" => repair_with_xp: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        "minecraft:attributes" => attributes: Vec<EnchantmentAttributeEffect>,
        "minecraft:crossbow_charge_time" => crossbow_charge_time: EnchantmentValueEffect,
        "minecraft:crossbow_charging_sounds" => crossbow_charging_sounds: Vec<ChargingSounds>,
        "minecraft:trident_sound" => trident_sound: Vec<String>,
        "minecraft:prevent_equipment_drop" => prevent_equipment_drop: Unit,
        "minecraft:prevent_armor_change" => prevent_armor_change: Unit,
        "minecraft:trident_spin_attack_strength" => trident_spin_attack_strength: EnchantmentValueEffect,
    }
}
