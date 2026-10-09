use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_worldgen_builtin as builtin;
use std::io::Cursor;

use mcrs_minecraft_nbt::nbt_compress::from_gzip_bytes;
use mcrs_minecraft_worldgen_feature::template::Template;

#[test]
fn a_path_outside_the_built_in_folders_is_not_served() {
    assert_eq!(
        builtin::asset("minecraft/worldgen/structure/igloo.json"),
        None
    );
    assert_eq!(
        builtin::asset("minecraft/worldgen/noise/no_such_noise.json"),
        None
    );
    assert_eq!(
        builtin::asset("minecraft/worldgen/template_pool/empty.json"),
        None
    );
}

#[test]
fn the_typed_folders_are_built_for_a_pack() {
    assert_eq!(builtin::built("vanilla").len(), 5);
    assert_eq!(builtin::built("beta").len(), 3);
    assert!(builtin::built("mcrs").is_empty());
}

fn templates_read_back_from_the_bytes_they_are_served_as(count: usize) {
    let built = builtin::templates();
    assert_eq!(built.len(), 483);
    let listed = builtin::template_paths("minecraft/structure");
    assert_eq!(listed.len(), built.len());
    for (id, template) in built.into_iter().take(count) {
        let path = format!("minecraft/structure/{}.nbt", id.path());
        assert!(listed.contains(&path), "{path} is not listed");
        let bytes = builtin::asset(&path).unwrap_or_else(|| panic!("{path} is not served"));
        let read: Template = from_gzip_bytes(Cursor::new(bytes)).unwrap();
        assert_eq!(read, template, "{path}");
    }
    assert!(builtin::asset("minecraft/structure/igloo/top.nbt").is_none());
}

#[test]
fn a_template_reads_back_from_the_bytes_it_is_served_as() {
    templates_read_back_from_the_bytes_they_are_served_as(5);
}

mod exhaustive {
    #[test]
    fn every_template_reads_back_from_the_bytes_it_is_served_as() {
        super::templates_read_back_from_the_bytes_they_are_served_as(usize::MAX);
    }
}

fn digest(entries: impl IntoIterator<Item = (ResourceLocation, Vec<u8>)>) -> String {
    use sha2::{Digest, Sha256};
    let mut hasher = Sha256::new();
    for (id, bytes) in entries {
        hasher.update(id.as_str());
        hasher.update([0]);
        hasher.update(&bytes);
    }
    hasher
        .finalize()
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// Reading back only proves an entry survives its own encoding. These pin the
/// encoding itself, which matched the game's files entry for entry when the
/// files were removed, so a change to a description shows here.
#[test]
fn the_biomes_template_pools_and_templates_are_the_ones_that_matched_the_game() {
    assert_eq!(
        digest(mcrs_minecraft_worldgen_testing::built_biomes()),
        "268bd82d55f28aaa4fcf5d85f0042d9e0ec173b88edb553619a6b248f5742051"
    );
    assert_eq!(
        digest(mcrs_minecraft_worldgen_testing::built_template_pools()),
        "f13af32a6f2766c69986cd799dcf0ee948023bf3592ca34c1a15d062a7620f2d"
    );
    let templates = builtin::templates().into_iter().map(|(id, template)| {
        let mut nbt = Vec::new();
        mcrs_minecraft_nbt::to_bytes(&template, &mut nbt).unwrap();
        (id, nbt)
    });
    assert_eq!(
        digest(templates),
        "69ff9a400f5a9bea339f86ab4806dfc147a6c0228cca6546eb85d3e83f7ba6ed"
    );
}
