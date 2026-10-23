use crate::asset::read_whole;
use bevy_app::App;
use bevy_asset::AssetApp;
use bevy_asset::io::{
    AssetReader, AssetReaderError, AssetSource, AssetSourceBuilder, AssetSourceId,
    ErasedAssetReader, PathStream, Reader, VecReader,
};
use bevy_ecs::resource::Resource;
use bevy_tasks::futures_lite::StreamExt;
use std::path::{Path, PathBuf};
use std::sync::OnceLock;

/// The directory below the asset root that holds one directory per local pack.
pub const PACKS_ROOT: &str = "mcrs/datapacks";
/// The vanilla pack's directory below a pack root. The game jar's `data/minecraft` tree fills
/// it, never the assets root.
pub const VANILLA_ROOT: &str = "minecraft";

/// `path`, relative to a pack root, below the [`VANILLA_ROOT`] directory, if it lies there.
pub fn below_vanilla_root(path: &Path) -> Option<&Path> {
    path.strip_prefix(VANILLA_ROOT).ok()
}

pub async fn pack_names(reader: &dyn ErasedAssetReader) -> Vec<String> {
    let Ok(mut listing) = reader.read_directory(Path::new(PACKS_ROOT)).await else {
        return Vec::new();
    };
    let mut names = Vec::new();
    while let Some(path) = listing.next().await {
        if is_local_pack(reader, &path).await
            && let Some(name) = path.file_name().and_then(|name| name.to_str())
        {
            names.push(name.to_owned());
        }
    }
    names.sort();
    names
}

// chisle: admits a pack by layout only; one whose pack.mcmeta the registry
// loader rejects as malformed or incompatible still layers here. Taking the
// loaded pack stack as the layer list lifts this.
async fn is_local_pack(reader: &dyn ErasedAssetReader, pack: &Path) -> bool {
    reader.is_directory(pack).await.unwrap_or(false)
        && reader.read(&pack.join("pack.mcmeta")).await.is_ok()
        && !reader
            .is_directory(&pack.join("data"))
            .await
            .unwrap_or(false)
}

/// The directories a data pack root spans: `assets` holds the reports, the definitions and the
/// local packs, and `vanilla` the game jar's `data/minecraft` tree, served as the
/// [`VANILLA_ROOT`] directory of `assets`. Both resolve as the asset root does.
#[derive(Resource, Clone, Debug, PartialEq, Eq)]
pub struct PackRoots {
    pub assets: String,
    pub vanilla: PathBuf,
}

/// `assets` with its [`VANILLA_ROOT`] directory answered by `vanilla`.
pub struct VanillaRoot {
    assets: Box<dyn ErasedAssetReader>,
    vanilla: Box<dyn ErasedAssetReader>,
}

impl VanillaRoot {
    pub fn new(assets: Box<dyn ErasedAssetReader>, vanilla: Box<dyn ErasedAssetReader>) -> Self {
        Self { assets, vanilla }
    }

    fn route<'a>(&'a self, path: &'a Path) -> (&'a dyn ErasedAssetReader, &'a Path) {
        match below_vanilla_root(path) {
            Some(below) => (&*self.vanilla, below),
            None => (&*self.assets, path),
        }
    }
}

impl AssetReader for VanillaRoot {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<Box<dyn Reader + 'a>, AssetReaderError> {
        let (reader, path) = self.route(path);
        reader.read(path).await
    }

    async fn read_meta<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<dyn Reader + 'a>, AssetReaderError> {
        let (reader, path) = self.route(path);
        reader.read_meta(path).await
    }

    async fn read_directory<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<PathStream>, AssetReaderError> {
        match below_vanilla_root(path) {
            Some(below) => {
                let listing = self.vanilla.read_directory(below).await?;
                Ok(Box::new(
                    listing.map(|path| Path::new(VANILLA_ROOT).join(path)),
                ))
            }
            None => self.assets.read_directory(path).await,
        }
    }

    async fn is_directory<'a>(&'a self, path: &'a Path) -> Result<bool, AssetReaderError> {
        let (reader, path) = self.route(path);
        reader.is_directory(path).await
    }
}

