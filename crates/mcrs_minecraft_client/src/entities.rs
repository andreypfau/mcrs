//! The glue between the entity passes and the client: the entity pack of the repository, the
//! entity textures that the synced variant registries and the equipment assets name, and the
//! client entity of each entity type.

use std::collections::{BTreeMap, BTreeSet};

use bevy::prelude::*;
use bevy::render::renderer::RenderDevice;
use mcrs_minecraft_client_entity::KindTraits;
use mcrs_minecraft_entity::entity_type_keys::EntityType;
use mcrs_minecraft_entity::variant::PigModel;
use mcrs_minecraft_entity_render::{
    EntityAssets, EntityLibrary, EntityPack, EntityRenderPlugin, MISSING_TEXTURE, TextureImage,
    VariantInfo, VariantKey, texture_identifier,
};
use mcrs_minecraft_game_registries::variant::{FrogVariantFile, PigVariantFile};
use mcrs_minecraft_model::atlas::{MISSING_SIZE, decode_png, missing_pixels};
use mcrs_minecraft_model::resource_set::{ResourceSet, resource_path};
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_resource::ResourceLocation;
use serde::Deserialize;

use crate::stream::BlockCatalog;

/// The files of the entity pack of the repository, by their path below `assets/mcrs/entity`.
const PACK: [(&str, &str); 8] = [
    (
        "entity/pig.entity.json",
        include_str!("../../../assets/mcrs/entity/entity/pig.entity.json"),
    ),
    (
        "entity/frog.entity.json",
        include_str!("../../../assets/mcrs/entity/entity/frog.entity.json"),
    ),
    (
        "animations/pig.animation.json",
        include_str!("../../../assets/mcrs/entity/animations/pig.animation.json"),
    ),
    (
        "animations/frog.animation.json",
        include_str!("../../../assets/mcrs/entity/animations/frog.animation.json"),
    ),
    (
        "geometry/pig.json",
        include_str!("../../../assets/mcrs/entity/geometry/pig.json"),
    ),
    (
        "geometry/frog.json",
        include_str!("../../../assets/mcrs/entity/geometry/frog.json"),
    ),
    (
        "render_controllers/pig.render_controllers.json",
        include_str!("../../../assets/mcrs/entity/render_controllers/pig.render_controllers.json"),
    ),
    (
        "render_controllers/frog.render_controllers.json",
        include_str!("../../../assets/mcrs/entity/render_controllers/frog.render_controllers.json"),
    ),
];

/// The variant of a pig or a frog whose entity data names none, when the registry has it.
const DEFAULT_VARIANT: &str = "minecraft:temperate";

/// The texture array layers that one array holds at most.
const ARRAY_LAYERS: u32 = 2048;

/// Draws the client entities with the entity passes, once the resource pack and the synced
/// registries are in.
pub struct ClientEntityRenderPlugin;

impl Plugin for ClientEntityRenderPlugin {
    fn build(&self, app: &mut App) {
        app.add_plugins(EntityRenderPlugin).add_systems(
            Update,
            build_entity_assets.run_if(resource_exists::<CurrentRegistries>),
        );
    }
}

/// The registries and the pack that the assets were built from.
#[derive(Default)]
struct Built {
    registries: Option<bevy::ecs::change_detection::Tick>,
}

fn build_entity_assets(
    mut commands: Commands,
    catalog: Option<Res<BlockCatalog>>,
    registries: Res<CurrentRegistries>,
    device: Option<Res<RenderDevice>>,
    assets: Option<Res<EntityAssets>>,
    mut built: Local<Built>,
) {
    let Some(resources) = catalog.as_deref().and_then(BlockCatalog::pack) else {
        return;
    };
    let changed = registries.last_changed();
    if built.registries == Some(changed) {
        return;
    }
    built.registries = Some(changed);
    let layer_limit = device.map_or(256, |device| {
        device.limits().max_texture_array_layers.min(ARRAY_LAYERS)
    });
    let generation = assets.map_or(0, |assets| assets.generation.wrapping_add(1));
    let entity_assets = entity_assets(resources, &registries, layer_limit, generation);
    info!(
        definitions = entity_assets.library.definitions.len(),
        textures = entity_assets.library.textures.len(),
        diagnostics = entity_assets.pack.diagnostics.len(),
        "entity assets built"
    );
    for diagnostic in &entity_assets.pack.diagnostics {
        warn!("{diagnostic}");
    }
    commands.insert_resource(entity_assets);
}

