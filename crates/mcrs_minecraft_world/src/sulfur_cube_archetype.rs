use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::{NonNegativeInt, PositiveInt, is_default};
use mcrs_minecraft_core::value_provider::{DispatchedFloatProvider, FloatProvider};
use mcrs_minecraft_entity::keys::Attribute;
use mcrs_minecraft_item::AttributeOperation;
use mcrs_minecraft_item::keys::Item;
use mcrs_minecraft_registry::{Holder, HolderSet, Id};
use mcrs_minecraft_sound::SoundEvent;
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SulfurCubeArchetype {
    pub items: HolderSet<Item>,
    pub attribute_modifiers: Vec<AttributeEntry>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub buoyant: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explosion: Option<ExplosionData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_damage: Option<ContactDamage>,
    pub knockback_modifiers: KnockbackModifiers,
    pub sound_settings: SoundSettings,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttributeEntry {
    pub attribute: Id<Attribute>,
    pub id: ResourceLocation,
    pub amount: f64,
    pub operation: AttributeOperation,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContactDamage {
    pub damage_type: Id<mcrs_minecraft_item::damage_type::DamageType>,
    #[serde(deserialize_with = "non_negative_float_provider")]
    pub amount: FloatProvider,
    pub attribute_to_source: bool,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ExplosionData {
    pub power: NonNegativeInt,
    pub causes_fire: bool,
    pub fuse: PositiveInt,
}

#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct KnockbackModifiers {
    pub horizontal_power: f32,
    pub vertical_power: f32,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoundSettings {
    pub hit_sound: Holder<SoundEvent>,
    pub push_sound: Holder<SoundEvent>,
    pub push_sound_impulse_threshold: f32,
    pub push_sound_cooldown: f32,
}

fn non_negative_float_provider<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<FloatProvider, D::Error> {
    let provider = FloatProvider::deserialize(deserializer)?;
    let lowest = match provider {
        FloatProvider::Constant(value) => value,
        FloatProvider::Dispatched(DispatchedFloatProvider::Uniform { min_inclusive, .. }) => {
            min_inclusive
        }
        FloatProvider::Dispatched(
            DispatchedFloatProvider::ClampedNormal { min, .. }
            | DispatchedFloatProvider::Trapezoid { min, .. },
        ) => min,
    };
    if lowest < 0.0 {
        return Err(D::Error::custom(format_args!(
            "Value provider too low: 0.0 [{lowest}]"
        )));
    }
    Ok(provider)
}
