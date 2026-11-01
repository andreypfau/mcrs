use bevy::asset::io::AssetSourceBuilder;
use mcrs_minecraft_assets::packs::{PackLayers, PackRoots, pack_files_reader};

/// The default asset source over `roots`: the packs under the assets root layered over its
/// files and the game's data pack, with no built-in answer for a path none of them holds.
pub fn asset_source(roots: &PackRoots) -> AssetSourceBuilder {
    let mut files = pack_files_reader(roots);
    AssetSourceBuilder::platform_default(&roots.assets, None)
        .with_reader(move || Box::new(PackLayers::new(files())))
}
