use crate::json_list::{read_json_list, text_or_none, write_atomic};
use bevy_app::{App, Plugin};
use bevy_ecs::prelude::{Commands, Event, On, ResMut, Resource};
use mcrs_minecraft_protocol::uuid::Uuid;
use rustc_hash::FxHashMap;
use serde::{Deserialize, Serialize};
use std::path::Path;
use std::sync::Arc;
use time::format_description::BorrowedFormatItem;
use time::macros::format_description;
use time::{OffsetDateTime, UtcOffset};

const TIME_FORMAT: &[BorrowedFormatItem<'_>] = format_description!(
    "[year]-[month]-[day] [hour]:[minute]:[second] [offset_hour sign:mandatory][offset_minute]"
);
const EXPIRATION_FORMAT: &[BorrowedFormatItem<'_>] = format_description!(
    "[year]-[month]-[day] at [hour]:[minute]:[second] GMT[offset_hour sign:mandatory]:[offset_minute]"
);
pub const BANLIST_FILE: &str = "banned-players.json";

/// The host side of the ban list: it applies the edits that the dimensions send.
pub struct BanListHostPlugin;

impl Plugin for BanListHostPlugin {
    fn build(&self, app: &mut App) {
        app.init_resource::<BanList>();
        app.add_observer(apply_ban_edit);
    }
}

/// A ban was added, so a player that is online can be banned now.
#[derive(Event, Clone, Copy, Debug)]
pub struct EnforceBans;

fn apply_ban_edit(edit: On<EditBans>, mut bans: ResMut<BanList>, mut commands: Commands) {
    if bans.apply(edit.event()) {
        bans.write_file();
        if matches!(*edit, EditBans::Add(_)) {
            commands.trigger(EnforceBans);
        }
    }
}

/// A change to the ban list. The host owns the list, applies the change, writes the file and
/// tells every dimension.
#[derive(Event, Clone, Debug)]
pub enum EditBans {
    Add(BanEntry),
    Pardon(Uuid),
}

#[derive(Clone, Debug)]
pub struct BanEntry {
    pub uuid: Uuid,
    pub name: String,
    pub created: OffsetDateTime,
    pub reason: Option<String>,
    pub source: String,
    pub expiry: BanExpiry,
}

impl BanEntry {
    pub fn new(uuid: Uuid, name: String, source: String, reason: Option<String>) -> Self {
        Self {
            uuid,
            name,
            created: OffsetDateTime::now_utc(),
            reason,
            source,
            expiry: BanExpiry::Forever,
        }
    }

    pub fn expiration_text(&self) -> Option<String> {
        let BanExpiry::At(at) = self.expiry else {
            return None;
        };
        let offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
        at.to_offset(offset).format(EXPIRATION_FORMAT).ok()
    }

    fn to_file(&self, offset: UtcOffset) -> BanFileEntry {
        let format = |at: OffsetDateTime| at.to_offset(offset).format(TIME_FORMAT).ok();
        BanFileEntry {
            uuid: Some(self.uuid.to_string()),
            name: Some(self.name.clone()),
            created: format(self.created),
            source: Some(self.source.clone()),
            expires: match self.expiry {
                BanExpiry::Forever => Some("forever".to_owned()),
                BanExpiry::At(at) => format(at),
            },
            reason: self.reason.clone(),
        }
    }
}

#[derive(Clone, Debug)]
pub enum BanExpiry {
    Forever,
    At(time::OffsetDateTime),
}

#[derive(Resource, Debug, Clone, Default)]
pub struct BanList {
    entries: Arc<FxHashMap<Uuid, BanEntry>>,
    path: Option<Arc<Path>>,
}

