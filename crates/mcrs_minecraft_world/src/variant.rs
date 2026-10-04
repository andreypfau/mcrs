use std::collections::BTreeMap;

use mcrs_minecraft_core::codec::is_default;
use mcrs_minecraft_core::{HolderSet, RegistryKey, ResourceLocation};
use mcrs_minecraft_item::SoundEvent;
use mcrs_minecraft_registry::{EntrySet, Holder, Registry, RegistrySet, key};
use mcrs_minecraft_worldgen_feature::spawn_condition as feature;
use serde::{Deserialize, Serialize};

pub type SpawnSelector = feature::SpawnSelector<EntrySet<key::Structure>, EntrySet<key::Biome>>;

macro_rules! spawning_variant {
    (
        $name:ident, $network:ident {
            $($(#[$field_meta:meta])* $field:ident: $ty:ty),+ $(,)?
        }
    ) => {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $name {
            $($(#[$field_meta])* pub $field: $ty,)+
            pub spawn_conditions: Vec<SpawnSelector>,
        }

        #[derive(Debug, Clone, PartialEq, Serialize)]
        pub struct $network {
            $($(#[$field_meta])* pub $field: $ty,)+
        }

        impl From<&$name> for $network {
            fn from(variant: &$name) -> Self {
                Self { $($field: variant.$field.clone(),)+ }
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WolfVariantAssets {
    pub wild: ResourceLocation,
    pub tame: ResourceLocation,
    pub angry: ResourceLocation,
}

spawning_variant! {
    WolfVariant, NetworkWolfVariant {
        assets: WolfVariantAssets,
        baby_assets: WolfVariantAssets,
    }
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PigModel {
    #[default]
    Normal,
    Cold,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CowModel {
    #[default]
    Normal,
    Cold,
    Warm,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ChickenModel {
    #[default]
    Normal,
    Cold,
}

#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ZombieNautilusModel {
    #[default]
    Normal,
    Warm,
}

spawning_variant! {
    PigVariant, NetworkPigVariant {
        #[serde(default, skip_serializing_if = "is_default")]
        model: PigModel,
        asset_id: ResourceLocation,
        baby_asset_id: ResourceLocation,
    }
}

spawning_variant! {
    CowVariant, NetworkCowVariant {
        #[serde(default, skip_serializing_if = "is_default")]
        model: CowModel,
        asset_id: ResourceLocation,
        baby_asset_id: ResourceLocation,
    }
}

spawning_variant! {
    ChickenVariant, NetworkChickenVariant {
        #[serde(default, skip_serializing_if = "is_default")]
        model: ChickenModel,
        asset_id: ResourceLocation,
        baby_asset_id: ResourceLocation,
    }
}

spawning_variant! {
    ZombieNautilusVariant, NetworkZombieNautilusVariant {
        #[serde(default, skip_serializing_if = "is_default")]
        model: ZombieNautilusModel,
        asset_id: ResourceLocation,
    }
}

spawning_variant! {
    CatVariant, NetworkCatVariant {
        asset_id: ResourceLocation,
        baby_asset_id: ResourceLocation,
    }
}

spawning_variant! {
    FrogVariant, NetworkFrogVariant {
        asset_id: ResourceLocation,
    }
}

type Sound = Holder<SoundEvent>;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WolfSounds {
    pub ambient_sound: Sound,
    pub death_sound: Sound,
    pub growl_sound: Sound,
    pub hurt_sound: Sound,
    pub pant_sound: Sound,
    pub whine_sound: Sound,
    pub step_sound: Sound,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WolfSoundVariant {
    pub adult_sounds: WolfSounds,
    pub baby_sounds: WolfSounds,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PigSounds {
    pub ambient_sound: Sound,
    pub hurt_sound: Sound,
    pub death_sound: Sound,
    pub step_sound: Sound,
    pub eat_sound: Sound,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PigSoundVariant {
    pub adult_sounds: PigSounds,
    pub baby_sounds: PigSounds,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatSounds {
    pub ambient_sound: Sound,
    pub stray_ambient_sound: Sound,
    pub hiss_sound: Sound,
    pub hurt_sound: Sound,
    pub death_sound: Sound,
    pub eat_sound: Sound,
    pub beg_for_food_sound: Sound,
    pub purr_sound: Sound,
    pub purreow_sound: Sound,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CatSoundVariant {
    pub adult_sounds: CatSounds,
    pub baby_sounds: CatSounds,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct CowSoundVariant {
    pub ambient_sound: Sound,
    pub hurt_sound: Sound,
    pub death_sound: Sound,
    pub step_sound: Sound,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChickenSounds {
    pub ambient_sound: Sound,
    pub hurt_sound: Sound,
    pub death_sound: Sound,
    pub step_sound: Sound,
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ChickenSoundVariant {
    pub adult_sounds: ChickenSounds,
    pub baby_sounds: ChickenSounds,
}

/// A variant registry's spawn conditions with their sets written as names, the
/// form the structure generator resolves against its own indexes.
pub fn named_selectors<T: 'static>(
    registries: &RegistrySet,
    registry: &str,
    conditions: fn(&T) -> &[SpawnSelector],
) -> BTreeMap<ResourceLocation, Vec<feature::SpawnSelector>> {
    let structures = registries
        .registry::<key::Structure>()
        .expect("the structure registry is loaded");
    let biomes = registries
        .registry::<key::Biome>()
        .expect("the biome registry is loaded");
    let table = registries
        .table(registry)
        .unwrap_or_else(|| panic!("{registry} is not a loaded registry"));
    let values = registries
        .column::<T>(registry)
        .unwrap_or_else(|| panic!("{registry} holds no values of the requested type"));
    table
        .names()
        .iter()
        .zip(values)
        .map(|(name, value)| {
            let selectors = conditions(value)
                .iter()
                .map(|selector| feature::SpawnSelector {
                    priority: selector.priority,
                    condition: selector
                        .condition
                        .as_ref()
                        .map(|condition| match condition {
                            feature::SpawnCondition::Structure { structures: set } => {
                                feature::SpawnCondition::Structure {
                                    structures: names(set, &structures),
                                }
                            }
                            feature::SpawnCondition::Biome { biomes: set } => {
                                feature::SpawnCondition::Biome {
                                    biomes: names(set, &biomes),
                                }
                            }
                            feature::SpawnCondition::MoonBrightness { range } => {
                                feature::SpawnCondition::MoonBrightness { range: *range }
                            }
                        }),
                })
                .collect();
            (name.clone(), selectors)
        })
        .collect()
}

fn names<R: RegistryKey>(set: &EntrySet<R>, registry: &Registry<R>) -> HolderSet {
    let name = |id| {
        registry
            .key(id)
            .expect("a loaded entry is in its registry")
            .clone()
    };
    match set {
        EntrySet::Tag(tag) => HolderSet::Tag(tag.clone()),
        EntrySet::One(id) => HolderSet::One(name(*id)),
        EntrySet::List(ids) => HolderSet::List(ids.iter().copied().map(name).collect()),
    }
}
