use std::collections::BTreeMap;

use mcrs_minecraft_core::codec::is_default;
use mcrs_minecraft_core::{RegistryKey, ResourceLocation};
use mcrs_minecraft_entity::variant::{
    CatVariant, ChickenModel, ChickenVariant, CowModel, CowVariant, FrogVariant, PigModel,
    PigVariant, WolfVariant, WolfVariantAssets, ZombieNautilusModel, ZombieNautilusVariant,
};
use mcrs_minecraft_registry::RegistrySet;
use serde::{Deserialize, Serialize};

pub use mcrs_minecraft_worldgen_structure::spawn_condition::SpawnSelector;

macro_rules! spawning_variant {
    (
        $file:ident for $value:ident {
            $($(#[$field_meta:meta])* $field:ident: $ty:ty),+ $(,)?
        }
    ) => {
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $file {
            $($(#[$field_meta])* pub $field: $ty,)+
            pub spawn_conditions: Vec<SpawnSelector>,
        }

        impl $file {
            pub fn split(file: &Self) -> ($value, Vec<SpawnSelector>) {
                (
                    $value { $($field: file.$field.clone(),)+ },
                    file.spawn_conditions.clone(),
                )
            }

            pub fn join((value, spawn_conditions): (&$value, &Vec<SpawnSelector>)) -> Self {
                Self {
                    $($field: value.$field.clone(),)+
                    spawn_conditions: spawn_conditions.clone(),
                }
            }
        }
    };
}

spawning_variant! {
    WolfVariantFile for WolfVariant {
        assets: WolfVariantAssets,
        baby_assets: WolfVariantAssets,
    }
}

spawning_variant! {
    PigVariantFile for PigVariant {
        #[serde(default, skip_serializing_if = "is_default")]
        model: PigModel,
        asset_id: ResourceLocation,
        baby_asset_id: ResourceLocation,
    }
}

spawning_variant! {
    CowVariantFile for CowVariant {
        #[serde(default, skip_serializing_if = "is_default")]
        model: CowModel,
        asset_id: ResourceLocation,
        baby_asset_id: ResourceLocation,
    }
}

spawning_variant! {
    ChickenVariantFile for ChickenVariant {
        #[serde(default, skip_serializing_if = "is_default")]
        model: ChickenModel,
        asset_id: ResourceLocation,
        baby_asset_id: ResourceLocation,
    }
}

spawning_variant! {
    ZombieNautilusVariantFile for ZombieNautilusVariant {
        #[serde(default, skip_serializing_if = "is_default")]
        model: ZombieNautilusModel,
        asset_id: ResourceLocation,
    }
}

spawning_variant! {
    CatVariantFile for CatVariant {
        asset_id: ResourceLocation,
        baby_asset_id: ResourceLocation,
    }
}

spawning_variant! {
    FrogVariantFile for FrogVariant {
        asset_id: ResourceLocation,
    }
}

/// A variant registry's spawn conditions by variant, in registry order.
pub fn spawn_selectors<R>(
    registries: &RegistrySet,
    registry: RegistryKey<R>,
) -> BTreeMap<ResourceLocation, Vec<SpawnSelector>> {
    let registry = registry.location().as_static_str();
    let table = registries
        .table(registry)
        .unwrap_or_else(|| panic!("{registry} is not a loaded registry"));
    let conditions = registries
        .column::<Vec<SpawnSelector>>(registry)
        .unwrap_or_else(|| panic!("{registry} holds no spawn conditions"));
    table
        .names()
        .iter()
        .cloned()
        .zip(conditions.iter().cloned())
        .collect()
}
