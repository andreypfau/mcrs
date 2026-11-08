use crate::json_list::{parse_json_list, read_json_list, write_atomic};
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::{Commands, Event, On, ResMut, Resource};
use mcrs_minecraft_network::client::offline_player_uuid;
use mcrs_minecraft_protocol::uuid::Uuid;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::io::ErrorKind;
use std::path::Path;
use std::sync::Arc;

pub const WHITELIST_FILE: &str = "whitelist.json";

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq, Eq)]
pub struct WhitelistEntry {
    pub uuid: Uuid,
    pub name: String,
}

impl WhitelistEntry {
    pub fn offline(name: &str) -> Self {
        Self {
            uuid: offline_player_uuid(name),
            name: name.to_owned(),
        }
    }
}

#[derive(Resource, Clone, Debug, Default)]
pub struct Whitelist {
    enabled: bool,
    enforced: bool,
    entries: Arc<FxHashMap<Uuid, WhitelistEntry>>,
    path: Option<Arc<Path>>,
}

/// The host side of the whitelist: it applies the edits that the dimensions send.
pub struct WhitelistHostPlugin;

impl Plugin for WhitelistHostPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<Whitelist>();
        app.add_observer(apply_whitelist_edit);
    }
}

/// The whitelist changed in a way that can take an online player off it, with enforcement on.
#[derive(Event, Clone, Copy, Debug)]
pub struct EnforceWhitelist;

fn apply_whitelist_edit(
    edit: On<EditWhitelist>,
    mut whitelist: ResMut<Whitelist>,
    mut commands: Commands,
) {
    if whitelist.apply(edit.event()) {
        whitelist.write_file();
    }
    // An add cannot take a player off the whitelist, and enforcement must keep an operator that
    // is not listed.
    if !matches!(*edit, EditWhitelist::Add(_)) && whitelist.enforces() {
        commands.trigger(EnforceWhitelist);
    }
}

/// A change to the whitelist. The host owns the whitelist, applies the change, writes the file
/// and tells every dimension.
#[derive(Event, Clone, Debug, PartialEq, Eq)]
pub enum EditWhitelist {
    Add(WhitelistEntry),
    Remove(Uuid),
    SetEnabled(bool),
    Reload,
}

impl Whitelist {
    pub fn new(entries: impl IntoIterator<Item = WhitelistEntry>) -> Self {
        Self {
            enabled: false,
            enforced: false,
            entries: Arc::new(
                entries
                    .into_iter()
                    .map(|entry| (entry.uuid, entry))
                    .collect(),
            ),
            path: None,
        }
    }

    /// Returns whether the entries changed, so the file needs a write.
    pub fn apply(&mut self, edit: &EditWhitelist) -> bool {
        match edit {
            EditWhitelist::Add(entry) => self.add(entry.clone()),
            EditWhitelist::Remove(id) => self.remove(id).is_some(),
            EditWhitelist::SetEnabled(enabled) => {
                self.set_enabled(*enabled);
                false
            }
            EditWhitelist::Reload => {
                self.reload();
                false
            }
        }
    }

