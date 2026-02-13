use crate::biome::Biome;
use crate::dimension_type::DimensionType;
use crate::version::VERSION_ID;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::{Changed, Commands, Entity, On, Query};
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;
use mcrs_network::event::ReceivedPacketEvent;
use mcrs_network::{ConnectionState, InGameConnectionState, ServerSideConnection};
use mcrs_protocol::packets::configuration::clientbound::ClientboundSelectKnownPacks;
use mcrs_protocol::packets::configuration::serverbound::ServerboundFinishConfiguration;
use mcrs_protocol::packets::configuration::{
    ClientboundFinishConfiguration, ClientboundRegistryData,
};
use mcrs_protocol::registry::Entry;
use mcrs_protocol::resource_pack::KnownPack;
use mcrs_protocol::{Ident, WritePacket, ident, nbt};
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};
use std::borrow::Cow;
use std::collections::HashMap;
use std::env;
use std::str::FromStr;

/// Default world preset name used when MCRS_WORLD_PRESET is not set
const DEFAULT_WORLD_PRESET: &str = "normal";

/// Valid world preset names that are supported
const VALID_PRESETS: &[&str] = &[
    "normal",
    "flat",
    "amplified",
    "large_biomes",
    "single_biome_surface",
    "debug_all_block_states",
];

pub(crate) struct ConfigurationStatePlugin;

impl Plugin for ConfigurationStatePlugin {
    fn build(&self, app: &mut App) {
        app.add_systems(bevy_app::FixedPreUpdate, on_configuration_enter);
        app.insert_resource(SyncedRegistries(init_synced_registries()));
        app.insert_resource(LoadedDimensionTypes(init_dimension_types()));
        app.insert_resource(LoadedBiomes(init_biomes()));
        app.add_observer(on_configuration_ack);
    }
}

fn on_configuration_enter(
    mut query: Query<
        (Entity, &mut ServerSideConnection, &ConnectionState),
        Changed<ConnectionState>,
    >,
    res: Res<SyncedRegistries>,
    dimension_types: Res<LoadedDimensionTypes>,
    biomes: Res<LoadedBiomes>,
) {
    for (entity, mut con, conn_state) in query.iter_mut() {
        if *conn_state != ConnectionState::Configuration {
            continue;
        }

        con.write_packet(&ClientboundSelectKnownPacks {
            known_packs: vec![KnownPack {
                namespace: "minecraft",
                id: "core",
                version: VERSION_ID,
            }],
        });

        let requeried_regs = vec!["variant", "damage_type"];
        for (registry_id, entries) in &res.0 {
            if !requeried_regs
                .iter()
                .any(|r| registry_id.path().contains(r))
            {
                continue;
            }

            let packet_entries = entries
                .iter()
                .map(|name| Entry {
                    id: Cow::from(name.as_str()).try_into().unwrap(),
                    data: None,
                })
                .collect::<Vec<_>>();

            let packet = ClientboundRegistryData {
                registry: Cow::from(registry_id.as_str()).try_into().unwrap(),
                entries: packet_entries,
            };
            println!("sending registry data {:?}", &packet);
            con.write_packet(&packet);
        }

        // Send loaded dimension types to client
        let dim_entries: Vec<Entry> = dimension_types
            .0
            .iter()
            .map(|(id, dim_type)| {
                let dim_nbt = nbt::to_nbt_compound(dim_type)
                    .expect(&format!("Failed to serialize dimension type: {}", id));
                Entry {
                    id: Cow::from(id.as_str()).try_into().unwrap(),
                    data: Some(Cow::Owned(dim_nbt)),
                }
            })
            .collect();
        con.write_packet(&ClientboundRegistryData {
            registry: ident!("minecraft:dimension_type").into(),
            entries: dim_entries,
        });

        // Send loaded biomes to client
        let biome_entries: Vec<Entry> = biomes
            .0
            .iter()
            .map(|(id, biome)| {
                let biome_nbt = nbt::to_nbt_compound(biome)
                    .expect(&format!("Failed to serialize biome: {}", id));
                Entry {
                    id: Cow::from(id.as_str()).try_into().unwrap(),
                    data: Some(Cow::Owned(biome_nbt)),
                }
            })
            .collect();
        con.write_packet(&ClientboundRegistryData {
            registry: ident!("minecraft:worldgen/biome").into(),
            entries: biome_entries,
        });

        con.write_packet(&ClientboundFinishConfiguration)
    }
}

