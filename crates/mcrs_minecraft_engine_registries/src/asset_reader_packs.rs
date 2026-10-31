use mcrs_minecraft_datapack::{PackSource, ReadError};

/// The pack files an asset reader serves, read as the registry loader reads a pack root.
pub(crate) struct AssetReaderPacks<'a>(pub(crate) &'a dyn bevy_asset::io::ErasedAssetReader);

impl PackSource for AssetReaderPacks<'_> {
    fn read(&self, path: &str) -> Result<Vec<u8>, ReadError> {
        bevy_tasks::block_on(mcrs_minecraft_assets::asset::read_whole(
            self.0,
            std::path::Path::new(path),
        ))
        .map_err(|error| ReadError::new(path, error))
    }

    fn list(
        &self,
        directory: &str,
    ) -> Result<Vec<mcrs_minecraft_datapack::DirectoryEntry>, ReadError> {
        use bevy_asset::io::AssetReaderError;
        use bevy_tasks::futures_lite::StreamExt;
        bevy_tasks::block_on(async {
            let mut paths = match self.0.read_directory(std::path::Path::new(directory)).await {
                Ok(paths) => paths,
                Err(AssetReaderError::NotFound(_)) => return Ok(Vec::new()),
                Err(error) => return Err(ReadError::new(directory, error)),
            };
            let mut listed = Vec::new();
            while let Some(path) = paths.next().await {
                let directory = self
                    .0
                    .is_directory(&path)
                    .await
                    .map_err(|error| ReadError::new(path.display().to_string(), error))?;
                if let Some(name) = path.file_name().and_then(|name| name.to_str()) {
                    listed.push(mcrs_minecraft_datapack::DirectoryEntry {
                        name: name.to_owned(),
                        directory,
                    });
                }
            }
            Ok(listed)
        })
    }
}