#[derive(Deserialize, Serialize)]
struct BanFileEntry {
    uuid: Option<String>,
    #[serde(default, deserialize_with = "text_or_none")]
    name: Option<String>,
    #[serde(default, deserialize_with = "text_or_none")]
    created: Option<String>,
    #[serde(default, deserialize_with = "text_or_none")]
    source: Option<String>,
    #[serde(default, deserialize_with = "text_or_none")]
    expires: Option<String>,
    #[serde(
        default,
        deserialize_with = "text_or_none",
        skip_serializing_if = "Option::is_none"
    )]
    reason: Option<String>,
}

fn parse_time(text: &str) -> Option<OffsetDateTime> {
    OffsetDateTime::parse(text, TIME_FORMAT).ok()
}

impl BanFileEntry {
    fn into_entry(self, now: OffsetDateTime) -> Option<BanEntry> {
        Some(BanEntry {
            uuid: self.uuid?.parse().ok()?,
            name: self.name.unwrap_or_default(),
            created: self.created.as_deref().and_then(parse_time).unwrap_or(now),
            source: self.source.unwrap_or_else(|| "(Unknown)".to_owned()),
            expiry: self
                .expires
                .as_deref()
                .and_then(parse_time)
                .map_or(BanExpiry::Forever, BanExpiry::At),
            reason: self.reason,
        })
    }
}

impl BanList {
    pub fn new(entries: impl IntoIterator<Item = BanEntry>) -> Self {
        Self {
            entries: Arc::new(
                entries
                    .into_iter()
                    .map(|entry| (entry.uuid, entry))
                    .collect(),
            ),
            path: None,
        }
    }
    pub fn read(path: &Path) -> Self {
        let (entries, may_write) = match Self::read_entries(path) {
            Ok(entries) => (entries, true),
            Err(err) => {
                tracing::error!(
                    "failed to read {}: {err}; no player is banned until it is fixed",
                    path.display()
                );
                (Vec::new(), Self::set_aside(path))
            }
        };
        // A file that could not be set aside keeps its bans for an administrator, so no write
        // may replace it.
        let list = Self {
            path: may_write.then(|| Arc::from(path)),
            ..Self::new(entries)
        };
        list.write_file();
        list
    }

    fn set_aside(path: &Path) -> bool {
        let mut aside = path.as_os_str().to_owned();
        aside.push(".broken");
        std::fs::rename(path, &aside)
            .inspect_err(|err| tracing::error!("failed to set aside {}: {err}", path.display()))
            .is_ok()
    }

    /// Returns whether the entries changed, so the file needs a write.
    pub fn apply(&mut self, edit: &EditBans) -> bool {
        match edit {
            EditBans::Add(entry) => self.add(entry.clone()),
            EditBans::Pardon(id) => self.pardon(*id).is_some(),
        }
    }

    pub fn get(&self, uuid: &Uuid) -> Option<&BanEntry> {
        self.entries.get(uuid)
    }

    pub fn active(&self, uuid: &Uuid) -> Option<&BanEntry> {
        self.get(uuid).filter(|entry| match entry.expiry {
            BanExpiry::Forever => true,
            BanExpiry::At(at) => at > OffsetDateTime::now_utc(),
        })
    }

    pub fn entries(&self) -> impl Iterator<Item = &BanEntry> {
        self.entries.values()
    }

    pub fn find_by_name(&self, name: &str) -> Option<&BanEntry> {
        self.entries
            .values()
            .find(|entry| entry.name == name)
            .or_else(|| {
                self.entries
                    .values()
                    .find(|entry| entry.name.eq_ignore_ascii_case(name))
            })
    }

    pub fn pardon(&mut self, id: Uuid) -> Option<BanEntry> {
        if self.entries.contains_key(&id) {
            Arc::make_mut(&mut self.entries).remove(&id)
        } else {
            None
        }
    }

    pub fn add(&mut self, entry: BanEntry) -> bool {
        if self.active(&entry.uuid).is_some() {
            return false;
        }
        Arc::make_mut(&mut self.entries).insert(entry.uuid, entry);
        true
    }