fn on_configuration_ack(
    event: On<ReceivedPacketEvent>,
    mut query: Query<(Entity, &mut ConnectionState)>,
    mut commands: Commands,
) {
    let Ok((entity, mut state)) = query.get_mut(event.entity) else {
        return;
    };
    if *state != ConnectionState::Configuration {
        return;
    }
    let Some(_) = event.decode::<ServerboundFinishConfiguration>() else {
        return;
    };
    *state = ConnectionState::Game;
    commands.entity(entity).insert(InGameConnectionState);
}

#[derive(Default, Resource)]
struct SyncedRegistries(Vec<(Ident<String>, Vec<Ident<String>>)>);

#[derive(Default, Resource)]
pub(crate) struct LoadedDimensionTypes(pub Vec<(Ident<String>, DimensionType)>);

/// List of known dimension types to load from individual JSON files
const DIMENSION_TYPE_FILES: &[(&str, &str)] = &[
    ("minecraft:overworld", include_str!("../../../assets/minecraft/dimension_type/overworld.json")),
    ("minecraft:overworld_caves", include_str!("../../../assets/minecraft/dimension_type/overworld_caves.json")),
    ("minecraft:the_end", include_str!("../../../assets/minecraft/dimension_type/the_end.json")),
    ("minecraft:the_nether", include_str!("../../../assets/minecraft/dimension_type/the_nether.json")),
];

fn init_dimension_types() -> Vec<(Ident<String>, DimensionType)> {
    DIMENSION_TYPE_FILES
        .iter()
        .map(|(name, json_content)| {
            let dim_type: DimensionType = serde_json::from_str(json_content)
                .expect(&format!("Failed to parse dimension type: {}", name));
            let id = Ident::from_str(name).unwrap();
            (id, dim_type)
        })
        .collect()
}

#[derive(Default, Resource)]
pub(crate) struct LoadedBiomes(pub Vec<(Ident<String>, Biome)>);

fn init_biomes() -> Vec<(Ident<String>, Biome)> {
    let synced_registries = include_str!("../../../assets/synced_registries.json");
    let json = serde_json::from_str::<Map<String, Value>>(synced_registries).unwrap();
    let biome_registry = json
        .get("worldgen/biome")
        .expect("worldgen/biome registry not found in synced_registries.json")
        .as_object()
        .expect("worldgen/biome should be an object");

    biome_registry
        .iter()
        .map(|(name, value)| {
            let biome: Biome = serde_json::from_value(value.clone())
                .expect(&format!("Failed to parse biome: {}", name));
            let id = Ident::from_str(name).unwrap();
            (id, biome)
        })
        .collect()
}

fn init_synced_registries() -> Vec<(Ident<String>, Vec<Ident<String>>)> {
    let synced_registries = include_str!("../../../assets/synced_registries.json");
    let json = serde_json::from_str::<Map<String, Value>>(synced_registries).unwrap();
    json.iter()
        .map(|(registry_id, registry)| {
            let registry = registry.as_object().unwrap();
            let entries = registry
                .iter()
                .map(|(name, _value)| Ident::from_str(name).unwrap())
                .collect::<Vec<_>>();
            let registry_id: Ident<String> = Ident::from_str(registry_id).unwrap();
            (registry_id, entries)
        })
        .collect::<Vec<_>>()
}

/// Represents a dimension entry within a world preset
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldPresetDimensionEntry {
    /// Reference to the dimension type (e.g., "minecraft:overworld")
    #[serde(rename = "type")]
    pub dimension_type: String,
    /// Generator configuration (kept as raw Value since we don't need to parse it)
    #[serde(default)]
    pub generator: Value,
}

/// Represents a world preset loaded from assets/minecraft/worldgen/world_preset/{preset}.json
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WorldPreset {
    /// Map of dimension key to dimension entry
    pub dimensions: HashMap<String, WorldPresetDimensionEntry>,
}

