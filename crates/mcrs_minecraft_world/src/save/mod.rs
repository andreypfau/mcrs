use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Arc;

use mcrs_minecraft_core::ResourceLocation;
use serde::Deserialize;
use serde::de::DeserializeOwned;
use uuid::Uuid;

use crate::world_clock::ClockState;

pub const WORLD_VERSION: i32 = 5011;

#[derive(Debug, thiserror::Error)]
pub enum SaveError {
    #[error("{path}: no such file")]
    Missing { path: PathBuf },
    #[error("{path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("{path}: {source}")]
    Nbt {
        path: PathBuf,
        source: mcrs_minecraft_nbt::Error,
    },
    #[error("{path}: DataVersion {found}, expected {expected}")]
    DataVersion {
        path: PathBuf,
        found: i32,
        expected: i32,
    },
    #[error("{path}: `{field}` is {value}, expected {expected}")]
    OutOfRange {
        path: PathBuf,
        field: &'static str,
        value: String,
        expected: &'static str,
    },
    #[error("{path}: `{field}` has {found} elements, expected {expected}")]
    WrongLength {
        path: PathBuf,
        field: &'static str,
        found: usize,
        expected: usize,
    },
}

#[derive(Debug, Clone, PartialEq)]
pub struct RespawnData {
    pub dimension: String,
    pub pos: [i32; 3],
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for RespawnData {
    fn default() -> Self {
        Self {
            dimension: "minecraft:overworld".to_string(),
            pos: [0, 0, 0],
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct LevelDat {
    pub level_name: String,
    pub time: i64,
    pub singleplayer_uuid: Option<Uuid>,
    pub spawn: RespawnData,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
pub struct WeatherData {
    pub clear_weather_time: i32,
    pub rain_time: i32,
    pub thunder_time: i32,
    pub raining: bool,
    pub thundering: bool,
}

/// Only the rules with a consumer are named. The rule set is a registry we
/// implement incrementally, so an unknown key is ignored rather than rejected.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(default)]
pub struct GameRules {
    #[serde(rename = "minecraft:advance_time")]
    pub advance_time: bool,
}

impl Default for GameRules {
    fn default() -> Self {
        Self { advance_time: true }
    }
}

#[derive(Debug, Clone, PartialEq)]
pub struct PlayerData {
    pub pos: [f64; 3],
    pub yaw: f32,
    pub pitch: f32,
    pub dimension: String,
}

pub type WorldClockStates = HashMap<ResourceLocation<Arc<str>>, ClockState>;

pub fn read_level_dat(world: &Path) -> Result<LevelDat, SaveError> {
    let path = world.join("level.dat");
    parse_level_dat(&read_bytes(&path)?, &path)
}

pub fn read_world_clocks(world: &Path) -> Result<WorldClockStates, SaveError> {
    let path = saved_data_path(world, "world_clocks");
    parse_world_clocks(&read_bytes(&path)?, &path)
}

pub fn read_weather(world: &Path) -> Result<WeatherData, SaveError> {
    let path = saved_data_path(world, "weather");
    parse_weather(&read_bytes(&path)?, &path)
}

pub fn read_game_rules(world: &Path) -> Result<GameRules, SaveError> {
    let path = saved_data_path(world, "game_rules");
    parse_game_rules(&read_bytes(&path)?, &path)
}

pub fn read_player(world: &Path, player: Uuid) -> Result<PlayerData, SaveError> {
    let path = world
        .join("players/data")
        .join(format!("{}.dat", player.hyphenated()));
    parse_player(&read_bytes(&path)?, &path)
}

fn saved_data_path(world: &Path, name: &str) -> PathBuf {
    world.join("data/minecraft").join(format!("{name}.dat"))
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, SaveError> {
    fs::read(path).map_err(|source| match source.kind() {
        std::io::ErrorKind::NotFound => SaveError::Missing {
            path: path.to_path_buf(),
        },
        _ => SaveError::Io {
            path: path.to_path_buf(),
            source,
        },
    })
}

fn decode<T: DeserializeOwned>(bytes: &[u8], path: &Path) -> Result<T, SaveError> {
    mcrs_minecraft_nbt::nbt_compress::from_gzip_bytes(bytes).map_err(|source| SaveError::Nbt {
        path: path.to_path_buf(),
        source,
    })
}

fn check_data_version(found: i32, path: &Path) -> Result<(), SaveError> {
    if found == WORLD_VERSION {
        return Ok(());
    }
    Err(SaveError::DataVersion {
        path: path.to_path_buf(),
        found,
        expected: WORLD_VERSION,
    })
}

fn fixed<T: Copy, const N: usize>(
    values: &[T],
    field: &'static str,
    path: &Path,
) -> Result<[T; N], SaveError> {
    values.try_into().map_err(|_| SaveError::WrongLength {
        path: path.to_path_buf(),
        field,
        found: values.len(),
        expected: N,
    })
}

fn in_range(
    value: f32,
    min: f32,
    max: f32,
    field: &'static str,
    expected: &'static str,
    path: &Path,
) -> Result<f32, SaveError> {
    if (min..=max).contains(&value) {
        return Ok(value);
    }
    Err(SaveError::OutOfRange {
        path: path.to_path_buf(),
        field,
        value: value.to_string(),
        expected,
    })
}

#[derive(Deserialize)]
struct LevelDatFile {
    #[serde(rename = "Data")]
    data: RawLevelData,
}

#[derive(Deserialize)]
struct RawLevelData {
    #[serde(rename = "DataVersion")]
    data_version: i32,
    #[serde(rename = "LevelName")]
    level_name: String,
    #[serde(rename = "Time", default)]
    time: i64,
    singleplayer_uuid: Option<Vec<i32>>,
    spawn: Option<RawRespawnData>,
}

#[derive(Deserialize)]
struct RawRespawnData {
    dimension: String,
    pos: Vec<i32>,
    yaw: f32,
    pitch: f32,
}

#[derive(Deserialize)]
struct SavedDataFile<T> {
    data: T,
    #[serde(rename = "DataVersion")]
    data_version: i32,
}

#[derive(Deserialize)]
struct RawPlayerData {
    #[serde(rename = "DataVersion")]
    data_version: i32,
    #[serde(rename = "Pos")]
    pos: Vec<f64>,
    #[serde(rename = "Rotation")]
    rotation: Vec<f32>,
    #[serde(rename = "Dimension")]
    dimension: String,
}

fn parse_level_dat(bytes: &[u8], path: &Path) -> Result<LevelDat, SaveError> {
    let raw: LevelDatFile = decode(bytes, path)?;
    let raw = raw.data;
    check_data_version(raw.data_version, path)?;

    let singleplayer_uuid = raw
        .singleplayer_uuid
        .map(|ints| uuid_from_int_array(&ints, path))
        .transpose()?;

    let spawn = match raw.spawn {
        None => RespawnData::default(),
        Some(spawn) => RespawnData {
            dimension: spawn.dimension,
            pos: fixed(&spawn.pos, "spawn.pos", path)?,
            yaw: in_range(spawn.yaw, -180.0, 180.0, "spawn.yaw", "[-180, 180]", path)?,
            pitch: in_range(spawn.pitch, -90.0, 90.0, "spawn.pitch", "[-90, 90]", path)?,
        },
    };

    Ok(LevelDat {
        level_name: raw.level_name,
        time: raw.time,
        singleplayer_uuid,
        spawn,
    })
}

fn parse_world_clocks(bytes: &[u8], path: &Path) -> Result<WorldClockStates, SaveError> {
    let file: SavedDataFile<WorldClockStates> = decode(bytes, path)?;
    check_data_version(file.data_version, path)?;
    for (clock, state) in &file.data {
        if !(state.rate > 0.0 && state.rate <= f32::MAX) {
            return Err(SaveError::OutOfRange {
                path: path.to_path_buf(),
                field: "rate",
                value: format!("{} on `{clock}`", state.rate),
                expected: "> 0 and finite",
            });
        }
    }
    Ok(file.data)
}

fn parse_weather(bytes: &[u8], path: &Path) -> Result<WeatherData, SaveError> {
    let file: SavedDataFile<WeatherData> = decode(bytes, path)?;
    check_data_version(file.data_version, path)?;
    Ok(file.data)
}

fn parse_game_rules(bytes: &[u8], path: &Path) -> Result<GameRules, SaveError> {
    let file: SavedDataFile<GameRules> = decode(bytes, path)?;
    check_data_version(file.data_version, path)?;
    Ok(file.data)
}

fn parse_player(bytes: &[u8], path: &Path) -> Result<PlayerData, SaveError> {
    let raw: RawPlayerData = decode(bytes, path)?;
    check_data_version(raw.data_version, path)?;
    let [yaw, pitch]: [f32; 2] = fixed(&raw.rotation, "Rotation", path)?;
    Ok(PlayerData {
        pos: fixed(&raw.pos, "Pos", path)?,
        yaw,
        pitch,
        dimension: raw.dimension,
    })
}

fn uuid_from_int_array(ints: &[i32], path: &Path) -> Result<Uuid, SaveError> {
    let parts: [i32; 4] = fixed(ints, "singleplayer_uuid", path)?;
    let value = parts
        .into_iter()
        .fold(0u128, |acc, part| (acc << 32) | u128::from(part as u32));
    Ok(Uuid::from_u128(value))
}

#[cfg(test)]
mod tests;
