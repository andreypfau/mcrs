use std::path::Path;

use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_game_registries::worldgen::world_preset::WorldPreset;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_resource::ResourceLocationBuf;

use crate::folder_id::make_free_folder;
use crate::{
    Difficulty, DifficultySettings, LevelDat, LevelVersion, SaveError, SaveLock, WorldGenSettings,
    seed_from_text, trim_name, write_level_dat, write_world_gen_settings,
};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NewWorld {
    pub name: String,
    pub game_mode: NewWorldMode,
    pub world_type: ResourceLocationBuf,
    pub seed_text: String,
    pub random_seed: i64,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum NewWorldMode {
    #[default]
    Survival,
    Hardcore,
    Creative,
}

pub fn create_world(
    saves: &Path,
    world: &NewWorld,
    registries: &Registries,
    last_played: i64,
) -> Result<SaveLock, SaveError> {
    let dimensions = registries
        .registry::<WorldPreset>()
        .and_then(|presets| presets.get(presets.by_name(&world.world_type)?))
        .map(|preset| preset.dimensions.clone())
        .ok_or_else(|| SaveError::UnknownWorldPreset {
            preset: world.world_type.clone(),
        })?;
    let lock = SaveLock::take(&saves.join(make_free_folder(saves, &world.name)?))?;
    let settings = WorldGenSettings {
        seed: seed_from_text(&world.seed_text).unwrap_or(world.random_seed),
        dimensions,
        bonus_chest: false,
        generate_structures: true,
        legacy_custom_options: None,
    };
    write_world_gen_settings(lock.dir(), &settings, registries)?;
    write_level_dat(lock.dir(), &level(world, last_played))?;
    Ok(lock)
}

fn level(world: &NewWorld, last_played: i64) -> LevelDat {
    let (game_type, difficulty, hardcore, allow_commands) = match world.game_mode {
        NewWorldMode::Survival => (0, Difficulty::Normal, false, true),
        NewWorldMode::Hardcore => (0, Difficulty::Hard, true, false),
        NewWorldMode::Creative => (1, Difficulty::Normal, false, true),
    };
    LevelDat {
        version: Some(LevelVersion {
            name: VERSION.name.to_owned(),
            id: VERSION.world_version,
            snapshot: !VERSION.stable,
            series: VERSION.series_id.to_owned(),
        }),
        level_name: trim_name(&world.name).to_owned(),
        last_played,
        game_type,
        difficulty_settings: DifficultySettings {
            difficulty,
            hardcore,
            locked: false,
        },
        allow_commands,
        initialized: false,
        ..LevelDat::default()
    }
}
