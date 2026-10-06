use std::path::Path;

use mcrs_minecraft_core::{RegistryKey, TagKey};
use mcrs_minecraft_keys::{self as keys, biome_tags, block_tags, item_tags};

fn tag_file<R>(registry: RegistryKey<R>, tag: TagKey<R, &'static str>) -> String {
    let location = tag.resource_location();
    format!(
        "{}/tags/{}/{}.json",
        location.namespace(),
        registry.path(),
        location.path()
    )
}

#[test]
fn generated_tags_name_their_shipped_files() {
    let corpus = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets");
    let rows = [
        (
            tag_file(keys::BLOCK, block_tags::MINEABLE_PICKAXE),
            "minecraft/tags/block/mineable/pickaxe.json",
        ),
        (
            tag_file(keys::BLOCK, block_tags::LOGS),
            "minecraft/tags/block/logs.json",
        ),
        (
            tag_file(keys::ITEM, item_tags::LOGS),
            "minecraft/tags/item/logs.json",
        ),
        (
            tag_file(keys::BIOME, biome_tags::IS_OCEAN),
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
