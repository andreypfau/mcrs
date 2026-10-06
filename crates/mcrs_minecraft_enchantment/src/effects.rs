use std::collections::BTreeMap;
use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use super::predicate::{LootCondition, dispatched_map};
use super::value::LevelBasedValue;
use mcrs_minecraft_block_predicate::predicate::BlockPredicate;
use mcrs_minecraft_core::value_provider::FloatProvider;
use mcrs_minecraft_keys as keys;
use mcrs_minecraft_particle::ParticleOptions;
use mcrs_minecraft_registry::{HolderSet, Id};

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

const ENCHANTMENT_VALUE_EFFECT_TYPE_ROWS: &[&str] = &[
    "minecraft:set",
    "minecraft:add",
    "minecraft:multiply",
    "minecraft:remove_binomial",
    "minecraft:all_of",
];

const ENCHANTMENT_VALUE_EFFECT_TYPE_UNSUPPORTED: &[&str] = &["minecraft:exponential"];

const _: () = assert!(mcrs_minecraft_registry::static_rows::names_cover(
    ENCHANTMENT_VALUE_EFFECT_TYPE_ROWS,
    ENCHANTMENT_VALUE_EFFECT_TYPE_UNSUPPORTED,
    crate::keys::EnchantmentValueEffectType::ENTRIES
));

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

/// A bare id names a `worldgen/block_state_provider` entry; an object is a
/// block state or a typed provider.
#[derive(Debug, Clone, PartialEq)]
pub enum BlockStateProvider {
    Reference(Id<mcrs_minecraft_block_predicate::provider::DirectBlockStateProvider>),
    State(FullBlockState),
    Typed(TypedBlockStateProvider),
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct FullBlockState {
    pub id: Id<keys::Block>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub properties: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlockState {
    Block(Id<keys::Block>),
    Full(FullBlockState),
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(tag = "type", deny_unknown_fields)]
pub enum TypedBlockStateProvider {
    #[serde(rename = "minecraft:simple")]
    Simple { state: BlockState },
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderObject {
    #[serde(rename = "type")]
    kind: Option<String>,
    id: Option<Id<keys::Block>>,
    properties: Option<BTreeMap<String, String>>,
    state: Option<BlockState>,
}

impl ProviderObject {
    fn into_provider(self) -> Result<BlockStateProvider, String> {
        match self {
            ProviderObject {
                kind: None,
                id: Some(id),
                properties,
                state: None,
            } => Ok(BlockStateProvider::State(FullBlockState { id, properties })),
            ProviderObject {
                kind: Some(kind),
                id: None,
                properties: None,
                state: Some(state),
            } if kind == mcrs_minecraft_block_predicate::keys::BlockStateProviderType::Simple.as_static_str() => Ok(BlockStateProvider::Typed(
                TypedBlockStateProvider::Simple { state },
            )),
            ProviderObject {
                kind: Some(kind), ..
            } if kind != mcrs_minecraft_block_predicate::keys::BlockStateProviderType::Simple.as_static_str() => {
                Err(format!("unknown block state provider type `{kind}`"))
            }
            _ => Err("a block state states an `id` and its `properties`; a provider states a `type` and its fields".to_owned()),
        }
    }
}

impl Serialize for BlockStateProvider {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            BlockStateProvider::Reference(id) => id.serialize(s),
            BlockStateProvider::State(state) => state.serialize(s),
            BlockStateProvider::Typed(typed) => typed.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for BlockStateProvider {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct ProviderVisitor;

        impl<'de> Visitor<'de> for ProviderVisitor {
            type Value = BlockStateProvider;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a block state provider id, a block state or a typed provider")
            }

            fn visit_str<E: de::Error>(self, text: &str) -> Result<Self::Value, E> {
                Id::deserialize(de::value::StrDeserializer::new(text))
                    .map(BlockStateProvider::Reference)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                ProviderObject::deserialize(de::value::MapAccessDeserializer::new(map))?
                    .into_provider()
                    .map_err(de::Error::custom)
            }
        }

        d.deserialize_any(ProviderVisitor)
    }
}

impl Serialize for BlockState {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        match self {
            BlockState::Block(id) => id.serialize(s),
            BlockState::Full(state) => state.serialize(s),
        }
    }
}

impl<'de> Deserialize<'de> for BlockState {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct StateVisitor;

        impl<'de> Visitor<'de> for StateVisitor {
            type Value = BlockState;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a block name or a block state")
            }

            fn visit_str<E: de::Error>(self, text: &str) -> Result<Self::Value, E> {
                Id::deserialize(de::value::StrDeserializer::new(text)).map(BlockState::Block)
            }

