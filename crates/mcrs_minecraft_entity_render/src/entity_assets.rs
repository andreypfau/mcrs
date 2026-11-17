use std::collections::HashMap;

use bevy::prelude::Resource;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity_visual::VariantId;
use mcrs_molang::string_hash;

use crate::entity_library::EntityLibrary;
use crate::entity_pack::EntityPack;
use crate::frame_queries::VariantEntry;

pub(crate) const MISSING_TEXTURE: &str = "mcrs:missing";

#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VariantInfo {
    pub adult: VariantEntry,
    pub baby_asset: Option<u64>,
}

impl VariantInfo {
    #[must_use]
    pub fn entry(&self, baby: bool) -> VariantEntry {
        match (self.baby_asset, baby) {
            (Some(asset), true) => VariantEntry {
                asset,
                ..self.adult
            },
            _ => self.adult,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VariantKey {
    pub kind: EntityType,
    pub number: u16,
}

#[derive(Clone, Debug, Default, Resource)]
pub struct EntityAssets {
    pub generation: u32,
    pub pack: EntityPack,
    pub library: EntityLibrary,
    pub kinds: HashMap<EntityType, usize>,
    pub variants: HashMap<VariantKey, VariantInfo>,
    pub default_variants: HashMap<EntityType, u16>,
    pub equipment_textures: HashMap<(String, String), String>,
    pub radii: Vec<f32>,
    texture_hashes: HashMap<u64, u16>,
}

impl EntityAssets {
    pub fn index_textures(&mut self) {
        self.texture_hashes = self
            .library
            .textures
            .iter()
            .map(|(identifier, variant)| (string_hash(identifier), *variant))
            .collect();
    }

    #[must_use]
    pub fn texture(&self, hash: u64) -> u16 {
        self.texture_hashes
            .get(&hash)
            .or_else(|| self.texture_hashes.get(&string_hash(MISSING_TEXTURE)))
            .copied()
            .unwrap_or(0)
    }

    #[must_use]
    pub fn variant(&self, kind: EntityType, variant: Option<VariantId>) -> Option<&VariantInfo> {
        let number = match variant {
            Some(id) => id.number(),
            None => *self.default_variants.get(&kind)?,
        };
        self.variants.get(&VariantKey { kind, number })
    }
}

#[must_use]
pub(crate) fn texture_identifier(path: &str) -> String {
    match path.strip_prefix("textures/") {
        Some(rest) => format!("minecraft:{rest}"),
        None if path.contains(':') => path.to_owned(),
        None => format!("minecraft:{path}"),
    }
}
