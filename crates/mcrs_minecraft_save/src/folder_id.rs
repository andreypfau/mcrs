use std::fs;
use std::io::ErrorKind;
use std::path::Path;

use crate::SaveError;

pub(crate) const DEFAULT_WORLD_NAME: &str = "New World";

const FALLBACK_BASE: &str = "World";

const CHANGED: [char; 16] = [
    '/', '\\', ':', '*', '?', '"', '<', '>', '|', '`', '.', '\0', '\t', '\n', '\u{c}', '\r',
];

const RESERVED: [&str; 24] = [
    "COM", "CLOCK$", "CON", "PRN", "AUX", "NUL", "COM1", "COM2", "COM3", "COM4", "COM5", "COM6",
    "COM7", "COM8", "COM9", "LPT1", "LPT2", "LPT3", "LPT4", "LPT5", "LPT6", "LPT7", "LPT8", "LPT9",
];

#[must_use]
pub(crate) fn trim_name(text: &str) -> &str {
    text.trim_matches(|character: char| character <= ' ')
}

pub(crate) fn make_free_folder(saves: &Path, name: &str) -> Result<String, SaveError> {
    fs::create_dir_all(saves).map_err(|source| SaveError::Io {
        path: saves.to_owned(),
        source,
    })?;
    let base = folder_base(name);
    match first_free(saves, &base) {
        Err(source) if base != FALLBACK_BASE && source.kind() != ErrorKind::AlreadyExists => {
            first_free(saves, FALLBACK_BASE)
        }
        found => found,
    }
    .map_err(|source| SaveError::Io {
        path: saves.join(&base),
        source,
    })
}

fn first_free(saves: &Path, base: &str) -> std::io::Result<String> {
    let mut number: u32 = 0;
    loop {
        let candidate = match number {
            0 => base.to_owned(),
            _ => format!("{base} ({number})"),
        };
        match fs::create_dir(saves.join(&candidate)) {
            Ok(()) => return Ok(candidate),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => return Err(error),
        }
        number = number
            .checked_add(1)
            .ok_or_else(|| std::io::Error::from(ErrorKind::AlreadyExists))?;
    }
}

fn folder_base(name: &str) -> String {
    let trimmed = match trim_name(name) {
        "" => DEFAULT_WORLD_NAME,
        trimmed => trimmed,
    };
    let base: String = trimmed
        .chars()
        .map(|character| {
            if CHANGED.contains(&character) {
                '_'
            } else {
                character
            }
        })
        .collect();
    if RESERVED
        .iter()
        .any(|reserved| reserved.eq_ignore_ascii_case(&base))
    {
        format!("_{base}_")
    } else {
        base
    }
}