/// Every pack's own files under `roots`, unlayered.
pub fn pack_files_reader(
    roots: &PackRoots,
) -> impl FnMut() -> Box<dyn ErasedAssetReader> + Send + Sync + use<> {
    let mut assets = AssetSource::get_default_reader(roots.assets.clone());
    let vanilla = roots.vanilla.clone();
    move || Box::new(VanillaRoot::new(assets(), directory_reader(&vanilla)))
}

#[cfg(not(target_arch = "wasm32"))]
fn directory_reader(path: &Path) -> Box<dyn ErasedAssetReader> {
    Box::new(bevy_asset::io::file::FileAssetReader::new(path))
}

// The browser reads over HTTP, whose paths are text.
#[cfg(target_arch = "wasm32")]
fn directory_reader(path: &Path) -> Box<dyn ErasedAssetReader> {
    AssetSource::get_default_reader(path.to_string_lossy().into_owned())()
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

    async fn read_from_packs(&self, path: &Path) -> Result<Option<Vec<u8>>, AssetReaderError> {
        for pack in self.packs().await.iter().rev() {
            let at = Path::new(PACKS_ROOT).join(pack).join(path);
            match read_whole(&*self.inner, &at).await {
                Ok(bytes) => return Ok(Some(bytes)),
                Err(AssetReaderError::NotFound(_)) => {}
                Err(error) => return Err(error),
            }
        }
        Ok(None)
    }
}

pub type BuiltinAsset = fn(&str) -> Option<Vec<u8>>;

/// The packs layered over `builtin`, layered over `inner`'s own files: a pack overrides a
/// built-in by shipping the same path, and a built-in replaces the file of the vanilla tree.
/// Only structure templates are answered this way; registry entries are built by the registry
/// loader and never reach this reader.
pub fn layered_reader(
    inner: Box<dyn ErasedAssetReader>,
    builtin: BuiltinAsset,
) -> Box<dyn ErasedAssetReader> {
    Box::new(BuiltinLayer {
        layers: PackLayers::new(inner),
        builtin,
    })
}

pub fn layered_file_source(roots: &PackRoots, builtin: BuiltinAsset) -> AssetSourceBuilder {
    let mut files = pack_files_reader(roots);
    AssetSourceBuilder::platform_default(&roots.assets, None)
        .with_reader(move || layered_reader(files(), builtin))
}

// The default source merges the pack layers; the registry loader reads every pack's own files from
// this one instead.
pub const PACK_FILES: &str = "pack_files";

/// Registers the sources over `roots` and keeps `roots` as a resource, so an app built beside
/// this one reads the same trees.
pub fn register_pack_sources(app: &mut App, roots: &PackRoots, builtin: BuiltinAsset) {
    app.register_asset_source(AssetSourceId::Default, layered_file_source(roots, builtin));
    app.register_asset_source(
        PACK_FILES,
        AssetSourceBuilder::platform_default(&roots.assets, None)
            .with_reader(pack_files_reader(roots)),
    );
    app.insert_resource(roots.clone());
}

struct BuiltinLayer {
    layers: PackLayers,
    builtin: BuiltinAsset,
}

impl AssetReader for BuiltinLayer {
    async fn read<'a>(&'a self, path: &'a Path) -> Result<Box<dyn Reader + 'a>, AssetReaderError> {
        if let Some(bytes) = self.layers.read_from_packs(path).await? {
            return Ok(Box::new(VecReader::new(bytes)));
        }
        match path.to_str().and_then(self.builtin) {
            Some(bytes) => Ok(Box::new(VecReader::new(bytes))),
            None => self.layers.inner.read(path).await,
        }
    }

