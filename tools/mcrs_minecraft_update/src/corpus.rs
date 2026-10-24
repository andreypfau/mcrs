use std::fs;
use std::path::Path;

use mcrs_minecraft_client_jar::Directory;

const VERSION_FILE: &str = "version.json";

#[derive(Debug, Default, PartialEq, Eq)]
pub struct Report {
    pub written: Vec<String>,
    pub deleted: Vec<String>,
}

pub(crate) fn check(label: &str, path: &str) -> Result<(), String> {
    if path.contains('\\') {
        return Err(format!("{label}: the path holds a backslash"));
    }
    for part in path.split('/') {
        if part.is_empty() || part == "." || part == ".." {
            return Err(format!("{label}: the path is not a plain relative path"));
        }
    }
    Ok(())
}

/// The jar's `version.json`, byte for byte.
pub fn version_from_jar(jar: &[u8]) -> Result<Vec<u8>, String> {
    Directory::of(jar)?
        .read(jar, |name| name == VERSION_FILE)?
        .pop()
        .map(|(_, bytes)| bytes)
        .ok_or_else(|| format!("the jar has no {VERSION_FILE}"))
}

pub(crate) fn io(path: &Path, error: std::io::Error) -> String {
    format!("{}: {error}", path.display())
}

pub(crate) fn write_if_changed(dir: &Path, path: &str, bytes: &[u8]) -> Result<bool, String> {
    let full = dir.join(path);
    if fs::read(&full).is_ok_and(|held| held == bytes) {
        return Ok(false);
    }
    if let Some(parent) = full.parent() {
        fs::create_dir_all(parent).map_err(|error| io(parent, error))?;
    }
    fs::write(&full, bytes).map_err(|error| io(&full, error))?;
    Ok(true)
}

pub(crate) fn files_below(root: &Path, dir: &Path, out: &mut Vec<String>) -> Result<(), String> {
    let entries = match fs::read_dir(dir) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(()),
        Err(error) => return Err(io(dir, error)),
    };
    for entry in entries {
        let entry = entry.map_err(|error| io(dir, error))?;
        let path = entry.path();
        if entry
            .file_type()
            .map_err(|error| io(&path, error))?
            .is_dir()
        {
            files_below(root, &path, out)?;
        } else {
            let relative = path.strip_prefix(root).unwrap_or(&path);
            out.push(
                relative
                    .to_string_lossy()
                    .replace(std::path::MAIN_SEPARATOR, "/"),
            );
        }
    }
    Ok(())
}
