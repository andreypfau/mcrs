use std::collections::BTreeMap;
use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::item::{ItemStackValue, ItemStackWithSlot};
use serde::de::{Error as _, MapAccess, Visitor};
use serde::ser::SerializeMap;
use serde::{Deserialize, Deserializer, Serialize, Serializer};
use uuid::Uuid;

use super::{SaveError, WORLD_VERSION, check_data_version, fixed};

/// The keys mcrs models, typed; every other root key rides along in `rest`
/// so a vanilla file survives a round trip through a server that does not
/// understand it.
#[derive(Debug, Clone, PartialEq)]
pub struct PlayerDat {
    pub data_version: i32,
    pub pos: [f64; 3],
    pub rotation: [f32; 2],
    pub dimension: String,
    pub inventory: Vec<ItemStackWithSlot>,
    pub selected_item_slot: i32,
    pub equipment: BTreeMap<String, ItemStackValue>,
    pub rest: NbtCompound,
}

impl Default for PlayerDat {
    fn default() -> Self {
        Self {
            data_version: WORLD_VERSION,
            pos: [0.0; 3],
            rotation: [0.0; 2],
            dimension: "minecraft:overworld".to_owned(),
            inventory: Vec::new(),
            selected_item_slot: 0,
            equipment: BTreeMap::new(),
            rest: NbtCompound::new(),
        }
    }
}

const DATA_VERSION: &str = "DataVersion";
const POS: &str = "Pos";
const ROTATION: &str = "Rotation";
const DIMENSION: &str = "Dimension";
const INVENTORY: &str = "Inventory";
const SELECTED_ITEM_SLOT: &str = "SelectedItemSlot";
const EQUIPMENT: &str = "equipment";

impl Serialize for PlayerDat {
    fn serialize<S: Serializer>(&self, s: S) -> Result<S::Ok, S::Error> {
        let mut map = s.serialize_map(None)?;
        map.serialize_entry(DATA_VERSION, &self.data_version)?;
        map.serialize_entry(POS, &self.pos[..])?;
        map.serialize_entry(ROTATION, &self.rotation[..])?;
        map.serialize_entry(DIMENSION, &self.dimension)?;
        map.serialize_entry(INVENTORY, &self.inventory)?;
        map.serialize_entry(SELECTED_ITEM_SLOT, &self.selected_item_slot)?;
        map.serialize_entry(EQUIPMENT, &self.equipment)?;
        for (key, tag) in self.rest.child_tags.iter() {
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
                let mut dat = PlayerDat::default();
                let mut data_version = None;
                let mut pos: Option<Vec<f64>> = None;
                let mut rotation: Option<Vec<f32>> = None;
                let mut dimension = None;
                while let Some(key) = map.next_key::<String>()? {
                    match key.as_str() {
                        DATA_VERSION => data_version = Some(map.next_value()?),
                        POS => pos = Some(map.next_value()?),
                        ROTATION => rotation = Some(map.next_value()?),
                        DIMENSION => dimension = Some(map.next_value()?),
                        INVENTORY => dat.inventory = map.next_value()?,
                        SELECTED_ITEM_SLOT => dat.selected_item_slot = map.next_value()?,
                        EQUIPMENT => dat.equipment = map.next_value()?,
                        _ => {
                            let tag: NbtTag = map.next_value()?;
                            dat.rest.put(&key, tag);
                        }
                    }
                }
                dat.data_version = data_version.ok_or_else(|| A::Error::missing_field(DATA_VERSION))?;
                let pos = pos.ok_or_else(|| A::Error::missing_field(POS))?;
                dat.pos = <[f64; 3]>::try_from(pos.as_slice())
                    .map_err(|_| A::Error::invalid_length(pos.len(), &"3 coordinates"))?;
                let rotation = rotation.ok_or_else(|| A::Error::missing_field(ROTATION))?;
                dat.rotation = <[f32; 2]>::try_from(rotation.as_slice())
                    .map_err(|_| A::Error::invalid_length(rotation.len(), &"yaw and pitch"))?;
                dat.dimension = dimension.ok_or_else(|| A::Error::missing_field(DIMENSION))?;
                Ok(dat)
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
    let dat: PlayerDat = super::decode(bytes, path)?;
    check_data_version(dat.data_version, path)?;
    for (index, value) in dat.rotation.iter().enumerate() {
        if !value.is_finite() {
            return Err(SaveError::OutOfRange {
                path: path.to_path_buf(),
                field: ROTATION,
                value: format!("{value} at {index}"),
                expected: "finite",
            });
        }
    }
    let _: [f64; 3] = fixed(&dat.pos, POS, path)?;
    Ok(dat)
}

/// Written next to the target and renamed over it, so a crash mid-write
/// leaves the previous file whole.
pub fn write_player_dat(world: &Path, player: Uuid, dat: &PlayerDat) -> Result<(), SaveError> {
    let path = player_dat_path(world, player);
    let io = |source| SaveError::Io {
        path: path.clone(),
        source,
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(io)?;
    }
    let bytes = mcrs_minecraft_nbt::nbt_compress::to_gzip_bytes_vec(dat).map_err(|source| {
        SaveError::Nbt {
            path: path.clone(),
            source,
        }
    })?;
    let tmp = path.with_extension("dat.tmp");
    fs::write(&tmp, bytes).map_err(io)?;
    fs::rename(&tmp, &path).map_err(io)
}