    async fn read_meta<'a>(
        &'a self,
        path: &'a Path,
    ) -> Result<Box<dyn Reader + 'a>, AssetReaderError> {
        AssetReader::read_meta(&self.layers, path).await
    }

    // chisle: a directory listing shows files only. The template reader adds
    // the built-in template paths itself; merging the listings here lifts that.
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
        match self.read_from_packs(path).await? {
            Some(bytes) => Ok(Box::new(VecReader::new(bytes))),
            None => self.inner.read(path).await,
        }
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
    fn a_built_in_replaces_a_vanilla_file_and_yields_to_a_pack() {
        let root = Dir::default();
        root.insert_asset_text(Path::new("minecraft/in_root.json"), "root");
        root.insert_asset_text(Path::new("minecraft/root_only.json"), "root");
        root.insert_asset_text(Path::new("mcrs/datapacks/a/pack.mcmeta"), "{}");
        root.insert_asset_text(Path::new("mcrs/datapacks/a/minecraft/in_pack.json"), "pack");
        let reader = layered_reader(Box::new(MemoryAssetReader { root }), |path| {
            (path != "minecraft/root_only.json").then(|| format!("built-in {path}").into_bytes())
        });

        assert_eq!(
            read(&*reader, "minecraft/in_root.json").unwrap(),
            "built-in minecraft/in_root.json"
        );
        assert_eq!(read(&*reader, "minecraft/in_pack.json").unwrap(), "pack");
        assert_eq!(read(&*reader, "minecraft/root_only.json").unwrap(), "root");
        assert_eq!(
            read(&*reader, "minecraft/absent.json").unwrap(),
            "built-in minecraft/absent.json"
        );
    }

    #[test]
    fn the_vanilla_root_answers_for_its_directory_and_lists_under_it() {
        let (assets, vanilla) = (Dir::default(), Dir::default());
        assets.insert_asset_text(Path::new("mcrs/reports/a.json"), "assets");
        assets.insert_asset_text(Path::new("minecraft/shadowed.json"), "assets");
        vanilla.insert_asset_text(Path::new("shadowed.json"), "vanilla");
        vanilla.insert_asset_text(Path::new("structure/house.nbt"), "vanilla");
        let reader = VanillaRoot::new(
            Box::new(MemoryAssetReader { root: assets }),
            Box::new(MemoryAssetReader { root: vanilla }),
        );

        assert_eq!(read(&reader, "mcrs/reports/a.json").unwrap(), "assets");
        assert_eq!(read(&reader, "minecraft/shadowed.json").unwrap(), "vanilla");
        let listed: Vec<_> = bevy_tasks::block_on(async {
            let listing = AssetReader::read_directory(&reader, Path::new("minecraft/structure"))
                .await
                .unwrap();
            listing.collect().await
        });
        assert_eq!(listed, [Path::new("minecraft/structure/house.nbt")]);
        assert!(
            bevy_tasks::block_on(AssetReader::is_directory(&reader, Path::new("minecraft")))
                .unwrap()
        );
    }

    #[test]
    fn a_name_reads_from_the_last_layer_that_holds_it() {
        let layers = layered(&[
            ("minecraft/root_only.json", "root"),
            ("minecraft/shared.json", "root"),
            ("mcrs/datapacks/a/pack.mcmeta", "{}"),
            ("mcrs/datapacks/a/minecraft/shared_packs.json", "a"),
            ("mcrs/datapacks/b/pack.mcmeta", "{}"),
            ("mcrs/datapacks/b/minecraft/pack_only.json", "b"),
            ("mcrs/datapacks/b/minecraft/shared.json", "b"),
            ("mcrs/datapacks/b/minecraft/shared_packs.json", "b"),
            ("mcrs/datapacks/c_no_metadata/minecraft/shared.json", "c"),
            ("mcrs/datapacks/d_data_directory/pack.mcmeta", "{}"),
            (
                "mcrs/datapacks/d_data_directory/data/minecraft/shared.json",
                "d",
            ),
            ("mcrs/datapacks/d_data_directory/minecraft/shared.json", "d"),
        ]);

        for (path, held_by) in [
            ("minecraft/root_only.json", "root"),
            ("minecraft/pack_only.json", "b"),
            ("minecraft/shared.json", "b"),
            ("minecraft/shared_packs.json", "b"),
        ] {
            assert_eq!(read(&layers, path).unwrap(), held_by, "{path}");
        }

        assert!(matches!(
            read(&layers, "minecraft/absent.json"),
            Err(AssetReaderError::NotFound(_))
        ));
    }
}
