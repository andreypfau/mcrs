use serde::de::DeserializeOwned;
use serde::{Deserialize, Deserializer};
use std::fs::File;
use std::io::{ErrorKind, Write};
use std::path::Path;

#[derive(Deserialize)]
#[serde(untagged)]
pub(crate) enum FileEntry<T> {
    Entry(T),
    Ignored(serde::de::IgnoredAny),
}

pub(crate) fn text_or_none<'de, D: Deserializer<'de>>(de: D) -> Result<Option<String>, D::Error> {
    Ok(match FileEntry::<String>::deserialize(de)? {
        FileEntry::Entry(text) => Some(text),
        FileEntry::Ignored(_) => None,
    })
}

pub(crate) fn read_json_list<T: DeserializeOwned>(path: &Path) -> std::io::Result<Vec<T>> {
    let bytes = match std::fs::read(path) {
        Ok(bytes) => bytes,
        Err(err) if err.kind() == ErrorKind::NotFound => {
            std::fs::write(path, "[]")?;
            return Ok(vec![]);
        }
        Err(err) => return Err(err),
    };
    parse_json_list(path, &bytes)
}

pub(crate) fn parse_json_list<T: DeserializeOwned>(
    path: &Path,
    bytes: &[u8],
) -> std::io::Result<Vec<T>> {
    let entries: Vec<FileEntry<T>> = serde_json::from_slice(bytes)?;
    Ok(entries
        .into_iter()
        .filter_map(|entry| match entry {
            FileEntry::Entry(entry) => Some(entry),
            FileEntry::Ignored(_) => {
                tracing::warn!("skipped an entry of {}", path.display());
                None
            }
        })
        .collect())
}

pub(crate) fn write_atomic(path: &Path, contents: &str) -> std::io::Result<()> {
    let mut temp = path.as_os_str().to_owned();
    temp.push(".tmp");
    let mut file = File::create(&temp)?;
    file.write_all(contents.as_bytes())?;
    // Without the sync a power loss can keep the rename and lose the data.
    file.sync_all()?;
    drop(file);
    std::fs::rename(&temp, path)
}
