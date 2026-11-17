use std::cmp::Ordering;
use std::fs;
use std::path::Path;
use std::time::UNIX_EPOCH;

use mcrs_minecraft_core::VERSION;
use serde::Deserialize;
use uuid::Uuid;

use crate::LEVEL_FORMAT_VERSION;
use crate::SaveError;
use crate::SaveLock;
use crate::level::decode;

const CONVERSION_STORAGE_VERSION: i32 = 19132;

const VANILLA_FEATURE: &str = "minecraft:vanilla";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct WorldSummary {
    pub folder_id: String,
    pub last_played: Option<i64>,
    pub locked: bool,
    pub level: Option<ListedLevel>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedLevel {
    pub level_name: String,
    pub version_name: String,
    pub world_version: i32,
    pub storage_version: i32,
    pub game_type: i32,
    pub hardcore: bool,
    pub allow_commands: bool,
    pub experimental: bool,
    pub host: Option<Uuid>,
}

impl WorldSummary {
    #[must_use]
    pub fn name(&self) -> &str {
        match &self.level {
            Some(level) if !level.level_name.is_empty() => &level.level_name,
            _ => &self.folder_id,
        }
    }

    #[must_use]
    pub fn is_incompatible(&self) -> bool {
        self.level.as_ref().is_some_and(|level| {
            level.world_version != VERSION.world_version
                || level.storage_version == CONVERSION_STORAGE_VERSION
                || level.experimental
        })
    }

    #[must_use]
    pub fn is_disabled(&self) -> bool {
        self.locked || self.level.is_none() || self.is_incompatible()
    }
}

pub fn read_saves(saves: &Path) -> Result<Vec<WorldSummary>, SaveError> {
    let io = |source| SaveError::Io {
        path: saves.to_owned(),
        source,
    };
    fs::create_dir_all(saves).map_err(io)?;
    let mut worlds = Vec::new();
    for entry in fs::read_dir(saves).map_err(io)? {
        let entry = entry.map_err(io)?;
        let Ok(folder_id) = entry.file_name().into_string() else {
            continue;
        };
        if let Some(world) = summary(&entry.path(), folder_id) {
            worlds.push(world);
        }
    }
    worlds.sort_by(newest_first);
    Ok(worlds)
}

fn newest_first(a: &WorldSummary, b: &WorldSummary) -> Ordering {
    b.last_played
        .unwrap_or(0)
        .cmp(&a.last_played.unwrap_or(0))
        .then_with(|| a.folder_id.encode_utf16().cmp(b.folder_id.encode_utf16()))
}

fn summary(dir: &Path, folder_id: String) -> Option<WorldSummary> {
    let file = ["level.dat", "level.dat_old"]
        .into_iter()
        .map(|name| dir.join(name))
        .find(|path| fs::metadata(path).is_ok_and(|metadata| metadata.is_file()))?;
    let locked = SaveLock::is_held(dir).ok()?;
    let level_dat = dir.join("level.dat");
    let parsed = fs::read(&level_dat)
        .ok()
        .and_then(|bytes| decode::<SummaryFile>(&bytes, &level_dat).ok())
        .and_then(|summary| summary.data.into_level());
    let (level, last_played) = match parsed {
        Some(Played { level, last_played }) => (Some(level), Some(last_played)),
        None => (None, modified_millis(&file)),
    };
    Some(WorldSummary {
        folder_id,
        last_played,
        locked,
        level,
    })
}

fn modified_millis(file: &Path) -> Option<i64> {
    let modified = fs::metadata(file).ok()?.modified().ok()?;
    let since = modified.duration_since(UNIX_EPOCH).ok()?;
    i64::try_from(since.as_millis()).ok()
}

struct Played {
    level: ListedLevel,
    last_played: i64,
}

#[derive(Deserialize)]
struct SummaryFile {
    #[serde(rename = "Data")]
    data: SummaryData,
}

#[derive(Deserialize)]
struct SummaryData {
    #[serde(rename = "version")]
    storage_version: Option<i32>,
    #[serde(rename = "LastPlayed", default)]
    last_played: i64,
    #[serde(rename = "Version")]
    version: Option<SummaryVersion>,
    #[serde(rename = "DataVersion")]
    data_version: Option<i32>,
    #[serde(rename = "LevelName", default)]
    level_name: String,
    #[serde(rename = "GameType", default)]
    game_type: i32,
    difficulty_settings: Option<SummaryDifficulty>,
    #[serde(rename = "allowCommands")]
    allow_commands: Option<bool>,
    enabled_features: Option<Vec<String>>,
    singleplayer_uuid: Option<Vec<i32>>,
}

#[derive(Deserialize)]
struct SummaryVersion {
    #[serde(rename = "Name", default)]
    name: String,
    #[serde(rename = "Id")]
    id: Option<i32>,
}

#[derive(Deserialize)]
struct SummaryDifficulty {
    #[serde(default)]
    hardcore: bool,
}

impl SummaryData {
    fn into_level(self) -> Option<Played> {
        let storage_version = self.storage_version.filter(|version| {
            *version == LEVEL_FORMAT_VERSION || *version == CONVERSION_STORAGE_VERSION
        })?;
        let version_id = self.version.as_ref().and_then(|version| version.id);
        let game_type = if (0..=3).contains(&self.game_type) {
            self.game_type
        } else {
            0
        };
        let level = ListedLevel {
            level_name: self.level_name,
            version_name: self.version.map(|version| version.name).unwrap_or_default(),
            world_version: self.data_version.or(version_id).unwrap_or(0),
            storage_version,
            game_type,
            hardcore: self
                .difficulty_settings
                .is_some_and(|difficulty| difficulty.hardcore),
            allow_commands: self.allow_commands.unwrap_or(game_type == 1),
            experimental: self
                .enabled_features
                .is_some_and(|features| features.iter().any(|flag| flag != VANILLA_FEATURE)),
            host: self
                .singleplayer_uuid
                .and_then(|ints| uuid_from_ints(&ints)),
        };
        Some(Played {
            level,
            last_played: self.last_played,
        })
    }
}

fn uuid_from_ints(ints: &[i32]) -> Option<Uuid> {
    let parts: [i32; 4] = ints.try_into().ok()?;
    let value = parts.into_iter().fold(0u128, |acc, part| {
        (acc << 32) | u128::from(part.cast_unsigned())
    });
    Some(Uuid::from_u128(value))
}
