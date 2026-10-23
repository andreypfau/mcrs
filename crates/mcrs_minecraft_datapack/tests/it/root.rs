use mcrs_minecraft_datapack::{DirectoryEntry, PackSource, ReadError, read_vanilla};

struct Echo;

impl PackSource for Echo {
    fn read(&self, path: &str) -> Result<Vec<u8>, ReadError> {
        Ok(path.as_bytes().to_vec())
    }

    fn list(&self, _: &str) -> Result<Vec<DirectoryEntry>, ReadError> {
        Ok(Vec::new())
    }
}

#[test]
fn a_vanilla_read_never_leaves_the_vanilla_pack() {
    let read = |path: &str| read_vanilla(&Echo, path).map(String::from_utf8);
    assert_eq!(
        read("data/minecraft/worldgen/biome/plains.json"),
        Ok(Ok("minecraft/worldgen/biome/plains.json".to_owned()))
    );
    for outside in [
        "data/minecraft/worldgen/biome/../../../../secret.json",
        "data/minecraft/../mcrs/datapacks/beta/pack.mcmeta",
        "data/other/worldgen/biome/plains.json",
    ] {
        assert!(read(outside).is_err(), "{outside}");
    }
}
