#![expect(clippy::expect_used, reason = "a test reports a failure by panicking")]

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};
use std::process::Command;

use mcrs_minecraft_anvil::RegionFile;
use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_fixtures::vanilla_dir;
use serde::Deserialize;

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

#[derive(Deserialize)]
struct Climate {
    x0: i32,
    z0: i32,
    step: i32,
    nx: usize,
    nz: usize,
    signals: BTreeMap<String, Vec<f64>>,
}

fn run_area(case: &str, extra: &[&str]) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("worldgen_preview")
        .join(case);
    let _ = fs::remove_dir_all(&root);
    let output = Command::new(env!("CARGO_BIN_EXE_mcrs_minecraft_worldgen_preview"))
        .arg("--assets")
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets"))
        .arg("--vanilla")
        .arg(vanilla_dir())
        .args(["--seed", "2", "--from", "0,0", "--to", "0,0", "--out"])
        .arg(root.join("out"))
        .args(extra)
        .output()
        .expect("the preview runs");
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert!(output.status.success(), "{case}: {stderr}");
    root
}

#[test]
fn the_world_flag_writes_the_area_as_a_region_file() {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("worldgen_preview")
        .join("world");
    let world = root.join("save");
    run_area("world", &["--world", world.to_str().expect("a utf-8 path")]);
    let region = RegionFile::open(world.join("region/r.0.0.mca")).expect("the region file");
    assert_eq!(region.present().count(), 1);
}

#[test]
fn the_climate_flag_writes_every_signal_on_the_grid() {
    let file = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("worldgen_preview")
        .join("climate_run")
        .join("climate.json");
    let arguments = [
        "--climate",
        file.to_str().expect("a utf-8 path"),
        "--climate-step",
        "8",
    ];
    run_area("climate_run", &arguments);
    let climate: Climate =
        serde_json::from_slice(&fs::read(&file).expect("the climate file")).expect("JSON");
    assert_eq!((climate.x0, climate.z0), (0, 0));
    assert_eq!((climate.nx, climate.nz, climate.step), (2, 2, 8));
    assert_eq!(climate.signals.len(), 6);
    for (name, values) in &climate.signals {
        assert_eq!(values.len(), climate.nx * climate.nz, "{name}");
    }
}
