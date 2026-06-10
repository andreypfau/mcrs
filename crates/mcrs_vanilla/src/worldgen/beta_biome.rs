use std::env;
use std::sync::Arc;

use bevy_app::{App, Plugin, Startup};
use bevy_asset::AssetServer;
use bevy_ecs::prelude::{Commands, Res, Resource};
use bevy_reflect::TypePath;

use crate::biome::source::{BiomeSource, ProtoBiomeSource, build_beta_lookup_table};
use crate::ResourceLocation;

/// Carries the active world preset's Beta biome source once it is built at startup.
///
/// Present only when the active preset uses `mcrs:beta` as its overworld biome source.
/// `dispatch_column_generation` reads this resource to fill `BiomePalette` from climate.
#[derive(Resource, TypePath)]
pub struct ActiveBiomeSource(pub Arc<BiomeSource>);

pub struct BetaBiomeSourcePlugin;

impl Plugin for BetaBiomeSourcePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(Startup, build_beta_biome_source_on_start);
    }
}

/// Reads the active world preset JSON from disk, extracts the overworld biome_source,
/// and inserts `ActiveBiomeSource` if it is an `mcrs:beta` source.
///
/// Mirrors the `resolve_overworld_noise_settings` pattern from `mcrs_minecraft_worldgen`:
/// reads the preset path from `MCRS_WORLD_PRESET` (defaulting to `minecraft:normal`) and
/// constructs `Handle<Biome>` via `AssetServer::load` for each listed biome id.
fn build_beta_biome_source_on_start(mut commands: Commands, asset_server: Res<AssetServer>) {
    let (preset_ns, preset_path) = resolve_active_preset();
    let asset_root = env::var("BEVY_ASSET_ROOT").unwrap_or_else(|_| ".".to_string());
    let json_path = format!(
        "{}/assets/{}/worldgen/world_preset/{}.json",
        asset_root, preset_ns, preset_path
    );

    let data = match std::fs::read_to_string(&json_path) {
        Ok(d) => d,
        // A missing preset file is expected when no Beta preset is selected.
        Err(_) => return,
    };

    let json: serde_json::Value = match serde_json::from_str(&data) {
        Ok(v) => v,
        Err(e) => {
            tracing::warn!(path = %json_path, error = %e, "world preset is not valid JSON");
            return;
        }
    };

    let biome_source_val = match json
        .get("dimensions")
        .and_then(|d| d.get("minecraft:overworld"))
        .and_then(|ow| ow.get("generator"))
        .and_then(|g| g.get("biome_source"))
    {
        Some(v) => v.clone(),
        // No biome_source: not a Beta preset, expected.
        None => return,
    };

    let proto: ProtoBiomeSource = match serde_json::from_value(biome_source_val) {
        Ok(p) => p,
        Err(e) => {
            tracing::warn!(path = %json_path, error = %e, "world preset biome_source failed to deserialize");
            return;
        }
    };

    // Only a `mcrs:beta` source activates the Beta biome fill path; any other
    // source type is handled elsewhere and is silently skipped here.
    let ProtoBiomeSource::Beta { biomes, ocean_biomes } = proto else {
        return;
    };

    let land_handles: Vec<_> = biomes
        .into_iter()
        .map(|loc| {
            let path = format!("{}/worldgen/biome/{}.json", loc.namespace(), loc.path());
            asset_server.load(path)
        })
        .collect();

    let ocean_handles: Vec<_> = ocean_biomes
        .into_iter()
        .map(|loc| {
            let path = format!("{}/worldgen/biome/{}.json", loc.namespace(), loc.path());
            asset_server.load(path)
        })
        .collect();

    let land_biomes: [_; 11] = match land_handles.try_into() {
        Ok(arr) => arr,
        Err(handles) => {
            tracing::error!(
                path = %json_path,
                got = handles.len(),
                expected = 11,
                "mcrs:beta world preset lists the wrong number of land biomes"
            );
            return;
        }
    };
    let ocean_biomes: [_; 5] = match ocean_handles.try_into() {
        Ok(arr) => arr,
        Err(handles) => {
            tracing::error!(
                path = %json_path,
                got = handles.len(),
                expected = 5,
                "mcrs:beta world preset lists the wrong number of ocean biomes"
            );
            return;
        }
    };

    let source = BiomeSource::Beta {
        land_biomes,
        ocean_biomes,
        lookup: Box::new(build_beta_lookup_table()),
    };
    commands.insert_resource(ActiveBiomeSource(Arc::new(source)));
}

fn resolve_active_preset() -> (String, String) {
    match env::var("MCRS_WORLD_PRESET") {
        Ok(raw) => {
            let trimmed = raw.trim().to_lowercase();
            if let Some(colon) = trimmed.find(':') {
                (trimmed[..colon].to_string(), trimmed[colon + 1..].to_string())
            } else if !trimmed.is_empty() {
                ("minecraft".to_string(), trimmed)
            } else {
                ("minecraft".to_string(), "normal".to_string())
            }
        }
        Err(_) => ("minecraft".to_string(), "normal".to_string()),
    }
}
