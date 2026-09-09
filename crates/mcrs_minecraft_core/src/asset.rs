use std::path::Path;

use bevy_asset::AsyncSeekExt;
use bevy_asset::io::{AssetReaderError, ErasedAssetReader, Reader};

/// One retry is enough: the attempts are independent, and the race below has
/// been measured at roughly one read in sixty thousand.
const ATTEMPTS: usize = 2;

/// Reads one asset file whole.
///
/// The read-ahead pipe behind `async_fs::File` (piper 0.2.4) looks at the write
/// cursor before it looks at the closed flag and never looks again, so a reader
/// that polls while the writing thread publishes the last bytes and exits is
/// told EOF with the entire file still sitting in the pipe. Under load that
/// turns a whole file into zero bytes. Every attempt gets a fresh pipe, and a
/// file that is really empty reads empty every time.
pub async fn read_whole(
    reader: &dyn ErasedAssetReader,
    path: &Path,
) -> Result<Vec<u8>, AssetReaderError> {
    let mut bytes = Vec::new();
    for _ in 0..ATTEMPTS {
        let mut file = reader.read(path).await?;
        file.read_to_end(&mut bytes).await?;
        if !bytes.is_empty() {
            break;
        }
    }
    Ok(bytes)
}

/// Reads what an [`AssetLoader`](bevy_asset::AssetLoader) has been handed, whole.
///
/// Same race as [`read_whole`] one level down. A loader cannot reopen the file
/// it was given, so the recovery is to seek back to the start: that drops the
/// pipe that lied and the next read takes a fresh one.
pub async fn read_all(reader: &mut dyn Reader) -> std::io::Result<Vec<u8>> {
    let mut bytes = Vec::new();
    reader.read_to_end(&mut bytes).await?;
    if bytes.is_empty()
        && let Ok(seekable) = reader.seekable()
    {
        seekable.seek(std::io::SeekFrom::Start(0)).await?;
        seekable.read_to_end(&mut bytes).await?;
    }
    Ok(bytes)
}

/// Loads any asset that is nothing but its JSON: parse the file, hand back the
/// value. An asset that names other assets needs a loader of its own, because
/// only that loader knows which ids to turn into handles.
pub struct JsonLoader<A>(std::marker::PhantomData<fn() -> A>);

impl<A> Default for JsonLoader<A> {
    fn default() -> Self {
        Self(std::marker::PhantomData)
    }
}

impl<A: bevy_reflect::TypePath> bevy_reflect::TypePath for JsonLoader<A> {
    fn type_path() -> &'static str {
        "mcrs_minecraft_core::asset::JsonLoader"
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
        Ok(serde_json::from_slice(&read_all(reader).await?)?)
    }
}
