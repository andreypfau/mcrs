use std::collections::BTreeSet;

use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::variant::PigModel;
use mcrs_minecraft_game_registries::variant::{FrogVariantFile, PigVariantFile};
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::{RegistryValue, ResourceLocation, ResourceLocationBuf};
use mcrs_molang::string_hash;

use crate::entity_assets::{EntityAssets, VariantInfo, VariantKey};
use crate::frame_queries::VariantEntry;

pub(crate) trait VariantSource: RegistryValue + 'static {
    fn asset(&self) -> &ResourceLocationBuf;

    fn baby_asset(&self) -> Option<&ResourceLocationBuf> {
        None
    }

    fn model(&self) -> &'static str {
        ""
    }
}

impl VariantSource for PigVariantFile {
    fn asset(&self) -> &ResourceLocationBuf {
        &self.asset_id
    }

    fn baby_asset(&self) -> Option<&ResourceLocationBuf> {
        Some(&self.baby_asset_id)
    }

    fn model(&self) -> &'static str {
        match self.model {
            PigModel::Normal => "normal",
            PigModel::Cold => "cold",
        }
    }
}

impl VariantSource for FrogVariantFile {
    fn asset(&self) -> &ResourceLocationBuf {
        &self.asset_id
    }
}

#[derive(Clone, Copy, Debug)]
pub(crate) struct VariantRegistry {
    pub(crate) kind: EntityType,
    pub(crate) default: &'static str,
    pub(crate) add: fn(&Registries, &VariantRegistry, &mut EntityAssets, &mut BTreeSet<String>),
}

pub(crate) const VARIANT_REGISTRIES: [VariantRegistry; 2] = [
    VariantRegistry {
        kind: EntityType::Pig,
        default: "minecraft:temperate",
        add: add_variants::<PigVariantFile>,
    },
    VariantRegistry {
        kind: EntityType::Frog,
        default: "minecraft:temperate",
        add: add_variants::<FrogVariantFile>,
    },
];

fn add_variants<V: VariantSource>(
    registries: &Registries,
    registry: &VariantRegistry,
    assets: &mut EntityAssets,
    identifiers: &mut BTreeSet<String>,
) {
    let Some(entries) = registries.registry::<V>() else {
        return;
    };
    for (id, identifier, entry) in entries.iter() {
        let asset = entry.asset().to_string();
        let baby_asset = entry.baby_asset().map(ToString::to_string);
        let info = VariantInfo {
            adult: VariantEntry {
                id: f32::from(id.number()),
                identifier: string_hash(&identifier.to_string()),
                asset: string_hash(&asset),
                model: string_hash(entry.model()),
            },
            baby_asset: baby_asset.as_deref().map(string_hash),
        };
        identifiers.insert(asset);
        identifiers.extend(baby_asset);
        let key = VariantKey {
            kind: registry.kind,
            number: id.number(),
        };
        assets.variants.insert(key, info);
    }
    let default = ResourceLocation::read(registry.default)
        .ok()
        .and_then(|name| entries.by_name(name))
        .map_or(0, |id| id.number());
    assets.default_variants.insert(registry.kind, default);
}
