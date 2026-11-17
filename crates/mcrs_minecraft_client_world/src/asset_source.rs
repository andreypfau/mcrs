use bevy::asset::io::AssetSourceBuilder;
use mcrs_minecraft_assets::packs::{PackLayers, PackRoots, pack_files_reader};

pub fn asset_source(roots: &PackRoots) -> AssetSourceBuilder {
    let mut files = pack_files_reader(roots);
    AssetSourceBuilder::platform_default(&roots.assets, None)
        .with_reader(move || Box::new(PackLayers::new(files())))
}
