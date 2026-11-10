use std::collections::HashMap;
use std::error::Error;
use std::fmt;
use std::path::{Path, PathBuf};

use bevy::asset::AssetServer;
use bevy::asset::io::{AssetSourceId, ErasedAssetReader};
use bevy::prelude::Resource;
use bevy::tasks::futures_lite::StreamExt;

use crate::block_entity_model::{BlockEntityModels, DATA_DIRECTORY, GEOMETRY_DIRECTORY};
use crate::vanilla_source::SOURCE;
use mcrs_minecraft_block::definition::CORPUS_DIRECTORY;
use mcrs_minecraft_model_file::pack_files::{PackError, PackFiles};

/// The directories of the default asset source that the static block entities read.
const BLOCK_ENTITY_DIRECTORIES: [&str; 3] = [DATA_DIRECTORY, GEOMETRY_DIRECTORY, CORPUS_DIRECTORY];

/// The folders under `assets/<namespace>/` that the client reads.
const FOLDERS: [&str; 8] = [
    "equipment",
    "blockstates",
    "models",
    "textures",
    "items",
    "atlases",
    "font",
    "lang",
];

/// Why the pack could not be read from the asset sources.
#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PackLoadError {
    /// The app registers no asset source of this name.
    NoSource { source: String, reason: String },
    /// A file of an asset source could not be read.
    Read { path: String, reason: String },
    /// Two layers supply the same file.
    Pack(PackError),
}

impl fmt::Display for PackLoadError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            PackLoadError::NoSource { source, reason } => {
                write!(f, "no {source} asset source: {reason}")
            }
            PackLoadError::Read { path, reason } => write!(f, "cannot read {path}: {reason}"),
            PackLoadError::Pack(error) => error.fmt(f),
        }
    }
}

impl Error for PackLoadError {}

impl From<PackError> for PackLoadError {
    fn from(error: PackError) -> Self {
        PackLoadError::Pack(error)
    }
}

/// The files of the client resource pack that the client reads, each keyed by its path below
/// `assets/`, such as `minecraft/models/block/stone.json`.
#[derive(Resource, Default, Debug)]
pub struct ResourceSet {
    files: PackFiles,
    block_entity_models: BlockEntityModels,
}

impl ResourceSet {
    /// Reads every file of the `vanilla` asset source that [`is_resource`] picks, and the
    /// static block entity files of the default asset source.
    pub async fn load(assets: &AssetServer) -> Result<Self, PackLoadError> {
        let vanilla = assets
            .get_source(AssetSourceId::from(SOURCE))
            .map_err(|error| PackLoadError::NoSource {
                source: SOURCE.to_owned(),
                reason: error.to_string(),
            })?;
        let mut set = Self::default();
        for (path, bytes) in read_tree(vanilla.reader(), "").await? {
            if is_resource(&path) {
                set.add_file(&path, bytes)?;
            }
        }
        let own =
            assets
                .get_source(AssetSourceId::Default)
                .map_err(|error| PackLoadError::NoSource {
                    source: "default".to_owned(),
                    reason: error.to_string(),
                })?;
        let mut layer = HashMap::new();
        for directory in BLOCK_ENTITY_DIRECTORIES {
            layer.extend(read_tree(own.reader(), directory).await?);
        }
        set.block_entity_models = BlockEntityModels::from_files(&layer);
        Ok(set)
    }

    /// The static block entities of the pack layer of mcrs.
    #[must_use]
    pub fn block_entity_models(&self) -> &BlockEntityModels {
        &self.block_entity_models
    }

    /// The files of the pack.
    #[must_use]
    pub fn files(&self) -> &PackFiles {
        &self.files
    }

    /// Adds a file at its path below `assets/`. A file of an extra pack layer never replaces a
    /// file that the set holds.
    pub fn add_file(&mut self, path: &str, bytes: Vec<u8>) -> Result<(), PackError> {
        self.files.add_file(path, bytes)
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.files.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.files.is_empty()
    }

