use std::fs;
use std::io;
use std::path::Path;

use serde::Deserialize;
use serde::Serialize;

#[derive(Debug, Default, Serialize, Deserialize)]
pub struct Facts {
    pub helpers: Vec<Helper>,
    pub sets: Vec<SetFact>,
    pub writes: Vec<SystemWrites>,
    pub always_changed: Vec<AlwaysChanged>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SystemWrites {
    pub system: String,
    pub components: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct AlwaysChanged {
    pub component: String,
    pub writer: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct Helper {
    pub owner: String,
    pub index: usize,
    pub schedule: Option<String>,
    pub run_if: bool,
    pub sets: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct SetFact {
    pub set: String,
    pub run_if: bool,
    pub parents: Vec<String>,
}

impl Facts {
    pub fn read(path: &Path) -> io::Result<Facts> {
        serde_json::from_slice(&fs::read(path)?).map_err(io::Error::other)
    }

    pub fn store(&self, path: &Path) -> io::Result<()> {
        if self.helpers.is_empty()
            && self.sets.is_empty()
            && self.writes.is_empty()
            && self.always_changed.is_empty()
        {
            return match fs::remove_file(path) {
                Err(error) if error.kind() != io::ErrorKind::NotFound => Err(error),
                _ => Ok(()),
            };
        }
        let tmp = path.with_extension("tmp");
        fs::write(&tmp, serde_json::to_vec(self).map_err(io::Error::other)?)?;
        fs::rename(tmp, path)
    }
}
