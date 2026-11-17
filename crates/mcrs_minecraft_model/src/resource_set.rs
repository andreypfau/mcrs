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

const BLOCK_ENTITY_DIRECTORIES: [&str; 3] = [DATA_DIRECTORY, GEOMETRY_DIRECTORY, CORPUS_DIRECTORY];

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

#[derive(Clone, Debug, PartialEq, Eq)]
#[non_exhaustive]
pub enum PackLoadError {
    NoSource { source: String, reason: String },
    Read { path: String, reason: String },
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

#[derive(Resource, Default, Debug)]
pub struct ResourceSet {
    files: PackFiles,
    block_entity_models: BlockEntityModels,
}

impl ResourceSet {
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

    #[must_use]
    pub fn block_entity_models(&self) -> &BlockEntityModels {
        &self.block_entity_models
    }

    #[must_use]
    pub fn files(&self) -> &PackFiles {
        &self.files
    }

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

    #[must_use]
    pub fn get(&self, path: &str) -> Option<&[u8]> {
        self.files.get(path)
    }

    pub fn read(&self, path: &str) -> Result<&[u8], PackError> {
        self.files.read(path)
    }

    pub fn entries<'a>(
        &'a self,
        folder: &'a str,
        extension: &'a str,
    ) -> impl Iterator<Item = (String, &'a [u8])> + 'a {
        self.files.entries(folder, extension)
    }

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

    #[cfg(any(test, feature = "test-support"))]
    pub fn shared_corpus() -> std::sync::Arc<Self> {
        static SET: std::sync::LazyLock<std::sync::Arc<ResourceSet>> =
            std::sync::LazyLock::new(|| std::sync::Arc::new(ResourceSet::read_corpus()));
        SET.clone()
    }
}

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
