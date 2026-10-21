use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_core::codec::{NonNegativeInt, PositiveInt, is_default};
use mcrs_minecraft_entity::keys::Attribute;
use mcrs_minecraft_item::AttributeOperation;
use mcrs_minecraft_item::keys::Item;
use mcrs_minecraft_registries::{Refs, RegistryOf, Resolved, registry_value};
use mcrs_minecraft_sound::Sound;
use mcrs_minecraft_value_provider::{DispatchedFloatProvider, FloatProvider};
use serde::de::Error as _;
use serde::{Deserialize, Deserializer, Serialize};

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SulfurCubeArchetype<M: Refs = Resolved> {
    pub items: M::Set<Item>,
    pub attribute_modifiers: Vec<AttributeEntry<M>>,
    #[serde(default, skip_serializing_if = "is_default")]
    pub buoyant: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explosion: Option<ExplosionData>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub contact_damage: Option<ContactDamage<M>>,
    pub knockback_modifiers: KnockbackModifiers,
    pub sound_settings: SoundSettings<M>,
}

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct AttributeEntry<M: Refs = Resolved> {
    pub attribute: M::Id<Attribute>,
    pub id: ResourceLocation,
    pub amount: f64,
    pub operation: AttributeOperation,
}

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ContactDamage<M: Refs = Resolved> {
    pub damage_type: M::Id<RegistryOf<mcrs_minecraft_item::damage_type::DamageType>>,
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

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SoundSettings<M: Refs = Resolved> {
    pub hit_sound: Sound<M>,
    pub push_sound: Sound<M>,
    pub push_sound_impulse_threshold: f32,
    pub push_sound_cooldown: f32,
}

fn non_negative_float_provider<'de, D: Deserializer<'de>>(
    deserializer: D,
) -> Result<FloatProvider, D::Error> {
    let provider = FloatProvider::deserialize(deserializer)?;
    let lowest = match provider {
        FloatProvider::Constant(value)
        | FloatProvider::Dispatched(DispatchedFloatProvider::Constant { value }) => value,
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

mcrs_minecraft_registries::declare_registries!(
    SulfurCubeArchetype => crate::keys::SULFUR_CUBE_ARCHETYPE, World;
);
