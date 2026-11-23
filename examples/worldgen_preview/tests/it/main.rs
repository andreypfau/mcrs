#![expect(clippy::expect_used, reason = "a test reports a failure by panicking")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_fixtures::vanilla_dir;

fn fixed_biome_pack(root: &Path, biome: &str) -> PathBuf {
    let pack = root.join(biome);
    let format = VERSION.pack_version.data_major;
    let metadata =
        format!(r#"{{"pack":{{"description":"t","min_format":{format},"max_format":{format}}}}}"#);
    let overworld = format!(
        r#"{{"type": "minecraft:overworld", "generator": {{
            "type": "minecraft:noise", "settings": "minecraft:overworld",
            "biome_source": {{"type": "minecraft:fixed", "biome": "minecraft:{biome}"}}}}}}"#
    );
    let dimension = pack.join("data/minecraft/dimension/overworld.json");
    fs::create_dir_all(dimension.parent().expect("a parent")).expect("the pack directory");
    fs::write(pack.join("pack.mcmeta"), metadata).expect("the pack metadata");
    fs::write(dimension, overworld).expect("the dimension");
    pack
}

#[test]
fn the_highest_extra_pack_decides_the_biomes_of_the_overworld() {
    let rows: [(&str, &[&str], &str); 2] = [
        ("one", &["desert"], "minecraft:desert"),
        ("two", &["desert", "badlands"], "minecraft:badlands"),
    ];
    for (case, biomes, expected) in rows {
        let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
            .join("worldgen_preview")
            .join(case);
        let _ = fs::remove_dir_all(&root);
        let out = root.join("out");
        let mut command = Command::new(env!("CARGO_BIN_EXE_mcrs_minecraft_worldgen_preview"));
        command
            .arg("--assets")
            .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets"))
            .arg("--vanilla")
            .arg(vanilla_dir());
        for biome in biomes {
            command.arg("--pack").arg(fixed_biome_pack(&root, biome));
        }
        let output = command
            .args(["--seed", "2", "--from", "0,0", "--to", "0,0", "--out"])
            .arg(&out)
            .output()
            .expect("the preview runs");
        let stderr = String::from_utf8_lossy(&output.stderr);
        assert!(output.status.success(), "{case}: {stderr}");
        let stdout = String::from_utf8_lossy(&output.stdout);
        let listed: Vec<&str> = stdout
            .lines()
            .filter_map(|line| line.split_once(' ').map(|(_, name)| name))
            .collect();
        assert_eq!(listed, [expected], "{case}: {stderr}");
        for name in ["height.png", "top_block.png", "biome.png"] {
            assert!(out.join(name).is_file(), "{case}: {name} is written");
        }
    }
}
