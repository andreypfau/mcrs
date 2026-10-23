use std::collections::BTreeMap;
use std::fmt;
use std::path::{Path, PathBuf};

use crate::{AvailablePack, PackDiagnostic, PackFiles, PackStack, StackedPack};

/// The directory below a pack root that holds one directory per local pack.
pub const PACKS_ROOT: &str = "mcrs/datapacks";
/// The vanilla pack's directory below a pack root. The game jar's `data/minecraft` tree fills
/// it, never the assets root.
pub const VANILLA_ROOT: &str = "minecraft";

const FEATURE_PACKS: &str = "minecraft/datapacks";
const METADATA: &str = "pack.mcmeta";
const STANDARD_DATA: &str = "data";

/// `path`, relative to a pack root, below the [`VANILLA_ROOT`] directory, if it lies there.
pub fn below_vanilla_root(path: &Path) -> Option<&Path> {
    path.strip_prefix(VANILLA_ROOT).ok()
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ReadError {
    pub path: String,
    pub reason: String,
}

impl ReadError {
    pub fn new(path: impl Into<String>, reason: impl fmt::Display) -> Self {
        Self {
            path: path.into(),
            reason: reason.to_string(),
        }
    }
}

impl fmt::Display for ReadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "{}: {}", self.path, self.reason)
    }
}

impl std::error::Error for ReadError {}

pub struct DirectoryEntry {
    pub name: String,
    pub directory: bool,
}

/// A pack root: the vanilla pack's directory at [`VANILLA_ROOT`] and one directory per local
/// pack below [`PACKS_ROOT`]. A path is relative to the root and separated by `/`.
pub trait PackSource {
    fn read(&self, path: &str) -> Result<Vec<u8>, ReadError>;

    /// A directory that does not exist lists no entries.
    fn list(&self, directory: &str) -> Result<Vec<DirectoryEntry>, ReadError>;
}

/// The packs under `assets`, with the vanilla pack's directory read from `vanilla`, which holds
/// the game jar's `data/minecraft` tree.
pub struct DirectoryPacks {
    assets: PathBuf,
    vanilla: PathBuf,
}

impl DirectoryPacks {
    pub fn new(assets: PathBuf, vanilla: PathBuf) -> Self {
        Self { assets, vanilla }
    }

    fn path(&self, path: &str) -> PathBuf {
        match below_vanilla_root(Path::new(path)) {
            Some(below) => self.vanilla.join(below),
            None => self.assets.join(path),
        }
    }
}

impl PackSource for DirectoryPacks {
    fn read(&self, path: &str) -> Result<Vec<u8>, ReadError> {
        std::fs::read(self.path(path)).map_err(|error| ReadError::new(path, error))
    }

    fn list(&self, directory: &str) -> Result<Vec<DirectoryEntry>, ReadError> {
        let entries = match std::fs::read_dir(self.path(directory)) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(ReadError::new(directory, error)),
        };
        let mut listed = Vec::new();
        for entry in entries {
            let entry = entry.map_err(|error| ReadError::new(directory, error))?;
            let kind = entry
                .file_type()
                .map_err(|error| ReadError::new(directory, error))?;
            if let Ok(name) = entry.file_name().into_string() {
                listed.push(DirectoryEntry {
                    name,
                    directory: kind.is_dir(),
                });
            }
        }
        Ok(listed)
    }
}

