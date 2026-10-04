use crate::asset::read_whole;
use bevy_asset::io::{
    AssetReader, AssetReaderError, AssetSource, AssetSourceBuilder, ErasedAssetReader, PathStream,
    Reader, VecReader,
};
use bevy_tasks::futures_lite::StreamExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

pub const PACKS_ROOT: &str = "mcrs/datapacks";

pub async fn pack_names(reader: &dyn ErasedAssetReader) -> Vec<String> {
    let Ok(mut listing) = reader.read_directory(Path::new(PACKS_ROOT)).await else {
        return Vec::new();
    };
    let mut names = Vec::new();
    while let Some(path) = listing.next().await {
        if reader.is_directory(&path).await.unwrap_or(false)
            && let Some(name) = path.file_name().and_then(|name| name.to_str())
        {
            names.push(name.to_owned());
        }
    }
    names.sort();
    names
}

pub struct PackLayers {
    inner: Box<dyn ErasedAssetReader>,
    packs: OnceLock<Vec<String>>,
}

impl PackLayers {
    pub fn new(inner: Box<dyn ErasedAssetReader>) -> Self {
        PackLayers {
            inner,
            packs: OnceLock::new(),
        }
    }

    async fn packs(&self) -> &[String] {
        match self.packs.get() {
            Some(packs) => packs,
            None => {
                let found = pack_names(&*self.inner).await;
                self.packs.get_or_init(|| found)
            }
        }
    }
}

fn refused(path: &Path, first: &Path, second: &Path) -> AssetReaderError {
    // chisle: the game lets the later pack win; a path two layers hold is refused instead, and pack stacking with override lifts this
    AssetReaderError::Io(Arc::new(std::io::Error::new(
        std::io::ErrorKind::InvalidData,
        format!(
            "{} is held by both {} and {}",
            path.display(),
            first.display(),
            second.display()
        ),
    )))
}

pub fn layered_file_source(root: &str) -> AssetSourceBuilder {
    let mut files = AssetSource::get_default_reader(root.to_owned());
    AssetSourceBuilder::platform_default(root, None)
        .with_reader(move || Box::new(PackLayers::new(files())))
}

impl AssetReader for PackLayers {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<Box<dyn Reader + 'a>, AssetReaderError> {
        let packs = self.packs().await;
        if packs.is_empty() {
            return self.inner.read(path).await;
        }

        let mut held: Option<(PathBuf, Box<dyn Reader + 'a>)> = None;
        match self.inner.read(path).await {
            Ok(reader) => held = Some((path.to_path_buf(), reader)),
            Err(AssetReaderError::NotFound(_)) => {}
            Err(error) => return Err(error),
        }
        for pack in packs {
            let at = Path::new(PACKS_ROOT).join(pack).join(path);
            match read_whole(&*self.inner, &at).await {
                Ok(bytes) => {
                    if let Some((first, _)) = &held {
                        return Err(refused(path, first, &at));
                    }
                    held = Some((at, Box::new(VecReader::new(bytes))));
                }
                Err(AssetReaderError::NotFound(_)) => {}
                Err(error) => return Err(error),
            }
        }
        held.map(|(_, reader)| reader)
            .ok_or_else(|| AssetReaderError::NotFound(path.to_path_buf()))
    }

    async fn read_meta<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<dyn Reader + 'a>, AssetReaderError> {
        self.inner.read_meta(path).await
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        self.inner.read_directory(path).await
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        self.inner.is_directory(path).await
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use bevy_asset::io::memory::{Dir, MemoryAssetReader};

    fn layered(files: &[(&str, &str)]) -> PackLayers {
        let root = Dir::default();
        for (path, text) in files {
            root.insert_asset_text(Path::new(path), text);
        }
        PackLayers::new(Box::new(MemoryAssetReader { root }))
    }

    fn read(layers: &PackLayers, path: &str) -> Result<String, AssetReaderError> {
        let bytes = bevy_tasks::block_on(read_whole(layers, Path::new(path)))?;
        Ok(String::from_utf8(bytes).unwrap())
    }

    #[test]
    fn a_name_reads_from_the_one_layer_that_holds_it() {
        let layers = layered(&[
            ("minecraft/root_only.json", "root"),
            ("minecraft/shared.json", "root"),
            ("mcrs/datapacks/a/minecraft/shared_packs.json", "a"),
            ("mcrs/datapacks/b/minecraft/pack_only.json", "b"),
            ("mcrs/datapacks/b/minecraft/shared.json", "b"),
            ("mcrs/datapacks/b/minecraft/shared_packs.json", "b"),
        ]);

        assert_eq!(read(&layers, "minecraft/root_only.json").unwrap(), "root");
        assert_eq!(read(&layers, "minecraft/pack_only.json").unwrap(), "b");

        for (path, held_by) in [
            (
                "minecraft/shared.json",
                ["minecraft/shared.json", "datapacks/b"],
            ),
            (
                "minecraft/shared_packs.json",
                ["datapacks/a/minecraft", "datapacks/b/minecraft"],
            ),
        ] {
            let message = read(&layers, path).unwrap_err().to_string();
            for layer in held_by {
                assert!(message.contains(layer), "{layer} missing from: {message}");
            }
        }

        assert!(matches!(
            read(&layers, "minecraft/absent.json"),
            Err(AssetReaderError::NotFound(_))
        ));
    }
}
