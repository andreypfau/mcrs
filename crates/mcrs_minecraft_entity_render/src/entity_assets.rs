use std::collections::HashMap;

use bevy::prelude::Resource;
use mcrs_minecraft_client_entity::VariantId;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_molang::string_hash;

use crate::entity_library::EntityLibrary;
use crate::entity_pack::EntityPack;
use crate::frame_queries::VariantEntry;

/// The texture identifier of the missing texture, which the entity textures hold first. It is
/// not a texture of the resource pack, so no pack texture can take its place.
pub(crate) const MISSING_TEXTURE: &str = "mcrs:missing";

/// An entry of a synced variant registry, with the strings that the render controllers and the
/// animations compare as [`string_hash`]es.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct VariantInfo {
    /// The entry as an adult reads it.
    pub adult: VariantEntry,
    /// The asset of a baby, when the entry gives babies their own.
    pub baby_asset: Option<u64>,
}

impl VariantInfo {
    /// The entry as an adult or a baby reads it.
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

/// A variant entry: the entity type whose entity data names it and its position in the synced
/// registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct VariantKey {
    pub kind: EntityType,
    pub number: u16,
}

/// What the entity passes draw: the pack, its tables on the GPU, the client entity of each
/// entity type, and the registry data that the render controllers read.
#[derive(Clone, Debug, Default, Resource)]
pub struct EntityAssets {
    /// Changes with each new library, so that the render world builds it again.
    pub generation: u32,
    pub pack: EntityPack,
    pub library: EntityLibrary,
    /// The index of the client entity of each entity type in [`EntityPack::entities`].
    pub kinds: HashMap<EntityType, usize>,
    pub variants: HashMap<VariantKey, VariantInfo>,
    /// The variant number of an entity whose entity data names none.
    pub default_variants: HashMap<EntityType, u16>,
    /// The texture identifier of each equipment asset and layer type.
    pub equipment_textures: HashMap<(String, String), String>,
    /// The `string_hash` of the texture identifier of each skin layer.
    pub skin_textures: Vec<u64>,
    /// The cull radius of each definition at the scale 1, in blocks.
    pub radii: Vec<f32>,
    /// The variant of each texture identifier by its `string_hash`.
    texture_hashes: HashMap<u64, u16>,
}

impl EntityAssets {
    /// Indexes the textures of the library by the hash of their identifiers.
    pub fn index_textures(&mut self) {
        self.texture_hashes = self
            .library
            .textures
            .iter()
            .map(|(identifier, variant)| (string_hash(identifier), *variant))
            .collect();
    }

    /// The variant record of a texture identifier, or the missing texture.
    #[must_use]
    pub fn texture(&self, hash: u64) -> u16 {
        self.texture_hashes
            .get(&hash)
            .or_else(|| self.texture_hashes.get(&string_hash(MISSING_TEXTURE)))
            .copied()
            .unwrap_or(0)
    }

    /// The variant of an entity of the type `kind` whose entity data names `variant`, or the
    /// default variant of the type when it names none.
    #[must_use]
    pub fn variant(&self, kind: EntityType, variant: Option<VariantId>) -> Option<&VariantInfo> {
        let number = match variant {
            Some(id) => id.number(),
            None => *self.default_variants.get(&kind)?,
        };
        self.variants.get(&VariantKey { kind, number })
    }
}

/// The texture identifier `ns:path` of a texture path of a client entity: `textures/path` is
/// `minecraft:path`.
#[must_use]
pub(crate) fn texture_identifier(path: &str) -> String {
    match path.strip_prefix("textures/") {
        Some(rest) => format!("minecraft:{rest}"),
        None if path.contains(':') => path.to_owned(),
        None => format!("minecraft:{path}"),
    }
}