fn list(source: &dyn PackSource, directory: &str) -> Result<Vec<DirectoryEntry>, ReadError> {
    let mut entries = source.list(directory)?;
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

fn walk(source: &dyn PackSource, root: &str) -> Result<Vec<(String, String)>, ReadError> {
    let mut found = Vec::new();
    let mut directories = vec![root.to_owned()];
    while let Some(directory) = directories.pop() {
        for entry in list(source, &directory)? {
            let path = format!("{directory}/{}", entry.name);
            if entry.directory {
                if path != FEATURE_PACKS {
                    directories.push(path);
                }
            } else if entry.name.ends_with(".json")
                && let Some(relative) = path.strip_prefix(root)
            {
                let relative = relative.trim_start_matches('/').to_owned();
                found.push((path, relative));
            }
        }
    }
    Ok(found)
}

enum PackDirectory {
    Vanilla,
    Local(String),
}

/// The packs a pack root offers, the directory each one reads from, and the packs it cannot
/// offer.
pub struct RootPacks {
    available: Vec<AvailablePack>,
    directories: BTreeMap<String, PackDirectory>,
    unavailable: Vec<PackDiagnostic>,
}

impl RootPacks {
    pub fn vanilla() -> Self {
        let vanilla = AvailablePack::vanilla();
        Self {
            directories: BTreeMap::from([(vanilla.id().to_owned(), PackDirectory::Vanilla)]),
            available: vec![vanilla],
            unavailable: Vec::new(),
        }
    }

    /// The vanilla pack and every local pack of `source`.
    pub fn read(source: &dyn PackSource) -> Result<Self, ReadError> {
        let mut packs = Self::vanilla();
        for entry in list(source, PACKS_ROOT)?
            .into_iter()
            .filter(|e| e.directory)
        {
            let root = format!("{PACKS_ROOT}/{}", entry.name);
            let unavailable = |reason| PackDiagnostic::UnavailablePack {
                pack: format!("file/{}", entry.name),
                reason,
            };
            if list(source, &root)?
                .iter()
                .any(|e| e.directory && e.name == STANDARD_DATA)
            {
                packs.unavailable.push(unavailable(format!(
                    "{root} has a top-level {STANDARD_DATA} directory, but a local pack keeps its \
                     namespaces at the pack root"
                )));
                continue;
            }
            let pack = match source.read(&format!("{root}/{METADATA}")) {
                Ok(metadata) => AvailablePack::directory(&entry.name, metadata),
                Err(error) => {
                    packs.unavailable.push(unavailable(error.to_string()));
                    continue;
                }
            };
            packs
                .directories
                .insert(pack.id().to_owned(), PackDirectory::Local(root));
            packs.available.push(pack);
        }
        Ok(packs)
    }

    pub fn available(&self) -> &[AvailablePack] {
        &self.available
    }

    pub fn unavailable(&self) -> &[PackDiagnostic] {
        &self.unavailable
    }

    /// Reads the data of every pack of `stack` that this root offers into `files`.
    pub fn read_files(
        &self,
        source: &dyn PackSource,
        stack: &PackStack,
        files: &mut PackFiles,
    ) -> Result<(), ReadError> {
        for pack in stack.packs() {
            match self.directories.get(pack.id()) {
                Some(PackDirectory::Vanilla) => {
                    namespace_into(source, files, pack.id(), ("", STANDARD_DATA), VANILLA_ROOT)?
                }
                Some(PackDirectory::Local(root)) => local_into(source, files, pack, root)?,
                None => {}
            }
        }
        Ok(())
    }
}

fn namespace_into(
    source: &dyn PackSource,
    files: &mut PackFiles,
    pack: &str,
    (root, data): (&str, &str),
    namespace: &str,
) -> Result<(), ReadError> {
    let directory = match root {
        "" => namespace.to_owned(),
        _ => format!("{root}/{namespace}"),
    };
    for (path, relative) in walk(source, &directory)? {
        files.insert(
            pack,
            format!("{data}/{namespace}/{relative}"),
            source.read(&path)?,
        );
    }
    Ok(())
}

fn local_into(
    source: &dyn PackSource,
    files: &mut PackFiles,
    pack: &StackedPack,
    root: &str,
) -> Result<(), ReadError> {
    let overlays = pack.overlays();
    for entry in list(source, root)?.into_iter().filter(|e| e.directory) {
        if !overlays.contains(&entry.name) {
            namespace_into(source, files, pack.id(), (root, STANDARD_DATA), &entry.name)?;
        }
    }
    for overlay in overlays {
        let overlay_root = format!("{root}/{overlay}");
        let data = format!("{overlay}/{STANDARD_DATA}");
        for entry in list(source, &overlay_root)?
            .into_iter()
            .filter(|e| e.directory)
        {
            namespace_into(
                source,
                files,
                pack.id(),
                (&overlay_root, &data),
                &entry.name,
            )?;
        }
    }
    Ok(())
}