            fn visit_map<A: MapAccess<'de>>(self, map: A) -> Result<Self::Value, A::Error> {
                FullBlockState::deserialize(de::value::MapAccessDeserializer::new(map))
                    .map(BlockState::Full)
            }
        }

        d.deserialize_any(StateVisitor)
    }
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
        to_apply: HolderSet<mcrs_minecraft_item::keys::MobEffect>,
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
        immune_blocks: Option<HolderSet<keys::Block>>,
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
        sound: HolderSet<mcrs_minecraft_sound::SoundEvent>,
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
        entity: HolderSet<mcrs_minecraft_entity::keys::EntityType>,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        join_team: Option<bool>,
    },
}

const ENCHANTMENT_LOCATION_BASED_EFFECT_TYPE_ROWS: &[&str] = &[
    "minecraft:all_of",
    "minecraft:attribute",
    "minecraft:apply_mob_effect",
    "minecraft:change_item_damage",
    "minecraft:damage_entity",
    "minecraft:explode",
    "minecraft:ignite",
    "minecraft:apply_impulse",
    "minecraft:apply_exhaustion",
    "minecraft:play_sound",
    "minecraft:replace_disk",
    "minecraft:spawn_particles",
    "minecraft:summon_entity",
];

const ENCHANTMENT_LOCATION_BASED_EFFECT_TYPE_UNSUPPORTED: &[&str] = &[
    "minecraft:replace_block",
    "minecraft:run_function",
    "minecraft:set_block_properties",
];

const _: () = assert!(mcrs_minecraft_registry::static_rows::names_cover(
    ENCHANTMENT_LOCATION_BASED_EFFECT_TYPE_ROWS,
    ENCHANTMENT_LOCATION_BASED_EFFECT_TYPE_UNSUPPORTED,
    crate::keys::EnchantmentLocationBasedEffectType::ENTRIES
));

const ENCHANTMENT_ENTITY_EFFECT_TYPE_ROWS: &[&str] = &[
    "minecraft:all_of",
    "minecraft:apply_mob_effect",
    "minecraft:change_item_damage",
    "minecraft:damage_entity",
    "minecraft:explode",
    "minecraft:ignite",
    "minecraft:apply_impulse",
    "minecraft:apply_exhaustion",
    "minecraft:play_sound",
    "minecraft:replace_disk",
    "minecraft:spawn_particles",
    "minecraft:summon_entity",
];

const ENCHANTMENT_ENTITY_EFFECT_TYPE_UNSUPPORTED: &[&str] = &[
    "minecraft:replace_block",
    "minecraft:run_function",
    "minecraft:set_block_properties",
];

const _: () = assert!(mcrs_minecraft_registry::static_rows::names_cover(
    ENCHANTMENT_ENTITY_EFFECT_TYPE_ROWS,
    ENCHANTMENT_ENTITY_EFFECT_TYPE_UNSUPPORTED,
    crate::keys::EnchantmentEntityEffectType::ENTRIES
));

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

const _: () = assert!(mcrs_minecraft_registry::static_rows::names_cover(
    EnchantmentEffects::KEYS,
    &[],
    crate::keys::EnchantmentEffectComponentType::ENTRIES
));

#[cfg(test)]
mod dispatch_rows {
    use super::*;

    #[test]
    fn enchantment_value_effect_type_rows_select_their_variants() {
        mcrs_minecraft_registry::static_rows::assert_dispatch::<EnchantmentValueEffect>(
            ENCHANTMENT_VALUE_EFFECT_TYPE_ROWS,
            ENCHANTMENT_VALUE_EFFECT_TYPE_UNSUPPORTED,
            crate::keys::EnchantmentValueEffectType::ENTRIES,
            |name| serde_json::json!({ "type": name }),
        );
    }

    #[test]
    fn enchantment_entity_effect_type_rows_select_their_variants() {
        mcrs_minecraft_registry::static_rows::assert_dispatch::<EnchantmentEntityEffect>(
            ENCHANTMENT_ENTITY_EFFECT_TYPE_ROWS,
            ENCHANTMENT_ENTITY_EFFECT_TYPE_UNSUPPORTED,
            crate::keys::EnchantmentEntityEffectType::ENTRIES,
            |name| serde_json::json!({ "type": name }),
        );
    }

    #[test]
    fn enchantment_location_based_effect_type_rows_select_their_variants() {
        mcrs_minecraft_registry::static_rows::assert_dispatch::<EnchantmentEntityEffect>(
            ENCHANTMENT_LOCATION_BASED_EFFECT_TYPE_ROWS,
            ENCHANTMENT_LOCATION_BASED_EFFECT_TYPE_UNSUPPORTED,
            crate::keys::EnchantmentLocationBasedEffectType::ENTRIES,
            |name| serde_json::json!({ "type": name }),
        );
    }
}