    pub fn names(&self) -> Vec<&str> {
        self.entries
            .values()
            .map(|entry| entry.name.as_str())
            .collect()
    }
    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        let entries: Vec<_> = self.entries.values().collect();
        write_atomic(path, &serde_json::to_string_pretty(&entries)?)
    }

    pub fn remove(&mut self, id: &Uuid) -> Option<WhitelistEntry> {
        if self.entries.contains_key(id) {
            return Arc::make_mut(&mut self.entries).remove(id);
        }
        None
    }

    pub fn add(&mut self, entry: WhitelistEntry) -> bool {
        if self.entries.contains_key(&entry.uuid) {
            return false;
        }
        Arc::make_mut(&mut self.entries).insert(entry.uuid, entry);
        true
    }

    pub fn write_file(&self) {
        let Some(path) = &self.path else {
            return;
        };
        if let Err(err) = self.save(path) {
            tracing::warn!("failed to write {}: {err}", path.display());
        }
    }

    pub fn read(path: &Path) -> Self {
        Self {
            path: Some(Arc::from(path)),
            ..Self::new(Self::read_entries(path))
        }
    }

    pub fn contains(&self, id: &Uuid) -> bool {
        self.entries.contains_key(id)
    }

    pub fn enabled(mut self, enabled: bool) -> Self {
        self.enabled = enabled;
        self
    }

    pub fn enforced(mut self, enforced: bool) -> Self {
        self.enforced = enforced;
        self
    }

    pub fn enforces(&self) -> bool {
        self.enabled && self.enforced
    }

    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    pub fn set_enabled(&mut self, enabled: bool) {
        self.enabled = enabled;
    }

    pub fn reload(&mut self) {
        let Some(path) = self.path.clone() else {
            return;
        };
        match std::fs::read(&path) {
            Ok(bytes) => {
                let entries = parse_json_list(&path, &bytes).unwrap_or_else(|err| {
                    tracing::warn!("malformed {}: {err}", path.display());
                    vec![]
                });
                self.entries = Self::new(entries).entries;
            }
            Err(err) if err.kind() == ErrorKind::NotFound => {}
            Err(err) => tracing::warn!("failed to read {}: {err}", path.display()),
        }
    }

    pub fn allows(&self, id: &Uuid) -> bool {
        !self.enabled || self.entries.contains_key(id)
    }

    pub fn read_entries(path: &Path) -> Vec<WhitelistEntry> {
        read_json_list(path).unwrap_or_else(|err| {
            tracing::warn!("failed to read {}: {err}", path.display());
            vec![]
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const WA: Uuid = Uuid::from_u128(0x2ab59332_79f8_3b40_9c54_ea9d7be549c8);
    const WJ: Uuid = Uuid::from_u128(0x65337305_98aa_3838_9ef5_1133463431c5);

    fn temp_file(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mcrs-whitelist-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(test);
        let _ = std::fs::remove_file(&path);
        path
    }

    #[test]
    fn wl_file_001_an_entry_lists_its_profile_id() {
        let path = temp_file("wl_file_001");
        std::fs::write(
            &path,
            r#"[{"uuid": "2ab59332-79f8-3b40-9c54-ea9d7be549c8", "name": "wa"}]"#,
        )
        .unwrap();

        let whitelist = Whitelist::read(&path);

        assert!(whitelist.entries.contains_key(&WA));
        assert!(!whitelist.entries.contains_key(&Uuid::nil()));
    }

    #[test]
    fn wl_file_003_an_entry_without_both_fields_is_ignored_and_stays_in_the_file() {
        let path = temp_file("wl_file_003");
        let content = r#"[{"name": "Wh"},
            {"uuid": "65337305-98aa-3838-9ef5-1133463431c5"},
            {"uuid": "2ab59332-79f8-3b40-9c54-ea9d7be549c8", "name": "wa"}]"#;
        std::fs::write(&path, content).unwrap();

        let whitelist = Whitelist::read(&path);

        assert!(whitelist.entries.contains_key(&WA));
        assert!(!whitelist.entries.contains_key(&WJ));
        assert_eq!(whitelist.entries.len(), 1);
        assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
    }

    #[test]
    fn wl_file_004_a_file_without_a_json_array_is_an_empty_whitelist_and_stays() {
        for (test, content) in [
            ("wl_file_004_text", "not json"),
            ("wl_file_004_object", "{}"),
        ] {
            let path = temp_file(test);
            std::fs::write(&path, content).unwrap();

            assert!(Whitelist::read(&path).entries.is_empty(), "{content:?}");
            assert_eq!(std::fs::read_to_string(&path).unwrap(), content);
        }
    }

    #[test]
    fn wl_file_005_a_missing_file_is_created_with_an_empty_array() {
        let path = temp_file("wl_file_005");

        assert!(Whitelist::read(&path).entries.is_empty());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "[]");
    }

    #[test]
    fn wl_file_002_a_reload_reads_the_file_and_keeps_the_state_of_the_check() {
        let path = temp_file("wl_file_002");
        let mut whitelist = Whitelist::read(&path).enabled(true).enforced(true);
        std::fs::write(
            &path,
            r#"[{"uuid": "2ab59332-79f8-3b40-9c54-ea9d7be549c8", "name": "wa"}]"#,
        )
        .unwrap();
        assert!(!whitelist.contains(&WA));

        whitelist.reload();

        assert!(whitelist.contains(&WA));
        assert!(whitelist.is_enabled());
        assert!(whitelist.enforces());
    }

    #[test]
    fn wl_file_004_a_reload_of_a_file_without_a_json_array_empties_the_whitelist_and_keeps_the_file()
     {
        let path = temp_file("wl_file_004_reload");
        std::fs::write(
            &path,
            r#"[{"uuid": "2ab59332-79f8-3b40-9c54-ea9d7be549c8", "name": "wa"}]"#,
        )
        .unwrap();
        let mut whitelist = Whitelist::read(&path);
        std::fs::write(&path, "not json").unwrap();

        whitelist.reload();

        assert!(whitelist.entries.is_empty());
        assert_eq!(std::fs::read_to_string(&path).unwrap(), "not json");
    }

    fn wa() -> WhitelistEntry {
        WhitelistEntry {
            uuid: WA,
            name: "wa".to_owned(),
        }
    }

    #[test]
    fn wl_cmd_002_a_write_holds_the_entries_with_an_indent_of_two_spaces() {
        let path = temp_file("wl_cmd_002_format");
        let mut whitelist = Whitelist::default();
        whitelist.add(wa());

        whitelist.save(&path).unwrap();

        assert_eq!(
            std::fs::read_to_string(&path).unwrap(),
            r#"[
  {
    "uuid": "2ab59332-79f8-3b40-9c54-ea9d7be549c8",
    "name": "wa"
  }
]"#
        );
    }

    #[test]
    fn wl_cmd_002_a_write_replaces_ignored_entries_and_a_file_without_a_json_array() {
        for (test, content) in [
            ("wl_cmd_002_ignored", r#"[{"name": "Wh"}]"#),
            ("wl_cmd_002_text", "not json"),
        ] {
            let path = temp_file(test);
            std::fs::write(&path, content).unwrap();

            let mut whitelist = Whitelist::read(&path);
            whitelist.add(wa());
            whitelist.save(&path).unwrap();

            let written = std::fs::read_to_string(&path).unwrap();
            assert!(!written.contains("Wh"), "{written}");
            assert!(!written.contains("not json"), "{written}");
            assert!(Whitelist::read(&path).entries.contains_key(&WA));
        }
    }

    #[test]
    fn wl_cmd_002_a_remove_leaves_the_entry_out_of_the_next_write() {
        let path = temp_file("wl_cmd_002_remove");
        let mut whitelist = Whitelist::new([wa()]);

        whitelist.remove(&WA);
        whitelist.save(&path).unwrap();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "[]");
    }

    #[test]
    fn a_write_that_cannot_finish_leaves_the_file_as_it_was() {
        let path = temp_file("unfinished");
        std::fs::write(&path, "[]").unwrap();
        std::fs::create_dir_all(path.with_file_name("unfinished.tmp")).unwrap();

        assert!(Whitelist::new([wa()]).save(&path).is_err());

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "[]");
    }

    #[test]
    fn a_reload_that_cannot_read_the_file_keeps_the_whitelist() {
        let path = temp_file("unreadable");
        std::fs::write(
            &path,
            r#"[{"uuid": "2ab59332-79f8-3b40-9c54-ea9d7be549c8", "name": "wa"}]"#,
        )
        .unwrap();
        let mut whitelist = Whitelist::read(&path);
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();

        whitelist.reload();

        assert!(whitelist.contains(&WA));
        std::fs::remove_dir(&path).unwrap();
    }

    #[test]
    fn add_and_remove_report_whether_the_whitelist_changed() {
        let mut whitelist = Whitelist::default();

        assert!(whitelist.add(wa()));
        assert!(!whitelist.add(wa()));
        assert_eq!(whitelist.remove(&WA), Some(wa()));
        assert_eq!(whitelist.remove(&WA), None);
    }

    #[test]
    fn a_change_leaves_the_copy_that_a_clone_holds() {
        let mut whitelist = Whitelist::new([wa()]);
        let copy = whitelist.clone();

        whitelist.remove(&WA);

        assert!(copy.entries.contains_key(&WA));
    }

    #[test]
    fn wl_cmd_003_a_new_name_keeps_its_case_and_gets_the_id_of_that_name() {
        assert_eq!(WhitelistEntry::offline("wa"), wa());

        let upper = WhitelistEntry::offline("Wa");
        assert_eq!(upper.name, "Wa");
        assert_eq!(upper.uuid, offline_player_uuid("Wa"));
        assert_ne!(upper.uuid, WA);
    }
}
