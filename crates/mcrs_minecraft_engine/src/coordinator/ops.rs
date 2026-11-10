use crate::ability::PlayerOpLevel;
use crate::json_list::write_atomic;
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::{Event, On, ResMut, Resource};
use mcrs_minecraft_protocol::uuid::Uuid;
use rustc_hash::{FxHashMap, FxHashSet};
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

impl OpListEntry {
    /// An operator at the top level, which the `op` command and a loopback join give.
    pub fn owner(uuid: Uuid, name: String) -> Self {
        Self {
            uuid,
            name,
            level: PlayerOpLevel::MAX,
            bypasses_player_limit: false,
        }
    }
}

fn clamped_level<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u8, D::Error> {
    let level = i32::deserialize(deserializer)?;
    Ok(level.clamp(0, PlayerOpLevel::MAX as i32) as u8)
}

/// The operators. The operators of a loopback join stay out of the file, so they end with the
/// process and a later join from another address does not find them.
#[derive(Resource, Clone, Debug, Default)]
pub struct OpList {
    entries: Arc<FxHashMap<Uuid, OpListEntry>>,
    loopback: FxHashSet<Uuid>,
    path: Option<Arc<Path>>,
}

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

/// A change to the operators. The host owns the list, applies the change, writes the file and
/// tells every dimension.
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub enum EditOps {
    Add(OpListEntry),
    Remove(Uuid),
}

/// The host side of the operators: it applies the edits that the dimensions send.
pub struct OpListHostPlugin;

impl Plugin for OpListHostPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<OpList>();
        app.add_observer(apply_ops_edit);
    }
}

fn apply_ops_edit(edit: On<EditOps>, mut ops: ResMut<OpList>) {
    if ops.apply(edit.event()) {
        ops.write_file();
    }
}

impl OpList {
    pub fn new(entries: impl IntoIterator<Item = OpListEntry>) -> Self {
        Self {
            entries: Arc::new(
                entries
                    .into_iter()
                    .map(|entry| (entry.uuid, entry))
                    .collect(),
            ),
            ..Self::default()
        }
    }

    /// The operators of the file at `path`, which each later edit writes back.
    pub fn read(path: &Path) -> Result<Self, OpListError> {
        Ok(Self {
            path: Some(Arc::from(path)),
            ..Self::new(Self::read_entries(path)?)
        })
    }

    /// The entries of the ops file at `path`; none when the file does not exist.
    pub fn read_entries(path: &Path) -> Result<Vec<OpListEntry>, OpListError> {
        let bytes = match std::fs::read(path) {
            Ok(bytes) => bytes,
            Err(err) if err.kind() == ErrorKind::NotFound => return Ok(Vec::new()),
            Err(source) => {
                return Err(OpListError::Io {
                    path: path.to_owned(),
                    source,
                });
            }
        };
        serde_json::from_slice(&bytes).map_err(|source| OpListError::Malformed {
            path: path.to_owned(),
            source,
        })
    }

    pub fn get(&self, uuid: &Uuid) -> Option<&OpListEntry> {
        self.entries.get(uuid)
    }

    /// The operator with `name`, in any case.
    pub fn find_by_name(&self, name: &str) -> Option<&OpListEntry> {
        self.entries
            .values()
            .find(|entry| entry.name.eq_ignore_ascii_case(name))
    }

    pub fn level_of(&self, uuid: &Uuid, default: DefaultOpLevel) -> PlayerOpLevel {
        self.get(uuid)
            .map_or(default.0, |entry| PlayerOpLevel(entry.level))
    }

    /// Returns whether the operators changed. An add of an operator that is listed changes
    /// nothing.
    pub fn apply(&mut self, edit: &EditOps) -> bool {
        match edit {
            EditOps::Add(entry) => self.add(entry.clone()),
            EditOps::Remove(id) => {
                self.loopback.remove(id);
                self.entries.contains_key(id)
                    && Arc::make_mut(&mut self.entries).remove(id).is_some()
            }
        }
    }

    /// Makes `entry` an operator for the life of the process, unless it is one. The file does
    /// not hold it.
    pub fn grant_loopback(&mut self, entry: OpListEntry) -> bool {
        let id = entry.uuid;
        if !self.add(entry) {
            return false;
        }
        self.loopback.insert(id);
        true
    }

    fn add(&mut self, entry: OpListEntry) -> bool {
        if self.entries.contains_key(&entry.uuid) {
            return false;
        }
        Arc::make_mut(&mut self.entries).insert(entry.uuid, entry);
        true
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let mut entries: Vec<&OpListEntry> = self
            .entries
            .values()
            .filter(|entry| !self.loopback.contains(&entry.uuid))
            .collect();
        entries.sort_by(|left, right| left.name.cmp(&right.name));
        write_atomic(path, &serde_json::to_string_pretty(&entries)?)
    }

    pub fn write_file(&self) {
        let Some(path) = &self.path else {
            return;
        };
        if let Err(err) = self.save(path) {
            tracing::warn!("failed to write {}: {err}", path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const UUID: &str = "069a79f4-44e9-4726-a5be-fca90e38aaf5";

    #[test]
    fn reads_vanilla_layout_with_a_missing_and_an_out_of_range_level() {
        let entries: Vec<OpListEntry> = serde_json::from_str(&format!(
            r#"[{{"uuid":"{UUID}","name":"Notch","level":4,"bypassesPlayerLimit":false}},
                {{"uuid":"{}","name":"a"}},
                {{"uuid":"{}","name":"b","level":9}}]"#,
            Uuid::nil(),
            Uuid::max()
        ))
        .unwrap();
        assert_eq!(entries[1].level, 0);
        assert!(!entries[1].bypasses_player_limit);
        assert_eq!(entries[2].level, 4);

        let ops = OpList::new(entries);
        let uuid = Uuid::parse_str(UUID).unwrap();
        assert_eq!(ops.level_of(&uuid, DefaultOpLevel::default()).0, 4);
        assert_eq!(
            ops.level_of(&Uuid::from_u128(7), DefaultOpLevel(PlayerOpLevel(1)))
                .0,
            1
        );
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
