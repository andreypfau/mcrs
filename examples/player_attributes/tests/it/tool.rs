use std::env;
use std::fs;
use std::path::Path;
use std::path::PathBuf;
use std::process;
use std::process::Command;
use std::process::Output;

use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::from_tag;
use mcrs_minecraft_nbt::nbt_compress::to_gzip_bytes_vec;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_save::PlayerDat;
use mcrs_minecraft_save::parse_player_dat;

fn player_file(name: &str) -> PathBuf {
    let mut modifier = NbtCompound::new();
    modifier.put("id", NbtTag::String("example:bonus".to_owned()));
    modifier.put("amount", NbtTag::Double(0.5));
    modifier.put(
        "operation",
        NbtTag::String("add_multiplied_base".to_owned()),
    );
    let mut max_health = NbtCompound::new();
    max_health.put("id", NbtTag::String("minecraft:max_health".to_owned()));
    max_health.put("base", NbtTag::Double(24.0));
    max_health.put("modifiers", NbtTag::List(vec![NbtTag::Compound(modifier)]));
    let mut follow_range = NbtCompound::new();
    follow_range.put("id", NbtTag::String("minecraft:follow_range".to_owned()));
    follow_range.put("base", NbtTag::Double(16.0));
    let mut dat = PlayerDat::default();
    dat.root.put(
        "attributes",
        NbtTag::List(vec![
            NbtTag::Compound(max_health),
            NbtTag::Compound(follow_range),
        ]),
    );
    dat.root.put("XpLevel", NbtTag::Int(7));
    let path = env::temp_dir().join(format!(
        "mcrs-player-attributes-{name}-{}.dat",
        process::id()
    ));
    fs::write(&path, to_gzip_bytes_vec(&dat).unwrap()).unwrap();
    path
}

fn run(path: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mcrs_minecraft_player_attributes"))
        .arg(path)
        .args(args)
        .output()
        .unwrap()
}

fn row<'a>(stdout: &'a str, attribute: &str) -> Vec<&'a str> {
    let line = stdout
        .lines()
        .find(|line| line.starts_with(attribute))
        .unwrap_or_else(|| panic!("no row for {attribute} in\n{stdout}"));
    line.split_whitespace().collect()
}

#[test]
fn prints_the_folded_value_then_sets_a_base_and_keeps_the_other_keys() {
    let path = player_file("edit");

    let listed = run(&path, &[]);
    let stdout = String::from_utf8(listed.stdout).unwrap();
    assert!(listed.status.success(), "{stdout}");
    assert_eq!(
        row(&stdout, "minecraft:max_health"),
        ["minecraft:max_health", "24*", "36"],
        "24 + 24 × 0.5 by ATR-VAL-001"
    );
    assert!(stdout.contains("\n  modifier example:bonus add_multiplied_base 0.5\n"));
    assert_eq!(
        row(&stdout, "minecraft:attack_damage"),
        ["minecraft:attack_damage", "1", "1"]
    );

    let edited = run(&path, &["--set", "attack_damage=3"]);
    let stdout = String::from_utf8(edited.stdout).unwrap();
    assert!(edited.status.success(), "{stdout}");
    let stderr = String::from_utf8(edited.stderr).unwrap();
    assert!(
        stderr.contains("1 of 2") && stderr.contains("minecraft:follow_range"),
        "the rewrite drops the compound outside the player set: {stderr}"
    );
    assert_eq!(
        row(&stdout, "minecraft:attack_damage"),
        ["minecraft:attack_damage", "3*", "3"]
    );

    let dat = parse_player_dat(&fs::read(&path).unwrap(), &path).unwrap();
    fs::remove_file(&path).unwrap();
    assert_eq!(dat.get("XpLevel"), Some(&NbtTag::Int(7)));
    let attributes: Vec<NbtCompound> = from_tag(dat.get("attributes").unwrap().clone()).unwrap();
    let base = |id: &str| {
        attributes
            .iter()
            .find(|compound| compound.get_string("id") == Some(id))
            .and_then(|compound| compound.get("base"))
            .cloned()
    };
    assert_eq!(base("minecraft:attack_damage"), Some(NbtTag::Double(3.0)));
    assert_eq!(base("minecraft:max_health"), Some(NbtTag::Double(24.0)));
}

#[test]
fn rejects_a_bad_edit_without_a_panic_and_leaves_the_file() {
    let path = player_file("reject");
    let before = fs::read(&path).unwrap();
    for args in [
        ["--set", "minecraft:no_such_attribute=1"],
        ["--set", "max_health=many"],
        ["--set", "max_health=NaN"],
        ["--set", "follow_range=16"],
        ["--sett", "max_health=1"],
    ] {
        let output = run(&path, &args);
        assert_eq!(
            output.status.code(),
            Some(1),
            "{args:?}: an error, not a panic"
        );
        assert!(!output.stderr.is_empty(), "{args:?}");
    }
    assert_eq!(fs::read(&path).unwrap(), before);
    let missing = run(&path.with_extension("missing"), &[]);
    assert_eq!(missing.status.code(), Some(1), "a missing file");
    fs::remove_file(&path).unwrap();
}

#[test]
fn a_base_equal_to_the_current_one_leaves_the_file() {
    let path = player_file("same");
    let before = fs::read(&path).unwrap();
    let output = run(&path, &["--set", "max_health=24"]);
    let after = fs::read(&path).unwrap();
    fs::remove_file(&path).unwrap();
    assert!(output.status.success());
    assert_eq!(
        String::from_utf8(output.stdout).unwrap(),
        "no change; the file is not written\n"
    );
    assert_eq!(after, before);
}