fn entity_assets(
    resources: &ResourceSet,
    registries: &CurrentRegistries,
    layer_limit: u32,
    generation: u32,
) -> EntityAssets {
    let files = PACK.iter().map(|(path, text)| (*path, *text));
    let pack = EntityPack::load(files);
    let mut assets = EntityAssets::default();
    assets.generation = generation;
    let mut identifiers: BTreeSet<String> = BTreeSet::new();
    if let Some(registry) = registries.registry::<PigVariantFile>() {
        for (id, name, variant) in registry.iter() {
            let model = match variant.model {
                PigModel::Normal => "normal",
                PigModel::Cold => "cold",
            };
            let info = VariantInfo {
                id: f32::from(id.number()),
                identifier: name.to_string(),
                asset: variant.asset_id.to_string(),
                baby_asset: Some(variant.baby_asset_id.to_string()),
                model: model.to_owned(),
            };
            identifiers.insert(info.asset.clone());
            identifiers.extend(info.baby_asset.clone());
            assets.variants.insert(VariantKey::Pig(id.number()), info);
        }
        let default = ResourceLocation::read(DEFAULT_VARIANT)
            .ok()
            .and_then(|name| registry.by_name(name))
            .map_or(0, |id| id.number());
        assets
            .default_variants
            .insert(EntityType::Pig, VariantKey::Pig(default));
    }
    if let Some(registry) = registries.registry::<FrogVariantFile>() {
        for (id, name, variant) in registry.iter() {
            let info = VariantInfo {
                id: f32::from(id.number()),
                identifier: name.to_string(),
                asset: variant.asset_id.to_string(),
                baby_asset: None,
                model: String::new(),
            };
            identifiers.insert(info.asset.clone());
            assets.variants.insert(VariantKey::Frog(id.number()), info);
        }
        let default = ResourceLocation::read(DEFAULT_VARIANT)
            .ok()
            .and_then(|name| registry.by_name(name))
            .map_or(0, |id| id.number());
        assets
            .default_variants
            .insert(EntityType::Frog, VariantKey::Frog(default));
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
    add_kinds(&pack, &mut assets);
    let library = EntityLibrary::build(&pack.sources(), &textures, layer_limit);
    assets.library = library;
    assets.pack = pack;
    assets.index_textures();
    assets
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

/// The texture of the first entry of each layer type of each equipment asset file
/// (`docs/spec/molang-queries.md`, MOLQ-STATE-002, item 7).
fn add_equipment(
    resources: &ResourceSet,
    assets: &mut EntityAssets,
    identifiers: &mut BTreeSet<String>,
) {
    for (asset, bytes) in resources.entries("equipment", "json") {
        let Ok(file) = serde_json::from_slice::<EquipmentAsset>(bytes) else {
            continue;
        };
        for (layer, entries) in file.layers {
            let Some(texture) = entries.first().map(|entry| entry.texture.as_str()) else {
                continue;
            };
            let (namespace, path) = texture.split_once(':').unwrap_or(("minecraft", texture));
            let identifier = format!("{namespace}:entity/equipment/{layer}/{path}");
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

/// The client entity and the cull radius of each entity type of the pack.
fn add_kinds(pack: &EntityPack, assets: &mut EntityAssets) {
    for (index, entity) in pack.entities.iter().enumerate() {
        let Some(kind) = EntityType::read(&entity.resources.identifier) else {
            continue;
        };
        assets.kinds.insert(kind, index);
        let traits = KindTraits::of(kind);
        let half = traits.width / 2.0 + 0.5;
        let corner = |x: f32, y: f32, z: f32| (x * x + y * y + z * z).sqrt();
        let box_radius = corner(half, (traits.height + 0.5).max(0.5), half);
        let bounds = entity
            .geometries
            .iter()
            .flatten()
            .map(|geometry| {
                let bounds = geometry.visible_bounds;
                let far = bounds.min.abs().max(bounds.max.abs());
                far.length()
            })
            .fold(0.0f32, f32::max);
        if assets.radii.len() <= index {
            assets.radii.resize(index + 1, 1.0);
        }
        if let Some(radius) = assets.radii.get_mut(index) {
            *radius = box_radius.max(bounds);
        }
    }
}
