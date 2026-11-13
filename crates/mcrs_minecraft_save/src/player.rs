use mcrs_minecraft_resource::{ResourceKeyBuf, ResourceLocationBuf};
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use serde::de::{Error as _, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

use crate::level::{SaveError, check_data_version};

/// The root compound of a player file besides `DataVersion`. The typed forms of its keys live with
/// the components that own them; the reader of the file fails on a key that no component claims.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerDat {
    pub data_version: i32,
    pub root: NbtCompound,
}

impl PlayerDat {
    pub fn new(data_version: i32, root: NbtCompound) -> Self {
        Self { data_version, root }
    }

    pub fn get(&self, key: &str) -> Option<&NbtTag> {
        self.root.get(key)
    }

    pub fn pos(&self) -> Option<[f64; 3]> {
        let list = self.get(POS)?.extract_list()?;
        let [x, y, z] = list else {
            return None;
        };
        Some([
            x.extract_double()?,
            y.extract_double()?,
            z.extract_double()?,
        ])
    }

    pub fn rotation(&self) -> Option<[f32; 2]> {
        let list = self.get(ROTATION)?.extract_list()?;
        let [yaw, pitch] = list else {
            return None;
        };
        Some([yaw.extract_float()?, pitch.extract_float()?])
    }

    pub fn dimension(&self) -> Option<ResourceKeyBuf<mcrs_minecraft_dimension::Dimension>> {
        let text = self.get(DIMENSION)?.extract_string()?;
        let location = text.parse::<ResourceLocationBuf>().ok()?;
        Some(ResourceKeyBuf::from_location(location))
    }
}

impl Default for PlayerDat {
    fn default() -> Self {
        let mut root = NbtCompound::new();
        root.put(
            POS,
            NbtTag::List(vec![
                NbtTag::Double(0.0),
                NbtTag::Double(0.0),
                NbtTag::Double(0.0),
            ]),
        );
        root.put(
            ROTATION,
            NbtTag::List(vec![NbtTag::Float(0.0), NbtTag::Float(0.0)]),
        );
        root.put(
            DIMENSION,
            NbtTag::String(
                mcrs_minecraft_dimension::dimension_keys::OVERWORLD
                    .to_buf()
                    .to_string(),
            ),
        );
        Self {
            data_version: VERSION.world_version,
            root,
        }
    }
}

const DATA_VERSION: &str = "DataVersion";
const POS: &str = "Pos";
const ROTATION: &str = "Rotation";
const DIMENSION: &str = "Dimension";

impl Serialize for PlayerDat {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(None)?;
        map.serialize_entry(DATA_VERSION, &self.data_version)?;
        for (key, tag) in &self.root.child_tags {
            map.serialize_entry(key, tag)?;
        }
        map.end()
    }
}

impl<'de> Deserialize<'de> for PlayerDat {
    fn deserialize<D: Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        struct DatVisitor;

        impl<'de> Visitor<'de> for DatVisitor {
            type Value = PlayerDat;

            fn expecting(&self, f: &mut fmt::Formatter) -> fmt::Result {
                f.write_str("a player data compound")
            }

            fn visit_map<A: MapAccess<'de>>(self, mut map: A) -> Result<PlayerDat, A::Error> {
                let mut data_version = None;
                let mut root = NbtCompound::new();
                while let Some(key) = map.next_key::<String>()? {
                    if key == DATA_VERSION {
                        data_version = Some(map.next_value()?);
                    } else {
                        let tag: NbtTag = map.next_value()?;
                        root.put(&key, tag);
                    }
                }
                let data_version =
                    data_version.ok_or_else(|| A::Error::missing_field(DATA_VERSION))?;
                Ok(PlayerDat { data_version, root })
            }
        }

        d.deserialize_map(DatVisitor)
    }
}

pub fn player_dat_path(world: &Path, player: Uuid) -> PathBuf {
    world
        .join("players/data")
        .join(format!("{}.dat", player.hyphenated()))
}

pub fn read_player_dat(world: &Path, player: Uuid) -> Result<Option<PlayerDat>, SaveError> {
    let path = player_dat_path(world, player);
    let bytes = match fs::read(&path) {
        Ok(bytes) => bytes,
        Err(source) if source.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(SaveError::Io { path, source }),
    };
    parse_player_dat(&bytes, &path).map(Some)
}

pub fn parse_player_dat(bytes: &[u8], path: &Path) -> Result<PlayerDat, SaveError> {
    let dat: PlayerDat = crate::level::decode(bytes, path)?;
    check_data_version(dat.data_version, path)?;
    if dat.get(DIMENSION).is_some() && dat.dimension().is_none() {
        return Err(SaveError::OutOfRange {
            path: path.to_path_buf(),
            field: DIMENSION,
            value: dat
                .get(DIMENSION)
                .and_then(NbtTag::extract_string)
                .unwrap_or_default()
                .to_owned(),
            expected: "a resource location",
        });
    }
    Ok(dat)
}

/// Written through [`crate::write_atomic`], so a crash mid-write leaves the previous file whole.
pub fn write_player_dat(world: &Path, player: Uuid, dat: &PlayerDat) -> Result<(), SaveError> {
    let path = player_dat_path(world, player);
    let io = |source| SaveError::Io {
        path: path.clone(),
        source,
    };
    let bytes = mcrs_minecraft_nbt::nbt_compress::to_gzip_bytes_vec(dat).map_err(|source| {
        SaveError::Nbt {
            path: path.clone(),
            source,
        }
    })?;
    crate::write_atomic(&path, &bytes).map_err(io)
}