impl WorldPreset {
    /// Returns an ordered list of dimension entries as (dimension_key, dimension_type_ref) tuples.
    /// Order is deterministic: sorted alphabetically by dimension key.
    pub fn ordered_dimensions(&self) -> Vec<(Ident<String>, Ident<String>)> {
        let mut dims: Vec<_> = self
            .dimensions
            .iter()
            .map(|(key, entry)| {
                let dim_key = Ident::from_str(key).expect(&format!("Invalid dimension key: {}", key));
                let dim_type = Ident::from_str(&entry.dimension_type)
                    .expect(&format!("Invalid dimension type: {}", entry.dimension_type));
                (dim_key, dim_type)
            })
            .collect();
        // Sort by dimension key for deterministic ordering
        dims.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));
        dims
    }
}

/// Parse a world preset from the embedded assets
pub fn parse_world_preset(preset_name: &str) -> Option<WorldPreset> {
    // Load the preset JSON based on preset name
    let json_content = match preset_name {
        "normal" => include_str!("../../../assets/minecraft/worldgen/world_preset/normal.json"),
        "flat" => include_str!("../../../assets/minecraft/worldgen/world_preset/flat.json"),
        "amplified" => include_str!("../../../assets/minecraft/worldgen/world_preset/amplified.json"),
        "large_biomes" => include_str!("../../../assets/minecraft/worldgen/world_preset/large_biomes.json"),
        "single_biome_surface" => include_str!("../../../assets/minecraft/worldgen/world_preset/single_biome_surface.json"),
        "debug_all_block_states" => include_str!("../../../assets/minecraft/worldgen/world_preset/debug_all_block_states.json"),
        _ => {
            eprintln!("Unknown world preset: '{}', falling back to 'normal'", preset_name);
            include_str!("../../../assets/minecraft/worldgen/world_preset/normal.json")
        }
    };

    match serde_json::from_str::<WorldPreset>(json_content) {
        Ok(preset) => Some(preset),
        Err(e) => {
            eprintln!("Failed to parse world preset '{}': {}", preset_name, e);
            None
        }
    }
}

/// Get the world preset name from the MCRS_WORLD_PRESET environment variable.
/// Returns the default 'normal' preset if not set or invalid.
pub fn get_world_preset_name() -> String {
    match env::var("MCRS_WORLD_PRESET") {
        Ok(preset_name) => {
            let preset_name = preset_name.trim().to_lowercase();

            if preset_name.is_empty() {
                println!("MCRS_WORLD_PRESET is empty, using default preset: '{}'", DEFAULT_WORLD_PRESET);
                return DEFAULT_WORLD_PRESET.to_string();
            }

            if VALID_PRESETS.contains(&preset_name.as_str()) {
                println!("Loading world preset from MCRS_WORLD_PRESET: '{}'", preset_name);
                preset_name
            } else {
                eprintln!(
                    "Invalid world preset '{}' specified in MCRS_WORLD_PRESET. Valid presets: {:?}. Falling back to '{}'",
                    preset_name,
                    VALID_PRESETS,
                    DEFAULT_WORLD_PRESET
                );
                DEFAULT_WORLD_PRESET.to_string()
            }
        }
        Err(_) => {
            println!("MCRS_WORLD_PRESET not set, using default preset: '{}'", DEFAULT_WORLD_PRESET);
            DEFAULT_WORLD_PRESET.to_string()
        }
    }
}

/// Load the world preset based on the MCRS_WORLD_PRESET environment variable.
/// Returns the parsed WorldPreset or None if loading fails.
pub fn load_world_preset_from_env() -> Option<WorldPreset> {
    let preset_name = get_world_preset_name();

    match parse_world_preset(&preset_name) {
        Some(preset) => {
            println!(
                "Successfully loaded world preset '{}' with {} dimensions",
                preset_name,
                preset.dimensions.len()
            );
            Some(preset)
        }
        None => {
            eprintln!(
                "Failed to load world preset '{}', attempting fallback to '{}'",
                preset_name,
                DEFAULT_WORLD_PRESET
            );

            // Try to load the default preset as a fallback
            if preset_name != DEFAULT_WORLD_PRESET {
                match parse_world_preset(DEFAULT_WORLD_PRESET) {
                    Some(preset) => {
                        println!(
                            "Loaded fallback world preset '{}' with {} dimensions",
                            DEFAULT_WORLD_PRESET,
                            preset.dimensions.len()
                        );
                        Some(preset)
                    }
                    None => {
                        eprintln!("CRITICAL: Failed to load default world preset '{}'", DEFAULT_WORLD_PRESET);
                        None
                    }
                }
            } else {
                None
            }
        }
    }
}
