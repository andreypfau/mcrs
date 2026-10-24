use std::collections::BTreeMap;

use mcrs_minecraft_core::codec::is_default;
use mcrs_minecraft_core::{RegistryValue, ResourceLocation};
use mcrs_minecraft_entity::variant::{
    CatVariant, ChickenModel, ChickenVariant, CowModel, CowVariant, FrogVariant, PigModel,
    PigVariant, WolfVariant, WolfVariantAssets, ZombieNautilusModel, ZombieNautilusVariant,
};
use mcrs_minecraft_registries::{Named, Refs, Registries, Resolve, Resolved, registry_value};
use serde::{Deserialize, Serialize};

pub use mcrs_minecraft_entity::spawn_condition::SpawnSelector;

macro_rules! spawning_variant {
    (
        $file:ident for $value:ident {
            $($(#[$field_meta:meta])* $field:ident: $ty:ty),+ $(,)?
        }
    ) => {
        #[registry_value]
        #[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
        #[serde(deny_unknown_fields)]
        pub struct $file<M: Refs = Resolved> {
            $($(#[$field_meta])* pub $field: $ty,)+
            pub spawn_conditions: Vec<SpawnSelector<M>>,
        }

        impl mcrs_minecraft_core::RegistryValue for $file {
            type Registry = $value;
        }

        impl SpawnConditions for $file {
            fn spawn_conditions(&self) -> &[SpawnSelector] {
                &self.spawn_conditions
            }
        }

        impl From<&$file> for $value {
            fn from(file: &$file) -> Self {
                $value { $($field: file.$field.clone(),)+ }
            }
        }

        /// A client never learns the spawn conditions of a variant.
        impl From<$value> for $file {
            fn from(value: $value) -> Self {
                Self {
                    $($field: value.$field,)+
                    spawn_conditions: Vec::new(),
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

pub trait SpawnConditions: Send + Sync + 'static {
    fn spawn_conditions(&self) -> &[SpawnSelector];
}

/// A variant registry's spawn conditions by variant name.
pub fn spawn_selectors<F: SpawnConditions + RegistryValue + 'static>(
    registries: &Registries,
) -> Result<BTreeMap<ResourceLocation, Vec<SpawnSelector<Named>>>, String> {
    let registry = registries
        .registry::<F>()
        .ok_or_else(|| format!("{} is not loaded", std::any::type_name::<F>()))?;
    registry
        .iter()
        .map(|(_, name, file)| {
            let selectors = file
                .spawn_conditions()
                .iter()
                .map(|selector| selector.name(registries))
                .collect::<Result<_, _>>()
                .map_err(|error| format!("{}: {name}: {error}", registry.key()))?;
            Ok((name.clone(), selectors))
        })
        .collect()
}
