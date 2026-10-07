use std::collections::BTreeMap;
use std::fmt;

use serde::de::{self, MapAccess, Visitor};
use serde::{Deserialize, Deserializer, Serialize, Serializer};

use mcrs_minecraft_block_predicate::predicate::BlockPredicate;
use mcrs_minecraft_item::AttributeOperation;
use mcrs_minecraft_item::enchantment::value::LevelBasedValue;
use mcrs_minecraft_loot::LootCondition;
use mcrs_minecraft_particle::ParticleOptions;
use mcrs_minecraft_registry::dispatched_map;
use mcrs_minecraft_registry::{Holder, HolderSet, Id};
use mcrs_minecraft_value_provider::FloatProvider;

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConditionalEffect<T> {
    pub effect: T,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub requirements: Option<Holder<LootCondition>>,
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
    pub requirements: Option<Holder<LootCondition>>,
}

/// The component's presence is the whole statement.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Unit {}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub enum EnchantmentValueEffect {
    Set {
        value: LevelBasedValue,
    },
    Add {
        value: LevelBasedValue,
    },
    Multiply {
        factor: LevelBasedValue,
    },
    RemoveBinomial {
        chance: LevelBasedValue,
    },
    AllOf {
        effects: Vec<EnchantmentValueEffect>,
    },
}

mcrs_minecraft_registry::dispatch! {
    EnchantmentValueEffect, key = "type", registry = crate::keys::EnchantmentValueEffectType,
    {
        Add => Add,
        AllOf => AllOf,
        Multiply => Multiply,
        RemoveBinomial => RemoveBinomial,
        Set => Set,
    }
    unsupported { Exponential }
}

impl EnchantmentValueEffect {
    /// `binomial` draws the removals `RemoveBinomial` needs; every other
    /// variant ignores it.
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
    pub id: Id<mcrs_minecraft_block::keys::Block>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub properties: Option<BTreeMap<String, String>>,
}

#[derive(Debug, Clone, PartialEq)]
pub enum BlockState {
    Block(Id<mcrs_minecraft_block::keys::Block>),
    Full(FullBlockState),
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(remote = "Self", deny_unknown_fields)]
pub enum TypedBlockStateProvider {
    Simple { state: BlockState },
}