    pub fn read_entries(path: &Path) -> std::io::Result<Vec<BanEntry>> {
        let raw: Vec<BanFileEntry> = read_json_list(path)?;
        let now = OffsetDateTime::now_utc();

        Ok(raw
            .into_iter()
            .filter_map(|entry| entry.into_entry(now))
            .collect())
    }

    pub fn write_file(&self) {
        let Some(path) = &self.path else {
            return;
        };
        let offset = UtcOffset::current_local_offset().unwrap_or(UtcOffset::UTC);
        let entries: Vec<_> = self.entries.values().map(|e| e.to_file(offset)).collect();
        let written = serde_json::to_string_pretty(&entries)
            .map_err(std::io::Error::from)
            .and_then(|json| write_atomic(path, &json));
        if let Err(err) = written {
            tracing::warn!("failed to write {}: {err}", path.display());
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    const STEVE: Uuid = Uuid::from_u128(0x5627dd98_e6be_3c21_b8a8_e92344183641);
    const GUEST: Uuid = Uuid::from_u128(0x404198aa_cc64_3021_9fdf_8766c0d0096c);

    fn temp_file(test: &str) -> PathBuf {
        let dir = std::env::temp_dir().join(format!("mcrs-banlist-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(test);
        let _ = std::fs::remove_file(&path);
        path
    }

    fn file_with(test: &str, contents: &str) -> PathBuf {
        let path = temp_file(test);
        std::fs::write(&path, contents).unwrap();
        path
    }

    fn on_disk(path: &Path) -> Vec<BanFileEntry> {
        serde_json::from_slice(&std::fs::read(path).unwrap()).unwrap()
    }

    fn ban(uuid: Uuid, name: &str) -> BanEntry {
        BanEntry::new(uuid, name.to_owned(), "Server".to_owned(), None)
    }

    #[test]
    fn a_missing_file_is_created_and_gives_an_empty_list() {
        let path = temp_file("missing");

        let list = BanList::read(&path);

        assert_eq!(list.entries().count(), 0);
        assert_eq!(on_disk(&path).len(), 0);
    }

    #[test]
    fn ban_file_001_reads_every_field_of_a_ban() {
        let path = file_with(
            "fields",
            &format!(
                r#"[{{"uuid": "{STEVE}", "name": "Steve", "created": "2026-11-02 02:03:43 +0500",
                     "source": "Server", "expires": "2099-01-01 05:00:00 +0500", "reason": "griefing"}}]"#
            ),
        );

        let list = BanList::read(&path);

        let entry = list.get(&STEVE).unwrap();
        assert_eq!(entry.name, "Steve");
        assert_eq!(entry.source, "Server");
        assert_eq!(entry.reason.as_deref(), Some("griefing"));
        assert_eq!(
            entry.created,
            parse_time("2026-11-02 02:03:43 +0500").unwrap()
        );
        assert!(matches!(
            entry.expiry,
            BanExpiry::At(at) if at == parse_time("2099-01-01 05:00:00 +0500").unwrap()
        ));
    }

    #[test]
    fn ban_file_002_a_ban_without_a_reason_writes_no_reason() {
        let path = temp_file("no_reason");
        let mut list = BanList::read(&path);
        list.add(ban(STEVE, "Steve"));

        list.write_file();

        let written = std::fs::read_to_string(&path).unwrap();
        assert!(written.contains("Steve"), "{written}");
        assert!(!written.contains("reason"), "{written}");
    }

    #[test]
    fn ban_file_004_the_defaults_apply_and_reach_the_file() {
        let path = file_with(
            "defaults",
            &format!(
                r#"[{{"uuid": "{STEVE}", "name": "Steve", "created": "yesterday", "expires": "soon"}}]"#
            ),
        );
        let before = OffsetDateTime::now_utc();

        let list = BanList::read(&path);

        let entry = list.get(&STEVE).unwrap();
        assert_eq!(entry.source, "(Unknown)");
        assert!(matches!(entry.expiry, BanExpiry::Forever));
        assert_eq!(entry.reason, None);
        assert!(entry.created >= before.replace_nanosecond(0).unwrap());

        let written = on_disk(&path);
        assert_eq!(written[0].source.as_deref(), Some("(Unknown)"));
        assert_eq!(written[0].expires.as_deref(), Some("forever"));
        assert!(parse_time(written[0].created.as_deref().unwrap()).is_some());
    }

    #[test]
    fn ban_file_005_a_ban_without_a_uuid_is_dropped_from_the_list_and_the_file() {
        let path = file_with(
            "no_uuid",
            &format!(r#"[{{"name": "Nobody"}}, {{"uuid": "{STEVE}", "name": "Steve"}}]"#),
        );

        let list = BanList::read(&path);

        assert_eq!(list.entries().count(), 1);
        assert!(list.get(&STEVE).is_some());
        assert_eq!(on_disk(&path).len(), 1);
    }

    #[test]
    fn a_field_that_is_not_text_takes_its_default_and_keeps_the_ban() {
        let path = file_with(
            "not_text",
            &format!(
                r#"[{{"uuid": "{STEVE}", "name": 1, "created": 5, "source": [], "expires": {{}}, "reason": 2}}]"#
            ),
        );

        let list = BanList::read(&path);

        let entry = list.get(&STEVE).unwrap();
        assert_eq!(entry.name, "");
        assert_eq!(entry.source, "(Unknown)");
        assert!(matches!(entry.expiry, BanExpiry::Forever));
        assert_eq!(entry.reason, None);
    }

    #[test]
    fn a_uuid_that_is_not_text_drops_the_ban() {
        let path = file_with("uuid_not_text", r#"[{"uuid": 5, "name": "Steve"}]"#);

        assert_eq!(BanList::read(&path).entries().count(), 0);
    }

    #[test]
    fn a_ban_with_a_uuid_that_is_not_valid_is_dropped() {
        let path = file_with("bad_uuid", r#"[{"uuid": "not a uuid", "name": "Steve"}]"#);

        assert_eq!(BanList::read(&path).entries().count(), 0);
    }

    #[test]
    fn an_entry_that_is_not_an_object_does_not_drop_the_other_bans() {
        let path = file_with("not_object", &format!(r#"[5, {{"uuid": "{STEVE}"}}]"#));

        let list = BanList::read(&path);

        assert_eq!(list.entries().count(), 1);
        assert!(list.get(&STEVE).is_some());
    }

    #[test]
    fn ban_file_006_a_time_keeps_its_moment_through_a_write_and_a_read() {
        let path = temp_file("round_trip");
        let at = parse_time("2099-01-01 05:00:00 +0500").unwrap();
        let mut list = BanList::read(&path);
        let mut entry = ban(STEVE, "Steve");
        entry.expiry = BanExpiry::At(at);
        list.add(entry);

        list.write_file();

        let read = BanList::read(&path);
        let entry = read.get(&STEVE).unwrap();
        assert!(matches!(entry.expiry, BanExpiry::At(found) if found == at));
    }

    #[test]
    fn ban_file_007_a_file_that_is_not_json_gives_an_empty_list_and_is_kept_aside() {
        let path = file_with("not_json", "not json");

        let list = BanList::read(&path);

        assert_eq!(list.entries().count(), 0);
        assert_eq!(on_disk(&path).len(), 0);
        let aside = path.with_file_name("not_json.broken");
        assert_eq!(std::fs::read_to_string(aside).unwrap(), "not json");
    }

    #[test]
    fn a_damaged_file_that_cannot_be_set_aside_stays_as_it_is_after_a_ban() {
        let path = file_with("not_movable", "not json");
        std::fs::create_dir_all(path.with_file_name("not_movable.broken")).unwrap();

        let mut list = BanList::read(&path);
        assert_eq!(list.entries().count(), 0);

        assert!(list.add(ban(STEVE, "Steve")));
        list.write_file();

        assert_eq!(std::fs::read_to_string(&path).unwrap(), "not json");
    }

    #[test]
    fn a_write_that_cannot_finish_leaves_the_file_as_it_was() {
        let path = temp_file("unfinished");
        let mut list = BanList::read(&path);
        std::fs::create_dir_all(path.with_file_name("unfinished.tmp")).unwrap();
        list.add(ban(STEVE, "Steve"));

        list.write_file();

        assert_eq!(on_disk(&path).len(), 0);
    }

    #[test]
    fn add_takes_a_new_ban_only() {
        let mut list = BanList::default();

        assert!(list.add(ban(STEVE, "Steve")));
        assert!(!list.add(ban(STEVE, "Other")));

        assert_eq!(list.get(&STEVE).unwrap().name, "Steve");
    }

    #[test]
    fn ban_exp_003_add_replaces_an_expired_ban() {
        let expired = BanEntry {
            expiry: BanExpiry::At(OffsetDateTime::UNIX_EPOCH),
            ..ban(STEVE, "Old")
        };
        let mut list = BanList::new([expired]);

        assert!(list.add(ban(STEVE, "Steve")));

        assert_eq!(list.get(&STEVE).unwrap().name, "Steve");
        assert!(list.active(&STEVE).is_some());
    }

    #[test]
    fn find_by_name_takes_the_exact_case_first() {
        let list = BanList::new([ban(STEVE, "Steve"), ban(GUEST, "steve")]);

        assert_eq!(list.find_by_name("Steve").unwrap().uuid, STEVE);
        assert_eq!(list.find_by_name("steve").unwrap().uuid, GUEST);
    }

    #[test]
    fn pardon_removes_a_ban_and_finds_nothing_after_it() {
        let mut list = BanList::new([ban(STEVE, "Steve"), ban(GUEST, "Guest")]);

        assert_eq!(list.pardon(STEVE).unwrap().name, "Steve");
        assert!(list.pardon(STEVE).is_none());

        assert!(list.get(&STEVE).is_none());
        assert!(list.get(&GUEST).is_some());
    }

    #[test]
    fn a_change_does_not_reach_a_copy_of_the_list() {
        let mut list = BanList::new([ban(STEVE, "Steve")]);
        let copy = list.clone();

        list.pardon(STEVE);

        assert!(copy.get(&STEVE).is_some());
    }

    #[test]
    fn find_by_name_ignores_the_case() {
        let list = BanList::new([ban(STEVE, "Steve")]);

        assert_eq!(list.find_by_name("sTEVE").unwrap().uuid, STEVE);
        assert!(list.find_by_name("Guest").is_none());
    }

    #[test]
    fn ban_exp_001_active_holds_a_forever_ban_and_a_future_ban_and_not_an_expired_ban() {
        let mut future = ban(STEVE, "Steve");
        future.expiry = BanExpiry::At(parse_time("2099-01-01 00:00:00 +0000").unwrap());
        let mut past = ban(GUEST, "Guest");
        past.expiry = BanExpiry::At(parse_time("2021-01-01 00:00:00 +0000").unwrap());
        let forever = ban(Uuid::from_u128(3), "Forever");
        let forever_id = forever.uuid;
        let list = BanList::new([future, past, forever]);

        assert!(list.active(&STEVE).is_some());
        assert!(list.active(&GUEST).is_none());
        assert!(list.active(&forever_id).is_some());
        assert!(list.get(&GUEST).is_some());
    }

    #[test]
    fn ban_join_003_the_expiration_text_appears_only_with_an_end() {
        let mut entry = ban(STEVE, "Steve");
        assert_eq!(entry.expiration_text(), None);

        entry.expiry = BanExpiry::At(parse_time("2099-06-01 12:00:00 +0000").unwrap());

        let text = entry.expiration_text().unwrap();
        assert!(text.starts_with("2099-06-01 at "), "{text}");
        assert!(text.contains(" GMT"), "{text}");
    }
}
