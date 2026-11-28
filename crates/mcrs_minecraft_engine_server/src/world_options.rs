use bevy_app::AppExit;
use bevy_ecs::prelude::{Commands, MessageWriter, ResMut};
use bevy_ecs::resource::Resource;
use bevy_ecs::system::Res;
use std::path::Path;
use std::time::{SystemTime, UNIX_EPOCH};

use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_engine::coordinator::default_game_mode::DefaultGameMode;
use mcrs_minecraft_engine::dimension_generators::DimensionGenerators;
use mcrs_minecraft_engine_world::world::dimension_list::DimensionList;
use mcrs_minecraft_engine_world::world::world_save::WorldSave;
use mcrs_minecraft_engine_world::world_seed::WorldSeed;
use mcrs_minecraft_game_mode::GameMode;
use mcrs_minecraft_game_registries::dimension::{DimensionEntry, Dimensions, bake, bake_list};
use mcrs_minecraft_game_registries::worldgen::world_preset::WorldPreset;
use mcrs_minecraft_protocol::uuid::Uuid;
use mcrs_minecraft_registries::{CurrentRegistries, Registries};
use mcrs_minecraft_resource::{ResourceKeyBuf, ResourceLocationBuf};
use mcrs_minecraft_save::{
    LevelDat, LevelVersion, RespawnData, SaveError, SaveLock, WorldGenSettings, read_level_dat,
    read_level_data_version, read_world_gen_settings, restore_level_dat_backup, write_level_dat,
    write_world_gen_settings,
};

use crate::WorldOpenError;
use tracing::{error, info};

#[derive(Resource)]
pub struct WorldLock(pub SaveLock);

#[derive(Resource, Debug)]
pub struct FailedStart(pub WorldOpenError);

#[derive(Resource, Clone)]
pub(crate) struct PluginDimensions(pub Dimensions);

#[derive(Resource)]
pub(crate) struct WorldPresetName(pub String);

#[derive(Resource, Clone, Copy)]
pub(crate) struct WorldHost(pub Uuid);

pub(crate) fn bake_dimensions(
    mut commands: Commands,
    registries: Res<CurrentRegistries>,
    save: Option<Res<WorldSave>>,
    given: Option<Res<PluginDimensions>>,
    preset: Res<WorldPresetName>,
    host: Option<Res<WorldHost>>,
    mut seed: ResMut<WorldSeed>,
    mut game_mode: ResMut<DefaultGameMode>,
    mut exit: MessageWriter<AppExit>,
) {
    let baked = dimension_list(
        &registries,
        save.as_deref(),
        given.as_deref(),
        &preset.0,
        host.map(|host| host.0),
        &mut seed,
        &mut game_mode,
    );
    match baked {
        Ok((source, list)) => {
            info!(%source, dimensions = list.len(), "dimension list");
            commands.insert_resource(DimensionList::new(
                list.iter()
                    .map(|(key, entry)| (key.clone(), entry.dimension_type)),
            ));
            commands.insert_resource(DimensionGenerators(
                list.into_iter().map(|(_, entry)| entry.generator).collect(),
            ));
        }
        Err(error) => {
            error!("the world does not open:\n{error}");
            commands.remove_resource::<WorldSave>();
            commands.remove_resource::<WorldLock>();
            commands.insert_resource(FailedStart(error));
            exit.write(AppExit::error());
        }
    }
}

fn dimension_list(
    registries: &Registries,
    save: Option<&WorldSave>,
    given: Option<&PluginDimensions>,
    preset: &str,
    host: Option<Uuid>,
    seed: &mut WorldSeed,
    game_mode: &mut DefaultGameMode,
) -> Result<(String, Vec<(ResourceKeyBuf<Dimension>, DimensionEntry)>), WorldOpenError> {
    let saved = match save {
        Some(save) => {
            let new_world = NewWorld {
                dimensions: given.map(|given| &given.0),
                preset,
                seed: seed.0.cast_signed(),
                game_mode: game_mode.0,
            };
            let opened = open_world(&save.0, registries, &new_world, host)?;
            seed.0 = opened.seed.cast_unsigned();
            if let Some(mode) = opened.game_mode {
                game_mode.0 = mode;
            }
            Some(opened.dimensions)
        }
        None => None,
    };
    if let Some(given) = given {
        let list = bake_list(&given.0, registries).map_err(WorldOpenError::Dimensions)?;
        Ok(("plugin".to_owned(), list))
    } else if let Some(saved) = saved {
        let list = bake(&saved, registries).map_err(WorldOpenError::Dimensions)?;
        Ok(("save".to_owned(), list))
    } else {
        let list = bake(preset_dimensions(registries, preset)?, registries)
            .map_err(WorldOpenError::Dimensions)?;
        Ok((preset.to_owned(), list))
    }
}