mcrs_minecraft_registry::dispatch! {
    TypedBlockStateProvider, key = "type", registry = mcrs_minecraft_block_predicate::keys::BlockStateProviderType,
    {
        Simple => Simple,
    }
    unsupported { CopyProperties, DualNoise, Noise, NoiseThreshold, RandomBlock, RandomizedInt, Rotated, RuleBased, Weighted }
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ProviderObject {
    #[serde(rename = "type")]
    kind: Option<String>,
    id: Option<Id<mcrs_minecraft_block::keys::Block>>,
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

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EnchantmentAttributeEffect {
    pub id: String,
    pub attribute: String,
    pub amount: LevelBasedValue,
    pub operation: AttributeOperation,
}

/// Location-based effects are the entity effects and `attribute`, so the two
/// enums share their variants and the location one adds its own.
macro_rules! effect_enum {
    ($(#[$meta:meta])* $name:ident { $($extra:tt)* }) => {
        $(#[$meta])*
        #[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
        #[serde(remote = "Self", deny_unknown_fields)]
        pub enum $name {
            AllOf {
                effects: Vec<$name>,
            },
            $($extra)*

        ApplyMobEffect {
            to_apply: HolderSet<mcrs_minecraft_item::keys::MobEffect>,
            min_duration: LevelBasedValue,
            max_duration: LevelBasedValue,
            min_amplifier: LevelBasedValue,
            max_amplifier: LevelBasedValue,
        },
        ChangeItemDamage { amount: LevelBasedValue },
        DamageEntity {
            min_damage: LevelBasedValue,
            max_damage: LevelBasedValue,
            damage_type: String,
        },
        Explode {
            #[serde(default, skip_serializing_if = "Option::is_none")]
            attribute_to_user: Option<bool>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            damage_type: Option<String>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            knockback_multiplier: Option<LevelBasedValue>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            immune_blocks: Option<HolderSet<mcrs_minecraft_block::keys::Block>>,
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
        Ignite { duration: LevelBasedValue },
        ApplyImpulse {
            direction: [f64; 3],
            coordinate_scale: [f64; 3],
            magnitude: LevelBasedValue,
        },
        ApplyExhaustion { amount: LevelBasedValue },
        PlaySound {
            sound: HolderSet<mcrs_minecraft_sound::SoundEvent>,
            volume: FloatProvider,
            pitch: FloatProvider,
        },
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
        SpawnParticles {
            particle: ParticleOptions,
            horizontal_position: PositionSource,
            vertical_position: PositionSource,
            horizontal_velocity: VelocitySource,
            vertical_velocity: VelocitySource,
            speed: FloatProvider,
        },
        SummonEntity {
            entity: HolderSet<mcrs_minecraft_entity::keys::EntityType>,
            #[serde(default, skip_serializing_if = "Option::is_none")]
            join_team: Option<bool>,
        },
        }
    };
}

effect_enum! {
    EnchantmentEntityEffect {}
}

effect_enum! {
    EnchantmentLocationBasedEffect {
        Attribute(EnchantmentAttributeEffect),
    }
}

mcrs_minecraft_registry::dispatch! {
    EnchantmentEntityEffect, key = "type", registry = crate::keys::EnchantmentEntityEffectType,
    {
        AllOf => AllOf,
        ApplyMobEffect => ApplyMobEffect,
        ChangeItemDamage => ChangeItemDamage,
        DamageEntity => DamageEntity,
        Explode => Explode,
        Ignite => Ignite,
        ApplyImpulse => ApplyImpulse,
        ApplyExhaustion => ApplyExhaustion,
        PlaySound => PlaySound,
        ReplaceDisk => ReplaceDisk,
        SpawnParticles => SpawnParticles,
        SummonEntity => SummonEntity,
    }
    unsupported { ReplaceBlock, RunFunction, SetBlockProperties }
}

mcrs_minecraft_registry::dispatch! {
    EnchantmentLocationBasedEffect, key = "type",
    registry = crate::keys::EnchantmentLocationBasedEffectType,
    {
        Attribute => Attribute,
        AllOf => AllOf,
        ApplyMobEffect => ApplyMobEffect,
        ChangeItemDamage => ChangeItemDamage,
        DamageEntity => DamageEntity,
        Explode => Explode,
        Ignite => Ignite,
        ApplyImpulse => ApplyImpulse,
        ApplyExhaustion => ApplyExhaustion,
        PlaySound => PlaySound,
        ReplaceDisk => ReplaceDisk,
        SpawnParticles => SpawnParticles,
        SummonEntity => SummonEntity,
    }
    unsupported { ReplaceBlock, RunFunction, SetBlockProperties }
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
    /// The key names the component and so chooses the type of its value.
    EnchantmentEffects on crate::keys::EnchantmentEffectComponentType {
        DamageProtection => damage_protection: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        DamageImmunity => damage_immunity: Vec<ConditionalEffect<Unit>>,
        Damage => damage: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        SmashDamagePerFallenBlock => smash_damage_per_fallen_block: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        Knockback => knockback: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        ArmorEffectiveness => armor_effectiveness: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        PostAttack => post_attack: Vec<TargetedConditionalEffect<EnchantmentEntityEffect>>,
        PostPiercingAttack => post_piercing_attack: Vec<ConditionalEffect<EnchantmentEntityEffect>>,
        HitBlock => hit_block: Vec<ConditionalEffect<EnchantmentEntityEffect>>,
        ItemDamage => item_damage: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        EquipmentDrops => equipment_drops: Vec<TargetedConditionalEffect<EnchantmentValueEffect>>,
        LocationChanged => location_changed: Vec<ConditionalEffect<EnchantmentLocationBasedEffect>>,
        Tick => tick: Vec<ConditionalEffect<EnchantmentEntityEffect>>,
        AmmoUse => ammo_use: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        ProjectilePiercing => projectile_piercing: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        ProjectileSpawned => projectile_spawned: Vec<ConditionalEffect<EnchantmentEntityEffect>>,
        ProjectileSpread => projectile_spread: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        ProjectileCount => projectile_count: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        TridentReturnAcceleration => trident_return_acceleration: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        FishingTimeReduction => fishing_time_reduction: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        FishingLuckBonus => fishing_luck_bonus: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        BlockExperience => block_experience: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        MobExperience => mob_experience: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        RepairWithXp => repair_with_xp: Vec<ConditionalEffect<EnchantmentValueEffect>>,
        Attributes => attributes: Vec<EnchantmentAttributeEffect>,
        CrossbowChargeTime => crossbow_charge_time: EnchantmentValueEffect,
        CrossbowChargingSounds => crossbow_charging_sounds: Vec<ChargingSounds>,
        TridentSound => trident_sound: Vec<String>,
        PreventEquipmentDrop => prevent_equipment_drop: Unit,
        PreventArmorChange => prevent_armor_change: Unit,
        TridentSpinAttackStrength => trident_spin_attack_strength: EnchantmentValueEffect,
    }
}
