use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use bevy::log::warn;
use glam::Vec3;
use mcrs_minecraft_entity::EntityTypeData;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_model::atlas::{MISSING_SIZE, decode_png, missing_pixels};
use mcrs_minecraft_model::resource_set::ResourceSet;
use mcrs_minecraft_model_file::resource_id::resource_path;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::ResourceLocation;
use serde::Deserialize;

use crate::entity_assets::{EntityAssets, MISSING_TEXTURE, texture_identifier};
use crate::entity_library::{EntityLibrary, TextureImage};
use crate::entity_pack::EntityPack;
use crate::player_skin::{SKIN_SLOTS, skin_identifier, skin_textures};
use crate::variant_source::VARIANT_REGISTRIES;

/// The texture array layers that one array holds at most.
const MAX_ARRAY_LAYERS: u32 = 2048;

impl EntityAssets {
    /// The assets of the entity pack of the repository, with the textures of `resources` that
    /// its client entities, the synced variant registries of `registries` and the equipment
    /// assets name. `array_layers` is the most layers of a texture array that the device takes.
    #[must_use]
    pub fn build(resources: &ResourceSet, registries: &Registries, array_layers: u32) -> Self {
        let pack = EntityPack::repository();
        let mut assets = Self::default();
        let mut identifiers: BTreeSet<String> = BTreeSet::new();
        for registry in &VARIANT_REGISTRIES {
            (registry.add)(registries, registry, &mut assets, &mut identifiers);
        }
        add_equipment(resources, &mut assets, &mut identifiers);
        for entity in &pack.entities {
            identifiers.extend(
                entity
                    .resources
                    .textures
                    .iter()
                    .map(|(_, path)| texture_identifier(path)),
            );
        }
        let textures = textures(resources, identifiers);
        assets.add_pack(pack, textures, array_layers);
        assets
    }

    /// Builds the library of `pack` with `textures` and the skin layers, and indexes it.
    pub(crate) fn add_pack(
        &mut self,
        pack: EntityPack,
        mut textures: Vec<TextureImage>,
        array_layers: u32,
    ) {
        textures.extend(skin_textures());
        add_kinds(&pack, self);
        self.library = EntityLibrary::build(
            &pack.sources(),
            &textures,
            array_layers.min(MAX_ARRAY_LAYERS),
        );
        self.library.skin_variants = skin_variants(&self.library);
        self.pack = pack;
        self.index_textures();
    }
}

/// The variants of the skin layers, which the library adds last and in slot order, or none when
/// a layer did not fit in the texture arrays.
fn skin_variants(library: &EntityLibrary) -> Range<u16> {
    let variant = |slot: u16| library.textures.get(&skin_identifier(slot)).copied();
    match (variant(0), variant(SKIN_SLOTS - 1)) {
        (Some(first), Some(last)) if last.checked_sub(first) == Some(SKIN_SLOTS - 1) => {
            first..last + 1
        }
        _ => {
            warn!("the skin layers do not fit in the entity textures; players draw default skins");
            0..0
        }
    }
}

#[derive(Deserialize)]
struct EquipmentAsset {
    #[serde(default)]
    layers: BTreeMap<String, Vec<EquipmentLayer>>,
}

#[derive(Deserialize)]
struct EquipmentLayer {
    texture: String,
}

/// The texture of the first entry of each layer type of each equipment asset file: the texture
/// that `docs/spec/molang-queries.md` names for an equipment layer.
fn add_equipment(
    resources: &ResourceSet,
    assets: &mut EntityAssets,
    identifiers: &mut BTreeSet<String>,
) {
    for (asset, bytes) in resources.entries("equipment", "json") {
        let file = match serde_json::from_slice::<EquipmentAsset>(bytes) {
            Ok(file) => file,
            Err(error) => {
                warn!("equipment asset {asset}: {error}");
                continue;
            }
        };
        for (layer, entries) in file.layers {
            let Some(texture) = entries.first().map(|entry| entry.texture.as_str()) else {
                continue;
            };
            let Ok(location) = ResourceLocation::read(texture) else {
                warn!("equipment asset {asset}: texture {texture:?} is not a resource location");
                continue;
            };
            let identifier = format!(
                "{}:entity/equipment/{layer}/{}",
                location.namespace(),
                location.path()
            );
            identifiers.insert(identifier.clone());
            assets
                .equipment_textures
                .insert((asset.clone(), layer), identifier);
        }
    }
}

/// The images of the texture identifiers that the resource pack has, after the missing texture.
fn textures(resources: &ResourceSet, identifiers: BTreeSet<String>) -> Vec<TextureImage> {
    let mut textures = vec![TextureImage {
        identifier: MISSING_TEXTURE.to_owned(),
        width: MISSING_SIZE,
        height: MISSING_SIZE,
        rgba: missing_pixels(),
    }];
    for identifier in identifiers {
        let path = resource_path(&identifier, "textures", "png");
        let Some(bytes) = resources.get(&path) else {
            continue;
        };
        match decode_png(bytes, &path) {
            Ok((rgba, width, height)) => textures.push(TextureImage {
                identifier,
                width,
                height,
                rgba,
            }),
            Err(error) => warn!("{error}"),
        }
    }
    textures
}

/// The client entity and the cull radius of each entity type of the pack. The radius encloses
/// the box of the type grown by half a block and the visible bounds of each geometry.
fn add_kinds(pack: &EntityPack, assets: &mut EntityAssets) {
    assets.radii = pack
        .entities
        .iter()
        .enumerate()
        .map(|(index, entity)| {
            let kind = EntityType::read(&entity.resources.identifier);
            if let Some(kind) = kind {
                assets.kinds.insert(kind, index);
            }
            let (width, height) = kind
                .and_then(EntityTypeData::of)
                .map_or((0.0, 0.0), |type_data| (type_data.width, type_data.height));
            let half = width / 2.0 + 0.5;
            let box_radius = Vec3::new(half, (height + 0.5).max(0.5), half).length();
            let bounds = entity
                .geometries
                .iter()
                .flatten()
                .map(|geometry| {
                    let bounds = geometry.visible_bounds;
                    bounds.min.abs().max(bounds.max.abs()).length()
                })
                .fold(0.0f32, f32::max);
            box_radius.max(bounds)
        })
        .collect();
}
