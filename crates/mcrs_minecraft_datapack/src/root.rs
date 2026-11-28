use std::collections::BTreeMap;
use std::fmt;
use std::path::{Component, Path, PathBuf};

use crate::{
    AvailablePack, Features, PackConflict, PackDiagnostic, PackFiles, PackSelection, PackStack,
    SelectedStack, StackedPack, pack_stack,
};

pub const PACKS_ROOT: &str = "mcrs/datapacks";
pub const VANILLA_ROOT: &str = "minecraft";
pub const REPOSITORY_PACKS: [&str; 2] = ["mcrs", "beta"];

const FEATURE_PACKS: &str = "minecraft/datapacks";
const METADATA: &str = "pack.mcmeta";
const STANDARD_DATA: &str = "data";
const FILE_PREFIX: &str = "file/";

pub fn below_vanilla_root(path: &Path) -> Option<&Path> {
    path.strip_prefix(VANILLA_ROOT).ok()
}

pub fn read_vanilla(source: &dyn PackSource, path: &str) -> Result<Vec<u8>, ReadError> {
    let below = (path.strip_prefix(STANDARD_DATA))
        .and_then(|below| below.strip_prefix('/'))
        .map(Path::new)
        .filter(|below| {
            below
                .components()
                .all(|c| matches!(c, Component::Normal(_)))
        })
        .filter(|below| below_vanilla_root(below).is_some())
        .and_then(Path::to_str);
    match below {
        Some(below) => source.read(below),
        None => Err(ReadError::new(path, "the vanilla pack holds no such file")),
    }
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

impl core::error::Error for ReadError {}

pub struct DirectoryEntry {
    pub name: String,
    pub directory: bool,
}

pub trait PackSource {
    fn read(&self, path: &str) -> Result<Vec<u8>, ReadError>;

    fn list(&self, directory: &str) -> Result<Vec<DirectoryEntry>, ReadError>;

    fn standard_packs(&self) -> &[String] {
        &[]
    }
}

#[derive(Clone)]
pub struct DirectoryPacks {
    assets: PathBuf,
    vanilla: PathBuf,
    standard_names: Vec<String>,
    standard_directories: Vec<PathBuf>,
}

impl DirectoryPacks {
    pub fn new(assets: PathBuf, vanilla: PathBuf) -> Self {
        Self {
            assets,
            vanilla,
            standard_names: Vec::new(),
            standard_directories: Vec::new(),
        }
    }

    pub fn with_pack(mut self, directory: &Path) -> Result<Self, ReadError> {
        let shown = directory.display().to_string();
        let directory =
            std::fs::canonicalize(directory).map_err(|error| ReadError::new(&shown, error))?;
        let name = (directory.file_name().and_then(|name| name.to_str()))
            .ok_or_else(|| ReadError::new(&shown, "the directory name is not a pack name"))?
            .to_owned();
        if self.standard_names.contains(&name) {
            return Err(ReadError::new(
                &shown,
                format!("another pack is named {name}"),
            ));
        }
        self.standard_names.push(name);
        self.standard_directories.push(directory);
        Ok(self)
    }

    fn path(&self, path: &str) -> PathBuf {
        let path = Path::new(path);
        if let Ok(packs) = path.strip_prefix(PACKS_ROOT) {
            for (name, directory) in self.standard_names.iter().zip(&self.standard_directories) {
                if let Ok(below) = packs.strip_prefix(name) {
                    return directory.join(below);
                }
            }
        }
        match below_vanilla_root(path) {
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
        let mut listed = Vec::new();
        let packs_root = directory == PACKS_ROOT;
        if packs_root {
            listed.extend(self.standard_names.iter().map(|name| DirectoryEntry {
                name: name.clone(),
                directory: true,
            }));
        }
        let entries = match std::fs::read_dir(self.path(directory)) {
            Ok(entries) => entries,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(listed),
            Err(error) => return Err(ReadError::new(directory, error)),
        };
        for entry in entries {
            let entry = entry.map_err(|error| ReadError::new(directory, error))?;
            let kind = entry
                .file_type()
                .map_err(|error| ReadError::new(directory, error))?;
            if let Ok(name) = entry.file_name().into_string()
                && !(packs_root && self.standard_names.contains(&name))
            {
                listed.push(DirectoryEntry {
                    name,
                    directory: kind.is_dir(),
                });
            }
        }
        Ok(listed)
    }

    fn standard_packs(&self) -> &[String] {
        &self.standard_names
    }
}

fn list(source: &dyn PackSource, directory: &str) -> Result<Vec<DirectoryEntry>, ReadError> {
    let mut entries = source.list(directory)?;
    entries.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(entries)
}

fn walk(
    source: &dyn PackSource,
    root: &str,
    extension: &str,
) -> Result<Vec<(String, String)>, ReadError> {
    let mut found = Vec::new();
    let mut directories = vec![root.to_owned()];
    while let Some(directory) = directories.pop() {
        for entry in list(source, &directory)? {
            let path = format!("{directory}/{}", entry.name);
            if entry.directory {
                if path != FEATURE_PACKS {
                    directories.push(path);
                }
            } else if entry.name.ends_with(extension)
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
    Standard(String),
}

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

    pub fn read(source: &dyn PackSource) -> Result<Self, ReadError> {
        let mut packs = Self::vanilla();
        let standard_packs = source.standard_packs();
        for entry in list(source, PACKS_ROOT)?
            .into_iter()
            .filter(|e| e.directory)
        {
            let root = format!("{PACKS_ROOT}/{}", entry.name);
            let unavailable = |reason| PackDiagnostic::UnavailablePack {
                pack: format!("file/{}", entry.name),
                reason,
            };
            let standard = standard_packs.contains(&entry.name);
            if !standard
                && list(source, &root)?
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
            let directory = match standard {
                true => PackDirectory::Standard(root),
                false => PackDirectory::Local(root),
            };
            packs.directories.insert(pack.id().to_owned(), directory);
            packs.available.push(pack);
        }
        Ok(packs)
    }

    pub fn stack(
        &self,
        standard_packs: &[String],
        local_packs: &[String],
        embedded_packs: Vec<String>,
    ) -> Result<SelectedStack, PackConflict> {
        if let Some(name) = standard_packs
            .iter()
            .find(|name| local_packs.contains(name))
        {
            return Err(PackConflict::LocalNamedAsStandard(name.clone()));
        }
        let standard = |name: &str| standard_packs.iter().any(|standard| standard == name);
        let local = |name: &str| local_packs.iter().any(|local| local == name);
        let mut selection = PackSelection::default();
        let mut available: Vec<AvailablePack> = (self.available.iter())
            .filter(|pack| {
                (pack.id().strip_prefix(FILE_PREFIX))
                    .is_none_or(|name| local(name) || standard(name))
            })
            .cloned()
            .collect();
        for id in embedded_packs {
            if available.iter().any(|pack| pack.id() == id) {
                return Err(PackConflict::EmbeddedIdTaken(id));
            }
            selection.enabled.push(id.clone());
            available.push(AvailablePack::embedded(id));
        }
        if !standard_packs.is_empty() {
            let mut local_ids: Vec<&str> = (available.iter())
                .map(AvailablePack::id)
                .filter(|id| {
                    id.strip_prefix(FILE_PREFIX)
                        .is_some_and(|name| !standard(name))
                })
                .collect();
            local_ids.sort_unstable();
            let standard_ids = standard_packs
                .iter()
                .map(|name| format!("{FILE_PREFIX}{name}"))
                .filter(|id| available.iter().any(|pack| pack.id() == id));
            let enabled = local_ids.into_iter().map(str::to_owned).chain(standard_ids);
            selection.enabled.extend(enabled);
        }
        let mut selected = pack_stack(&selection, &available, &Features::vanilla(), false);
        selected
            .diagnostics
            .extend(self.unavailable.iter().cloned());
        Ok(selected)
    }

    pub fn read_files(
        &self,
        source: &dyn PackSource,
        stack: &PackStack,
        files: &mut PackFiles,
        extension: &str,
        needs: impl Fn(&str) -> bool,
    ) -> Result<(), ReadError> {
        for pack in stack.packs() {
            match self.directories.get(pack.id()) {
                Some(PackDirectory::Vanilla) => read_tree(
                    source,
                    files,
                    pack.id(),
                    VANILLA_ROOT,
                    &format!("{STANDARD_DATA}/{VANILLA_ROOT}"),
                    extension,
                    &needs,
                )?,
                Some(PackDirectory::Local(root)) => {
                    local_into(source, files, pack, root, extension, &needs)?
                }
                Some(PackDirectory::Standard(root)) => {
                    standard_into(source, files, pack, root, extension, &needs)?;
                }
                None => {}
            }
        }
        Ok(())
    }
}

fn read_tree(
    source: &dyn PackSource,
    files: &mut PackFiles,
    pack: &str,
    directory: &str,
    in_pack: &str,
    extension: &str,
    needs: &impl Fn(&str) -> bool,
) -> Result<(), ReadError> {
    for (path, relative) in walk(source, directory, extension)? {
        let in_pack = format!("{in_pack}/{relative}");
        if !files.holds_value(pack, &in_pack) {
            let bytes = match needs(&relative) {
                true => source.read(&path)?,
                false => Vec::new(),
            };
            files.insert(pack, in_pack, bytes);
        }
    }
    Ok(())
}

fn local_into(
    source: &dyn PackSource,
    files: &mut PackFiles,
    pack: &StackedPack,
    root: &str,
    extension: &str,
    needs: &impl Fn(&str) -> bool,
) -> Result<(), ReadError> {
    let overlays = &pack.overlays;
    for entry in list(source, root)?.into_iter().filter(|e| e.directory) {
        if !overlays.contains(&entry.name) {
            let directory = format!("{root}/{}", entry.name);
            let in_pack = format!("{STANDARD_DATA}/{}", entry.name);
            read_tree(
                source,
                files,
                pack.id(),
                &directory,
                &in_pack,
                extension,
                needs,
            )?;
        }
    }
    for overlay in overlays {
        let overlay_root = format!("{root}/{overlay}");
        for entry in list(source, &overlay_root)?
            .into_iter()
            .filter(|e| e.directory)
        {
            let directory = format!("{overlay_root}/{}", entry.name);
            let in_pack = format!("{overlay}/{STANDARD_DATA}/{}", entry.name);
            read_tree(
                source,
                files,
                pack.id(),
                &directory,
                &in_pack,
                extension,
                needs,
            )?;
        }
    }
    Ok(())
}

fn standard_into(
    source: &dyn PackSource,
    files: &mut PackFiles,
    pack: &StackedPack,
    root: &str,
    extension: &str,
    needs: &impl Fn(&str) -> bool,
) -> Result<(), ReadError> {
    let overlays = pack.overlays.iter().map(|o| format!("{o}/{STANDARD_DATA}"));
    for data in std::iter::once(STANDARD_DATA.to_owned()).chain(overlays) {
        let data_root = format!("{root}/{data}");
        for entry in list(source, &data_root)?
            .into_iter()
            .filter(|e| e.directory)
        {
            let directory = format!("{data_root}/{}", entry.name);
            let in_pack = format!("{data}/{}", entry.name);
            read_tree(
                source,
                files,
                pack.id(),
                &directory,
                &in_pack,
                extension,
                needs,
            )?;
        }
    }
    Ok(())
}
