use std::path::Path;

use bevy_asset::io::{AssetReaderError, ErasedAssetReader, Reader};
use bevy_tasks::block_on;
use bevy_tasks::futures_lite::StreamExt;
use mcrs_minecraft_registry::RegistrySet;

#[derive(Debug, thiserror::Error)]
pub enum CorpusReadError {
    #[error("failed to list `{directory}`: {source}")]
    ListDirectory {
        directory: String,
        source: AssetReaderError,
    },
    #[error("failed to read `{path}`: {source}")]
    Read {
        path: String,
        source: AssetReaderError,
    },
}

/// Every `.json` file directly under `directory`, in path order, read whole.
///
/// One block_on for the whole corpus: entering the executor per file cost
/// far more than reading or parsing the 1289 block files put together.
pub fn read_json_corpus(
    reader: &dyn ErasedAssetReader,
    directory: &str,
) -> Result<Vec<(String, Vec<u8>)>, CorpusReadError> {
    block_on(async {
        let mut stream = reader
            .read_directory(Path::new(directory))
            .await
            .map_err(|source| CorpusReadError::ListDirectory {
                directory: directory.into(),
                source,
            })?;
        let mut paths = Vec::new();
        while let Some(path) = stream.next().await {
            if path.extension().is_some_and(|e| e == "json") {
                paths.push(path);
            }
        }
        paths.sort();
        let mut files = Vec::with_capacity(paths.len());
        for path in paths {
            let display = path.display().to_string();
            let bytes =
                read_whole(reader, &path)
                    .await
                    .map_err(|source| CorpusReadError::Read {
                        path: display.clone(),
                        source,
                    })?;
            files.push((display, bytes));
        }
        Ok(files)
    })
}

/// Reads one asset file whole.
///
/// Relies on piper 0.2.5 or later behind `async_fs::File`: 0.2.4 can report EOF
/// while the last chunk of a file is still in its read-ahead pipe, which
/// truncates the file anywhere, not only to zero bytes.
pub async fn read_whole(
    reader: &dyn ErasedAssetReader,
    path: &Path,
) -> Result<Vec<u8>, AssetReaderError> {
    let mut bytes = Vec::new();
    reader.read(path).await?.read_to_end(&mut bytes).await?;
    Ok(bytes)
}

/// Loads any asset that is nothing but its JSON: parse the file inside the
/// loaded registry set's scope, hand back the value. An asset that names other
/// assets needs a loader of its own, because only that loader knows which ids
/// to turn into handles.
pub struct JsonLoader<A> {
    registries: RegistrySet,
    asset: std::marker::PhantomData<fn() -> A>,
}

impl<A> JsonLoader<A> {
    pub fn new(registries: RegistrySet) -> Self {
        Self {
            registries,
            asset: std::marker::PhantomData,
        }
    }
}

impl<A: bevy_reflect::TypePath> bevy_reflect::TypePath for JsonLoader<A> {
    fn type_path() -> &'static str {
        "mcrs_minecraft_assets::asset::JsonLoader"
    }

    fn short_type_path() -> &'static str {
        "JsonLoader"
    }
}

#[derive(Debug, thiserror::Error)]
pub enum JsonLoaderError {
    #[error(transparent)]
    Io(#[from] std::io::Error),
    #[error("JSON parse error: {0}")]
    Json(#[from] serde_json::Error),
}

impl<A> bevy_asset::AssetLoader for JsonLoader<A>
where
    A: bevy_asset::Asset + serde::de::DeserializeOwned,
{
    type Asset = A;
    type Settings = ();
    type Error = JsonLoaderError;

    async fn load(
        &self,
        reader: &mut dyn Reader,
        _settings: &(),
        _load_context: &mut bevy_asset::LoadContext<'_>,
    ) -> Result<A, JsonLoaderError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes).await?;
        Ok(self.registries.scope(|| serde_json::from_slice(&bytes))?)
    }
}
