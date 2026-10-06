use std::path::Path;

use mcrs_minecraft_core::{ResourceKey, ResourceLocation};

use crate::saved::region_dir;
use mcrs_minecraft_dimension::Dimension;

#[test]
fn a_dimension_saves_under_its_own_directory() {
    let world = Path::new("world");
    let cases = [
        (
            "minecraft:overworld",
            Some("dimensions/minecraft/overworld/region"),
        ),
        (
            "minecraft:the_nether",
            Some("dimensions/minecraft/the_nether/region"),
        ),
        (
            "minecraft:the_end",
            Some("dimensions/minecraft/the_end/region"),
        ),
        (
            "test:nested/deep",
            Some("dimensions/test/nested/deep/region"),
        ),
        ("test:../outside", None),
        ("test:a//b", None),
    ];
    for (text, expected) in cases {
        let key = ResourceKey::<Dimension>::from_location(ResourceLocation::read(text).unwrap());
        assert_eq!(
            region_dir(world, &key),
            expected.map(|dir| world.join(dir)),
            "{text}"
        );
    }
}
