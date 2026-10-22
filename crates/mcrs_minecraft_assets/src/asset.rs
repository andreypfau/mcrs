use std::path::Path;

use bevy_asset::io::{AssetReaderError, ErasedAssetReader, Reader};
use bevy_tasks::block_on;
use bevy_tasks::futures_lite::StreamExt;

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
