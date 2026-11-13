use mcrs_minecraft_resource::{ResourceKeyBuf, ResourceLocationBuf};
use std::collections::HashMap;
use std::fs;
use std::path::{Path, PathBuf};

use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_game_rule::{GameRule, GameRuleValueType};
use mcrs_minecraft_nbt::nbt_int_array;
use mcrs_minecraft_registries::{
    Named, Refs, Registries, Resolve, ResolveError, Resolved, registry_value,
};
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use uuid::Uuid;

use mcrs_minecraft_game_registries::dimension::DimensionEntry;

use mcrs_minecraft_dimension::Dimension;
use mcrs_minecraft_environment::ClockTime;

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
    #[error("{path}: {source}")]
    Resolve { path: PathBuf, source: ResolveError },
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
    pub dimension: ResourceKeyBuf<Dimension>,
    pub pos: [i32; 3],
    pub yaw: f32,
    pub pitch: f32,
}

impl Default for RespawnData {
    fn default() -> Self {
        Self {
            dimension: mcrs_minecraft_dimension::dimension_keys::OVERWORLD.into(),
            pos: [0, 0, 0],
            yaw: 0.0,
            pitch: 0.0,
        }
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Difficulty {
    Peaceful,
    Easy,
    #[default]
    Normal,
    Hard,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DifficultySettings {
    pub difficulty: Difficulty,
    pub hardcore: bool,
    pub locked: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DataPacks {
    #[serde(rename = "Enabled")]
    pub enabled: Vec<String>,
    #[serde(rename = "Disabled")]
    pub disabled: Vec<String>,
}

impl Default for DataPacks {
    fn default() -> Self {
        Self {
            enabled: vec!["vanilla".to_owned()],
            disabled: Vec::new(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LevelVersion {
    #[serde(rename = "Name")]
    pub name: String,
    #[serde(rename = "Id")]
    pub id: i32,
    #[serde(rename = "Snapshot")]
    pub snapshot: bool,
    #[serde(rename = "Series")]
    pub series: String,
}

/// The format number that the server writes in `version`.
pub const LEVEL_FORMAT_VERSION: i32 = 19133;

#[derive(Debug, Clone, PartialEq)]
pub struct LevelDat {
    pub version: Option<LevelVersion>,
    pub version_history: Vec<i32>,
    pub level_name: String,
    pub time: i64,
    pub last_played: i64,
    pub game_type: i32,
    pub spawn: RespawnData,
    pub difficulty_settings: DifficultySettings,
    pub allow_commands: bool,
    pub initialized: bool,
    pub data_packs: DataPacks,
    pub enabled_features: Vec<ResourceLocationBuf>,
    pub was_modded: bool,
    pub server_brands: Vec<String>,
    pub removed_features: Vec<ResourceLocationBuf>,
    pub singleplayer_uuid: Option<Uuid>,
}

impl Default for LevelDat {
    fn default() -> Self {
        Self {
            version: None,
            version_history: Vec::new(),
            level_name: String::new(),
            time: 0,
            last_played: 0,
            game_type: 0,
            spawn: RespawnData::default(),
            difficulty_settings: DifficultySettings::default(),
            allow_commands: false,
            initialized: true,
            data_packs: DataPacks::default(),
            enabled_features: default_features(),
            was_modded: false,
            server_brands: Vec::new(),
            removed_features: Vec::new(),
            singleplayer_uuid: None,
        }
    }
}

fn default_features() -> Vec<ResourceLocationBuf> {
    ResourceLocationBuf::read("minecraft:vanilla")
        .into_iter()
        .collect()
}

#[registry_value]
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WorldGenSettings<M: Refs = Resolved> {
    pub seed: i64,
    pub dimensions: std::collections::BTreeMap<ResourceKeyBuf<Dimension>, DimensionEntry<M>>,
    #[serde(default)]
    pub bonus_chest: bool,
    #[serde(default = "default_true")]
    pub generate_structures: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legacy_custom_options: Option<String>,
}

fn default_true() -> bool {
    true
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WeatherData {
    pub clear_weather_time: i32,
    pub rain_time: i32,
    pub thunder_time: i32,
    pub raining: bool,
    pub thundering: bool,
}

/// The game rules that a file sets, in rule order. A rule that the file does not set has its
/// default value.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct GameRules(Vec<(GameRule, i32)>);

impl GameRules {
    #[must_use]
    pub fn get(&self, rule: GameRule) -> Option<i32> {
        let index = self.0.binary_search_by_key(&rule, |&(set, _)| set).ok()?;
        self.0.get(index).map(|&(_, value)| value)
    }

    /// A bool rule keeps 0 or 1 and an int rule keeps a value in its range, so that every value
    /// written can be read back.
    pub fn set(&mut self, rule: GameRule, value: i32) {
        let value = match rule.definition().value {
            GameRuleValueType::Bool { .. } => i32::from(value != 0),
            GameRuleValueType::Int { min, max, .. } => value.clamp(min, max),
        };
        match self.0.binary_search_by_key(&rule, |&(set, _)| set) {
            Ok(index) => {
                if let Some(slot) = self.0.get_mut(index) {
                    slot.1 = value;
                }
            }
            Err(index) => self.0.insert(index, (rule, value)),
        }
    }

    pub fn iter(&self) -> impl Iterator<Item = (GameRule, i32)> + '_ {
        self.0.iter().copied()
    }
}

impl Serialize for GameRules {
    fn serialize<S: serde::Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        use serde::ser::SerializeMap;
        let mut map = serializer.serialize_map(Some(self.0.len()))?;
        for &(rule, value) in &self.0 {
            match rule.definition().value {
                GameRuleValueType::Bool { .. } => {
                    map.serialize_entry(rule.as_static_str(), &(value != 0))?;
                }
                GameRuleValueType::Int { .. } => {
                    map.serialize_entry(rule.as_static_str(), &value)?
                }
            }
        }
        map.end()
    }
}

enum RuleValue {
    Flag(bool),
    Byte(i8),
    Number(i64),
    Other,
}

impl<'de> Deserialize<'de> for RuleValue {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::{Error, Visitor};
        struct Raw;
        impl<'de> Visitor<'de> for Raw {
            type Value = RuleValue;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a game rule value")
            }

            fn visit_bool<E: Error>(self, value: bool) -> Result<RuleValue, E> {
                Ok(RuleValue::Flag(value))
            }

            fn visit_i8<E: Error>(self, value: i8) -> Result<RuleValue, E> {
                Ok(RuleValue::Byte(value))
            }

            fn visit_i32<E: Error>(self, value: i32) -> Result<RuleValue, E> {
                Ok(RuleValue::Number(i64::from(value)))
            }

            fn visit_i64<E: Error>(self, value: i64) -> Result<RuleValue, E> {
                Ok(RuleValue::Number(value))
            }

            fn visit_u64<E: Error>(self, value: u64) -> Result<RuleValue, E> {
                Ok(i64::try_from(value).map_or(RuleValue::Other, RuleValue::Number))
            }
        }
        deserializer.deserialize_any(Raw)
    }
}

impl<'de> Deserialize<'de> for GameRules {
    fn deserialize<D: serde::Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        use serde::de::{Error, MapAccess, Visitor};
        struct Rules;
        impl<'de> Visitor<'de> for Rules {
            type Value = GameRules;

            fn expecting(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
                f.write_str("a compound of game rules")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<GameRules, A::Error> {
                let mut rules = GameRules::default();
                while let Some(key) = map.next_key::<String>()? {
                    let Some(rule) = GameRule::read(&key) else {
                        return Err(A::Error::custom(format!("unknown field `{key}`")));
                    };
                    let value = match rule.definition().value {
                        GameRuleValueType::Bool { .. } => map
                            .next_value::<RuleValue>()
                            .ok()
                            .and_then(|raw| match raw {
                                RuleValue::Flag(flag) => Some(i32::from(flag)),
                                RuleValue::Byte(flag @ (0 | 1)) => Some(i32::from(flag)),
                                _ => None,
                            }),
                        GameRuleValueType::Int { min, max, .. } => map
                            .next_value::<RuleValue>()
                            .ok()
                            .and_then(|raw| match raw {
                                RuleValue::Number(value) => i32::try_from(value)
                                    .ok()
                                    .filter(|value| (min..=max).contains(value)),
                                _ => None,
                            }),
                    };
                    let Some(value) = value else {
                        return Err(A::Error::custom(format!(
                            "game rule `{key}` has a value of the wrong type or range"
                        )));
                    };
                    rules.set(rule, value);
                }
                Ok(rules)
            }
        }
        deserializer.deserialize_map(Rules)
    }
}

pub type WorldClockStates = HashMap<ResourceLocationBuf, ClockTime>;

pub fn read_level_dat(world: &Path) -> Result<LevelDat, SaveError> {
    let path = world.join("level.dat");
    parse_level_dat(&read_bytes(&path)?, &path)
}

pub fn write_level_dat(world: &Path, level: &LevelDat) -> Result<(), SaveError> {
    let path = world.join("level.dat");
    write_gzip_compound(&path, LevelDatFile::from(level))
}

pub fn read_world_clocks(world: &Path) -> Result<WorldClockStates, SaveError> {
    let path = saved_data_path(world, "world_clocks");
    parse_world_clocks(&read_bytes(&path)?, &path)
}

pub fn read_world_gen_settings(
    world: &Path,
    registries: &Registries,
) -> Result<WorldGenSettings, SaveError> {
    let path = saved_data_path(world, "world_gen_settings");
    parse_world_gen_settings(&read_bytes(&path)?, &path, registries)
}

pub fn write_world_gen_settings(
    world: &Path,
    settings: &WorldGenSettings,
    registries: &Registries,
) -> Result<(), SaveError> {
    let path = saved_data_path(world, "world_gen_settings");
    let data = settings
        .name(registries)
        .map_err(|source| SaveError::Resolve {
            path: path.clone(),
            source,
        })?;
    write_saved_data(&path, data)
}

/// Writes `data` in the saved data form through [`crate::write_atomic`].
fn write_saved_data<T: Serialize>(path: &Path, data: T) -> Result<(), SaveError> {
    let io = |source| SaveError::Io {
        path: path.to_path_buf(),
        source,
    };
    let file = SavedDataFile {
        data,
        data_version: VERSION.world_version,
    };
    let bytes = mcrs_minecraft_nbt::nbt_compress::to_gzip_bytes_vec(&file).map_err(|source| {
        SaveError::Nbt {
            path: path.to_path_buf(),
            source,
        }
    })?;
    crate::write_atomic(path, &bytes).map_err(io)
}

pub fn write_world_clocks(world: &Path, clocks: &WorldClockStates) -> Result<(), SaveError> {
    write_saved_data(&saved_data_path(world, "world_clocks"), clocks)
}

pub fn read_weather(world: &Path) -> Result<WeatherData, SaveError> {
    let path = saved_data_path(world, "weather");
    parse_weather(&read_bytes(&path)?, &path)
}

pub fn write_weather(world: &Path, weather: &WeatherData) -> Result<(), SaveError> {
    write_saved_data(&saved_data_path(world, "weather"), weather)
}

pub fn write_game_rules(world: &Path, rules: &GameRules) -> Result<(), SaveError> {
    write_saved_data(&saved_data_path(world, "game_rules"), rules)
}

pub fn read_game_rules(world: &Path) -> Result<GameRules, SaveError> {
    let path = saved_data_path(world, "game_rules");
    parse_game_rules(&read_bytes(&path)?, &path)
}

/// The birth clock of the world: the birth tick of the new entity UUIDs. It is data of mcrs, so it
/// has its own file next to the saved data of the game.
#[derive(Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct BirthClockData {
    tick: i64,
}

fn birth_clock_path(world: &Path) -> PathBuf {
    world.join("data/mcrs/birth_clock.dat")
}

/// The saved value of the birth clock; [`SaveError::Missing`] in a save without one.
pub fn read_birth_clock(world: &Path) -> Result<u64, SaveError> {
    let path = birth_clock_path(world);
    let data: BirthClockData = parse_saved_data(&read_bytes(&path)?, &path)?;
    Ok(data.tick.cast_unsigned())
}

pub fn write_birth_clock(world: &Path, tick: u64) -> Result<(), SaveError> {
    let data = BirthClockData {
        tick: tick.cast_signed(),
    };
    write_saved_data(&birth_clock_path(world), data)
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

pub(crate) fn decode<T: DeserializeOwned>(bytes: &[u8], path: &Path) -> Result<T, SaveError> {
    mcrs_minecraft_nbt::nbt_compress::from_gzip_bytes(bytes).map_err(|source| SaveError::Nbt {
        path: path.to_path_buf(),
        source,
    })
}

pub(crate) fn check_data_version(found: i32, path: &Path) -> Result<(), SaveError> {
    if found == VERSION.world_version {
        return Ok(());
    }
    Err(SaveError::DataVersion {
        path: path.to_path_buf(),
        found,
        expected: VERSION.world_version,
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

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct LevelDatFile {
    #[serde(rename = "Data")]
    data: RawLevelData,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawLevelData {
    #[serde(rename = "DataVersion")]
    data_version: i32,
    #[serde(rename = "Version", default, skip_serializing_if = "Option::is_none")]
    version: Option<LevelVersion>,
    #[serde(default)]
    version_history: Vec<i32>,
    #[serde(rename = "LevelName", default)]
    level_name: String,
    #[serde(rename = "Time", default)]
    time: i64,
    #[serde(rename = "LastPlayed", default)]
    last_played: i64,
    #[serde(rename = "GameType", default)]
    game_type: i32,
    #[serde(default)]
    spawn: Option<RawRespawnData>,
    #[serde(default)]
    difficulty_settings: Option<DifficultySettings>,
    #[serde(rename = "allowCommands", default)]
    allow_commands: Option<bool>,
    #[serde(default = "default_true")]
    initialized: bool,
    #[serde(default, rename = "version")]
    format_version: i32,
    #[serde(rename = "DataPacks", default)]
    data_packs: Option<DataPacks>,
    #[serde(default)]
    enabled_features: Option<Vec<ResourceLocationBuf>>,
    #[serde(rename = "WasModded", default)]
    was_modded: bool,
    #[serde(rename = "ServerBrands", default)]
    server_brands: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    removed_features: Vec<ResourceLocationBuf>,
    #[serde(
        default,
        skip_serializing_if = "Option::is_none",
        serialize_with = "int_array_option"
    )]
    singleplayer_uuid: Option<Vec<i32>>,
}

fn int_array_option<S: serde::Serializer>(
    value: &Option<Vec<i32>>,
    serializer: S,
) -> Result<S::Ok, S::Error> {
    match value {
        Some(ints) => nbt_int_array(ints, serializer),
        None => serializer.serialize_none(),
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawRespawnData {
    dimension: ResourceKeyBuf<Dimension>,
    #[serde(serialize_with = "nbt_int_array")]
    pos: Vec<i32>,
    yaw: f32,
    pitch: f32,
}

impl From<&LevelDat> for LevelDatFile {
    fn from(level: &LevelDat) -> Self {
        let mut version_history = level.version_history.clone();
        if version_history.last() != Some(&VERSION.world_version) {
            version_history.push(VERSION.world_version);
        }
        Self {
            data: RawLevelData {
                data_version: VERSION.world_version,
                version: level.version.clone(),
                version_history,
                level_name: level.level_name.clone(),
                time: level.time,
                last_played: level.last_played,
                game_type: level.game_type,
                spawn: Some(RawRespawnData {
                    dimension: level.spawn.dimension.clone(),
                    pos: level.spawn.pos.to_vec(),
                    yaw: level.spawn.yaw,
                    pitch: level.spawn.pitch,
                }),
                difficulty_settings: Some(level.difficulty_settings),
                allow_commands: Some(level.allow_commands),
                initialized: level.initialized,
                format_version: LEVEL_FORMAT_VERSION,
                data_packs: Some(level.data_packs.clone()),
                enabled_features: Some(level.enabled_features.clone()),
                was_modded: level.was_modded,
                server_brands: level.server_brands.clone(),
                removed_features: level.removed_features.clone(),
                singleplayer_uuid: level
                    .singleplayer_uuid
                    .map(uuid_to_int_array)
                    .map(Vec::from),
            },
        }
    }
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct SavedDataFile<T> {
    data: T,
    #[serde(rename = "DataVersion")]
    data_version: i32,
}

fn write_gzip_compound<T: Serialize>(path: &Path, data: T) -> Result<(), SaveError> {
    let bytes = mcrs_minecraft_nbt::nbt_compress::to_gzip_bytes_vec(&data).map_err(|source| {
        SaveError::Nbt {
            path: path.to_path_buf(),
            source,
        }
    })?;
    let io = |source| SaveError::Io {
        path: path.to_path_buf(),
        source,
    };
    crate::write_atomic(path, &bytes).map_err(io)
}

/// The four ints of the NBT form of a UUID, most significant first.
pub fn uuid_to_int_array(uuid: Uuid) -> [i32; 4] {
    let value = uuid.as_u128();
    [
        (value >> 96) as u32 as i32,
        (value >> 64) as u32 as i32,
        (value >> 32) as u32 as i32,
        value as u32 as i32,
    ]
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
    if !(0..=3).contains(&raw.game_type) {
        return Err(SaveError::OutOfRange {
            path: path.to_path_buf(),
            field: "GameType",
            value: raw.game_type.to_string(),
            expected: "0 through 3",
        });
    }
    if !matches!(raw.format_version, 0 | 19132 | 19133) {
        return Err(SaveError::OutOfRange {
            path: path.to_path_buf(),
            field: "version",
            value: raw.format_version.to_string(),
            expected: "0, 19132 or 19133",
        });
    }

    Ok(LevelDat {
        version: raw.version,
        version_history: raw.version_history,
        level_name: raw.level_name,
        time: raw.time,
        last_played: raw.last_played,
        game_type: raw.game_type,
        spawn,
        difficulty_settings: raw.difficulty_settings.unwrap_or_default(),
        allow_commands: raw.allow_commands.unwrap_or(raw.game_type == 1),
        initialized: raw.initialized,
        data_packs: raw.data_packs.unwrap_or_default(),
        enabled_features: raw.enabled_features.unwrap_or_else(default_features),
        was_modded: raw.was_modded,
        server_brands: raw.server_brands,
        removed_features: raw.removed_features,
        singleplayer_uuid,
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

fn parse_world_gen_settings(
    bytes: &[u8],
    path: &Path,
    registries: &Registries,
) -> Result<WorldGenSettings, SaveError> {
    let file: SavedDataFile<WorldGenSettings<Named>> = decode(bytes, path)?;
    check_data_version(file.data_version, path)?;
    WorldGenSettings::resolve(file.data, registries).map_err(|source| SaveError::Resolve {
        path: path.to_path_buf(),
        source,
    })
}

fn parse_weather(bytes: &[u8], path: &Path) -> Result<WeatherData, SaveError> {
    parse_saved_data(bytes, path)
}

fn parse_game_rules(bytes: &[u8], path: &Path) -> Result<GameRules, SaveError> {
    parse_saved_data(bytes, path)
}

fn parse_saved_data<T: DeserializeOwned>(bytes: &[u8], path: &Path) -> Result<T, SaveError> {
    let file: SavedDataFile<T> = decode(bytes, path)?;
    check_data_version(file.data_version, path)?;
    Ok(file.data)
}

fn uuid_from_int_array(ints: &[i32], path: &Path) -> Result<Uuid, SaveError> {
    let parts: [i32; 4] = fixed(ints, "singleplayer_uuid", path)?;
    let value = parts
        .into_iter()
        .fold(0u128, |acc, part| (acc << 32) | u128::from(part as u32));
    Ok(Uuid::from_u128(value))
}

#[cfg(test)]
#[path = "level_tests.rs"]
mod tests;
