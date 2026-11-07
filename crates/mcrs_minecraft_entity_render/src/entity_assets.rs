use std::collections::HashMap;

use bevy::prelude::Resource;
use mcrs_minecraft_client_entity::VariantId;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_molang::string_hash;

use crate::entity_library::EntityLibrary;
use crate::entity_pack::EntityPack;

/// The texture identifier of the missing texture of the resource pack.
pub const MISSING_TEXTURE: &str = "mcrs:missing";

/// An entry of a synced variant registry.
#[derive(Clone, Debug, PartialEq)]
pub struct VariantInfo {
    /// The position of the entry in the registry.
    pub id: f32,
    pub identifier: String,
    pub asset: String,
    pub baby_asset: Option<String>,
    /// The model type in lower case.
    pub model: String,
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
    /// The variant of an entity whose entity data names none.
    pub default_variants: HashMap<EntityType, VariantKey>,
    /// The texture identifier of each equipment asset and layer type.
    pub equipment_textures: HashMap<(String, String), String>,
    /// The cull radius of each definition at the scale 1, in blocks.
    pub radii: Vec<f32>,
    /// The variant of each texture identifier by its `string_hash`.
    texture_hashes: HashMap<u64, u16>,
}

/// A variant entry of a variant registry.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum VariantKey {
    Pig(u16),
    Frog(u16),
}

impl From<VariantId> for VariantKey {
    fn from(id: VariantId) -> Self {
        match id {
            VariantId::Pig(id) => Self::Pig(id.number()),
            VariantId::Frog(id) => Self::Frog(id.number()),
        }
    }
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
}

/// The texture identifier `ns:path` of a texture path of a client entity: `textures/path` is
/// `minecraft:path`.
#[must_use]
pub fn texture_identifier(path: &str) -> String {
    match path.strip_prefix("textures/") {
        Some(rest) => format!("minecraft:{rest}"),
        None if path.contains(':') => path.to_owned(),
        None => format!("minecraft:{path}"),
    }
}