fn preset_dimensions<'r>(
    registries: &'r Registries,
    preset: &str,
) -> Result<&'r Dimensions, WorldOpenError> {
    registries
        .registry::<WorldPreset>()
        .and_then(|presets| presets.get(presets.by_name(&ResourceLocationBuf::read(preset).ok()?)?))
        .map(|named| &named.dimensions)
        .ok_or_else(|| {
            WorldOpenError::Dimensions(format!(
                "{}: no world preset is named {preset}",
                mcrs_minecraft_game_registries::registry_keys::WORLD_PRESET
            ))
        })
}

struct NewWorld<'a> {
    dimensions: Option<&'a Dimensions>,
    preset: &'a str,
    seed: i64,
    game_mode: GameMode,
}

struct OpenedWorld {
    seed: i64,
    dimensions: Dimensions,
    game_mode: Option<GameMode>,
}

fn open_world(
    dir: &Path,
    registries: &Registries,
    new_world: &NewWorld,
    host: Option<Uuid>,
) -> Result<OpenedWorld, WorldOpenError> {
    let level = match read_level_data_version(dir) {
        Ok(Some(found)) if found == VERSION.world_version => {
            restore_level_dat_backup(dir)?;
            Some(read_level_dat(dir)?)
        }
        Ok(found) => return Err(version_error(dir, found)),
        Err(SaveError::Missing { .. }) => None,
        Err(error) => return Err(error.into()),
    };
    let settings = match read_world_gen_settings(dir, registries) {
        Ok(settings) => settings,
        Err(SaveError::Missing { .. }) if level.is_none() => {
            let dimensions = match new_world.dimensions {
                Some(given) => given.clone(),
                None => preset_dimensions(registries, new_world.preset)?.clone(),
            };
            let settings = WorldGenSettings {
                seed: new_world.seed,
                dimensions,
                bonus_chest: false,
                generate_structures: true,
                legacy_custom_options: None,
            };
            write_world_gen_settings(dir, &settings, registries)?;
            settings
        }
        Err(error) => return Err(error.into()),
    };
    let mut level = match level {
        Some(level) => level,
        None => {
            let level = new_level(dir, new_world.game_mode);
            write_level_dat(dir, &level)?;
            level
        }
    };
    let first_start = !level.initialized;
    if first_start {
        level.spawn = RespawnData::default();
        level.initialized = true;
    }
    let new_host = host.filter(|&host| level.singleplayer_uuid != Some(host));
    level.singleplayer_uuid = new_host.or(level.singleplayer_uuid);
    if first_start || new_host.is_some() {
        write_level_dat(dir, &level)?;
    }
    Ok(OpenedWorld {
        seed: settings.seed,
        dimensions: settings.dimensions,
        game_mode: GameMode::from_id(level.game_type),
    })
}

fn version_error(dir: &Path, found: Option<i32>) -> WorldOpenError {
    WorldOpenError::DataVersion {
        dir: dir.to_owned(),
        found,
        expected: VERSION.world_version,
    }
}

fn new_level(dir: &Path, game_mode: GameMode) -> LevelDat {
    let last_played = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .ok()
        .and_then(|since| i64::try_from(since.as_millis()).ok())
        .unwrap_or_default();
    LevelDat {
        version: Some(LevelVersion {
            name: VERSION.name.to_owned(),
            id: VERSION.world_version,
            snapshot: !VERSION.stable,
            series: VERSION.series_id.to_owned(),
        }),
        level_name: dir
            .file_name()
            .map(|name| name.to_string_lossy().into_owned())
            .unwrap_or_default(),
        game_type: game_mode.id(),
        last_played,
        initialized: false,
        ..LevelDat::default()
    }
}