    /// The bytes of the file at `path` below `assets/`.
    #[must_use]
    pub fn get(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path)
    }

    /// The bytes of the file at `path`, or an error that names the path.
    pub fn read(&self, path: &str) -> Result<&[u8], PackError> {
        self.files.read(path)
    }

    /// Each file `<namespace>/<folder>/<path>.<extension>`, as the identifier
    /// `<namespace>:<path>` and its bytes, in path order.
    pub fn entries<'a>(
        &'a self,
        folder: &'a str,
        extension: &'a str,
    ) -> impl Iterator<Item = (String, &'a [u8])> + 'a {
        self.files.entries(folder, extension)
    }

    /// The resource half of the installed client jar.
    ///
    /// # Panics
    ///
    /// When the client jar can not be installed or read.
    #[cfg(any(test, feature = "test-support"))]
    pub fn corpus() -> &'static Self {
        static SET: std::sync::LazyLock<ResourceSet> =
            std::sync::LazyLock::new(ResourceSet::read_corpus);
        &SET
    }

    #[cfg(any(test, feature = "test-support"))]
    fn read_corpus() -> Self {
        let mut set: Self = crate::vanilla_source::resource_files()
            .iter()
            .map(|(path, bytes)| (path.clone(), bytes.clone()))
            .collect();
        let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
        let mut layer = HashMap::new();
        for directory in BLOCK_ENTITY_DIRECTORIES {
            let mut pending = vec![root.join(directory)];
            while let Some(at) = pending.pop() {
                let Ok(entries) = std::fs::read_dir(&at) else {
                    continue;
                };
                for entry in entries.flatten() {
                    let path = entry.path();
                    if path.is_dir() {
                        pending.push(path);
                    } else if let (Ok(key), Ok(bytes)) =
                        (path.strip_prefix(&root), std::fs::read(&path))
                    {
                        layer.insert(key.to_string_lossy().replace('\\', "/"), bytes);
                    }
                }
            }
        }
        set.block_entity_models = BlockEntityModels::from_files(&layer);
        set
    }

    /// [`ResourceSet::corpus`], shared.
    ///
    /// # Panics
    ///
    /// When the client jar can not be installed or read.
    #[cfg(any(test, feature = "test-support"))]
    pub fn shared_corpus() -> std::sync::Arc<Self> {
        static SET: std::sync::LazyLock<std::sync::Arc<ResourceSet>> =
            std::sync::LazyLock::new(|| std::sync::Arc::new(ResourceSet::read_corpus()));
        SET.clone()
    }
}

/// The `assets/` files of the client jar, each at its path below `assets/`. A file outside the
/// resource set is dropped; a second file at one path keeps the first.
impl FromIterator<(String, Vec<u8>)> for ResourceSet {
    fn from_iter<I: IntoIterator<Item = (String, Vec<u8>)>>(files: I) -> Self {
        let mut set = Self::default();
        for (path, bytes) in files {
            if is_resource(&path) {
                let _ = set.add_file(&path, bytes);
            }
        }
        set
    }
}

/// Each file below `directory` of an asset source, keyed by its path with `/` separators.
async fn read_tree(
    reader: &dyn ErasedAssetReader,
    directory: &str,
) -> Result<Vec<(String, Vec<u8>)>, PackLoadError> {
    let mut files = Vec::new();
    let mut pending = vec![PathBuf::from(directory)];
    while let Some(directory) = pending.pop() {
        let Ok(mut listing) = reader.read_directory(&directory).await else {
            continue;
        };
        while let Some(path) = listing.next().await {
            if reader.is_directory(&path).await.unwrap_or(false) {
                pending.push(path);
                continue;
            }
            let bytes = read_whole(reader, &path).await?;
            files.push((path.to_string_lossy().replace('\\', "/"), bytes));
        }
    }
    Ok(files)
}

async fn read_whole(reader: &dyn ErasedAssetReader, path: &Path) -> Result<Vec<u8>, PackLoadError> {
    use bevy::asset::io::Reader;
    let mut bytes = Vec::new();
    let mut file = reader
        .read(path)
        .await
        .map_err(|error| PackLoadError::Read {
            path: path.display().to_string(),
            reason: error.to_string(),
        })?;
    file.read_to_end(&mut bytes)
        .await
        .map_err(|error| PackLoadError::Read {
            path: path.display().to_string(),
            reason: error.to_string(),
        })?;
    Ok(bytes)
}

/// Whether a path below `assets/` of the client jar names a file of the resource set: a file
/// under `<namespace>/<folder>/` for a folder that the client reads.
#[must_use]
pub fn is_resource(path: &str) -> bool {
    let mut segments = path.split('/');
    let (Some(namespace), Some(folder), Some(_)) =
        (segments.next(), segments.next(), segments.next())
    else {
        return false;
    };
    !namespace.is_empty() && FOLDERS.contains(&folder) && !path.ends_with('/')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_layer_file_never_replaces_a_file_of_the_client_pack() {
        let mut set: ResourceSet = [(
            "minecraft/models/block/stone.json".to_owned(),
            b"{}".to_vec(),
        )]
        .into_iter()
        .collect();
        assert!(set.get("minecraft/models/block/stone.json").is_some());
        assert!(
            set.add_file("minecraft/models/block/stone.json", Vec::new())
                .is_err()
        );
        assert!(
            set.add_file("mcrs/models/block/extra.json", Vec::new())
                .is_ok()
        );
    }

    #[test]
    fn entries_name_each_file_of_a_folder_by_its_identifier() {
        let set: ResourceSet = [
            ("minecraft/items/stick.json", "a"),
            ("minecraft/items/sub/apple.json", "b"),
            ("minecraft/itemsx/no.json", "c"),
            ("minecraft/data/items/no.json", "d"),
        ]
        .into_iter()
        .map(|(path, text)| (path.to_owned(), text.as_bytes().to_vec()))
        .collect();
        let ids: Vec<String> = set.entries("items", "json").map(|(id, _)| id).collect();
        assert_eq!(ids, ["minecraft:stick", "minecraft:sub/apple"]);
    }
}
