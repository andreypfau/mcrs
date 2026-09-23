use crate::world::entity::player::ability::PlayerOpLevel;
use bevy_ecs::prelude::Resource;
use mcrs_minecraft_protocol::uuid::Uuid;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Deserializer, Serialize};
use std::io::ErrorKind;
use std::path::{Path, PathBuf};
use std::sync::Arc;

pub const OPS_FILE: &str = "ops.json";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct OpListEntry {
    pub uuid: Uuid,
    pub name: String,
    #[serde(default, deserialize_with = "clamped_level")]
    pub level: u8,
    #[serde(default)]
    pub bypasses_player_limit: bool,
}

fn clamped_level<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u8, D::Error> {
    let level = i32::deserialize(deserializer)?;
    Ok(level.clamp(0, PlayerOpLevel::MAX as i32) as u8)
}

#[derive(Resource, Clone, Debug, Default)]
pub struct OpList(Arc<FxHashMap<Uuid, OpListEntry>>);

#[derive(Resource, Clone, Copy, Debug, Default)]
pub struct DefaultOpLevel(pub PlayerOpLevel);

#[derive(Debug, thiserror::Error)]
pub enum OpListError {
    #[error("failed to read {path}: {source}")]
    Io {
        path: PathBuf,
        source: std::io::Error,
    },
    #[error("malformed {path}: {source}")]
    Malformed {
        path: PathBuf,
        source: serde_json::Error,
    },
}

impl OpList {
    pub fn new(entries: impl IntoIterator<Item = OpListEntry>) -> Self {
        Self(Arc::new(
            entries
                .into_iter()
                .map(|entry| (entry.uuid, entry))
                .collect(),
        ))
    }

    pub fn read(path: &Path) -> Result<Self, OpListError> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Self::default()),
            Err(source) => {
                return Err(OpListError::Io {
                    path: path.to_owned(),
                    source,
                });
            }
        };
        let entries: Vec<OpListEntry> =
            serde_json::from_slice(&bytes).map_err(|source| OpListError::Malformed {
                path: path.to_owned(),
                source,
            })?;
        Ok(Self::new(entries))
    }

    pub fn get(&self, uuid: &Uuid) -> Option<&OpListEntry> {
        self.0.get(uuid)
    }

    pub fn level_of(&self, uuid: &Uuid, default: DefaultOpLevel) -> PlayerOpLevel {
        self.get(uuid)
            .map_or(default.0, |entry| PlayerOpLevel(entry.level))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UUID: &str = "069a79f4-44e9-4726-a5be-fca90e38aaf5";

    #[test]
    fn reads_vanilla_layout() {
        let entries: Vec<OpListEntry> = serde_json::from_str(&format!(
            r#"[{{"uuid":"{UUID}","name":"Notch","level":4,"bypassesPlayerLimit":false}}]"#
        ))
        .unwrap();
        let ops = OpList::new(entries);
        let uuid = Uuid::parse_str(UUID).unwrap();
        assert_eq!(ops.level_of(&uuid, DefaultOpLevel::default()).0, 4);
        assert_eq!(
            ops.level_of(&Uuid::nil(), DefaultOpLevel(PlayerOpLevel(1)))
                .0,
            1
        );
    }

    #[test]
    fn missing_level_is_zero_and_out_of_range_clamps() {
        let entries: Vec<OpListEntry> = serde_json::from_str(&format!(
            r#"[{{"uuid":"{UUID}","name":"a"}},{{"uuid":"{}","name":"b","level":9}}]"#,
            Uuid::nil()
        ))
        .unwrap();
        assert_eq!(entries[0].level, 0);
        assert!(!entries[0].bypasses_player_limit);
        assert_eq!(entries[1].level, 4);
    }

    #[test]
    fn round_trips() {
        let entry = OpListEntry {
            uuid: Uuid::parse_str(UUID).unwrap(),
            name: "Notch".into(),
            level: 3,
            bypasses_player_limit: true,
        };
        let json = serde_json::to_string(&entry).unwrap();
        assert_eq!(
            json,
            format!(r#"{{"uuid":"{UUID}","name":"Notch","level":3,"bypassesPlayerLimit":true}}"#)
        );
        assert_eq!(serde_json::from_str::<OpListEntry>(&json).unwrap(), entry);
    }

    #[test]
    fn missing_file_is_empty_and_malformed_file_fails() {
        let dir = std::env::temp_dir().join(format!("mcrs-ops-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(OPS_FILE);
        let _ = std::fs::remove_file(&path);
        assert!(OpList::read(&path).unwrap().get(&Uuid::nil()).is_none());

        std::fs::write(&path, r#"[{"uuid":"not-a-uuid","name":"x"}]"#).unwrap();
        assert!(matches!(
            OpList::read(&path),
            Err(OpListError::Malformed { .. })
        ));
        std::fs::remove_dir_all(&dir).unwrap();
    }
}
