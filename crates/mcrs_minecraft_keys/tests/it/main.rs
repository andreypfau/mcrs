use std::path::Path;

use mcrs_minecraft_keys::{biome_tags, block_tags, item_tags};

#[test]
fn generated_tags_name_their_shipped_files() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let rows = [
        (
            block_tags::MINEABLE_PICKAXE.asset_path(),
            "minecraft/tags/block/mineable/pickaxe.json",
        ),
        (
            block_tags::LOGS.asset_path(),
            "minecraft/tags/block/logs.json",
        ),
        (
            item_tags::LOGS.asset_path(),
            "minecraft/tags/item/logs.json",
        ),
        (
            biome_tags::IS_OCEAN.asset_path(),
            "minecraft/tags/worldgen/biome/is_ocean.json",
        ),
    ];
    for (asset_path, expected) in rows {
        assert_eq!(asset_path, expected);
        let file = corpus.join("minecraft").join(
            asset_path
                .strip_prefix("minecraft/")
                .expect("a tag of the minecraft namespace"),
        );
        assert!(file.is_file(), "{} is not shipped", file.display());
    }
}
