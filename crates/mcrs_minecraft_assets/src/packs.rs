use crate::asset::read_whole;
use bevy_asset::io::{
    AssetReader, AssetReaderError, AssetSource, AssetSourceBuilder, ErasedAssetReader, PathStream,
    Reader, VecReader,
};
use bevy_tasks::futures_lite::StreamExt;
use std::path::{Path, PathBuf};
use std::sync::{Arc, OnceLock};

pub use mcrs_minecraft_registry::{PACKS_ROOT, VANILLA_PACK};

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

pub type BuiltinAsset = fn(&str) -> Option<Vec<u8>>;

/// The packs layered over `inner`, answering from `builtin` for a path no layer
/// holds. A file always wins, so a datapack overrides a built-in by shipping
/// the same path.
pub fn layered_reader(
    inner: Box<dyn ErasedAssetReader>,
    builtin: BuiltinAsset,
) -> Box<dyn ErasedAssetReader> {
    Box::new(BuiltinFallback {
        layers: PackLayers::new(inner),
        builtin,
    })
}

pub fn layered_file_source(root: &str, builtin: BuiltinAsset) -> AssetSourceBuilder {
    let mut files = AssetSource::get_default_reader(root.to_owned());
    AssetSourceBuilder::platform_default(root, None)
        .with_reader(move || layered_reader(files(), builtin))
}

struct BuiltinFallback {
    layers: PackLayers,
    builtin: BuiltinAsset,
}

impl AssetReader for BuiltinFallback {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<Box<dyn Reader + 'a>, AssetReaderError> {
        match AssetReader::read(&self.layers, path).await {
            Err(AssetReaderError::NotFound(missing)) => path
                .to_str()
                .and_then(self.builtin)
                .map(|bytes| Box::new(VecReader::new(bytes)) as Box<dyn Reader>)
                .ok_or(AssetReaderError::NotFound(missing)),
            read => read,
        }
    }

    async fn read_meta<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<dyn Reader + 'a>, AssetReaderError> {
        AssetReader::read_meta(&self.layers, path).await
    }

    // chisle: a directory listing shows files only. The registry loader adds
    // the built-in paths itself; merging the listings here lifts that.
    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        AssetReader::read_directory(&self.layers, path).await
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        AssetReader::is_directory(&self.layers, path).await
    }
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

    fn read(layers: &dyn ErasedAssetReader, path: &str) -> Result<String, AssetReaderError> {
        let bytes = bevy_tasks::block_on(read_whole(layers, Path::new(path)))?;
        Ok(String::from_utf8(bytes).unwrap())
    }

    #[test]
    fn a_built_in_answers_only_for_a_path_no_layer_holds() {
        let root = Dir::default();
        root.insert_asset_text(Path::new("minecraft/in_root.json"), "root");
        root.insert_asset_text(Path::new("mcrs/datapacks/a/minecraft/in_pack.json"), "pack");
        let reader = layered_reader(Box::new(MemoryAssetReader { root }), |path| {
            Some(format!("built-in {path}").into_bytes())
        });

        assert_eq!(read(&*reader, "minecraft/in_root.json").unwrap(), "root");
        assert_eq!(read(&*reader, "minecraft/in_pack.json").unwrap(), "pack");
        assert_eq!(
            read(&*reader, "minecraft/absent.json").unwrap(),
            "built-in minecraft/absent.json"
        );
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
