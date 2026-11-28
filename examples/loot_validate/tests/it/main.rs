#![expect(clippy::expect_used, reason = "a test reports a failure by panicking")]

use std::fs;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use mcrs_minecraft_core::VERSION;
use mcrs_minecraft_fixtures::vanilla_dir;

const VALID_TABLE: &str = r#"{"type": "minecraft:chest", "pools": [{"rolls": 1, "entries": [
    {"type": "minecraft:item", "name": "minecraft:stick"}]}]}"#;

fn pack(name: &str, table: &str) -> PathBuf {
    let root = PathBuf::from(env!("CARGO_TARGET_TMPDIR"))
        .join("loot_validate")
        .join(name);
    let _ = fs::remove_dir_all(&root);
    let format = VERSION.pack_version.data_major;
    let metadata =
        format!(r#"{{"pack":{{"description":"t","min_format":{format},"max_format":{format}}}}}"#);
    let table_path = root.join("data/t/loot_table/chest.json");
    fs::create_dir_all(table_path.parent().expect("a parent")).expect("the pack directory");
    fs::write(root.join("pack.mcmeta"), metadata).expect("the pack metadata");
    fs::write(table_path, table).expect("the table");
    root
}

fn validate(name: &str, table: &str) -> Output {
    run(&pack(name, table))
}

fn run(root: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_mcrs_minecraft_loot_validate"))
        .arg(root)
        .arg(vanilla_dir())
        .arg(Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets"))
        .output()
        .expect("the validator runs")
}

/// LOOT-CTX-013: a part that reads a parameter outside the type of its table is reported with
/// its table and path, and fails the run with status 1; a pack without such a part passes.
#[test]
fn loot_ctx_013_a_chest_table_that_reads_the_tool_fails_and_a_valid_table_passes() {
    let reads_tool = validate(
        "reads_tool",
        r#"{"type": "minecraft:chest", "pools": [{"rolls": 1, "entries": [
            {"type": "minecraft:item", "name": "minecraft:stick",
             "condition": {"type": "minecraft:match_tool"}}]}]}"#,
    );
    let stderr = String::from_utf8_lossy(&reads_tool.stderr);
    assert_eq!(reads_tool.status.code(), Some(1), "an issue fails the run");
    assert_eq!(
        String::from_utf8_lossy(&reads_tool.stdout),
        "t:chest at pools[0].entries[0].condition: \
         the parameter tool is not allowed in the key set minecraft:chest\n",
        "{stderr}"
    );

    let valid = validate("valid", VALID_TABLE);
    let stderr = String::from_utf8_lossy(&valid.stderr);
    assert!(valid.status.success(), "{stderr}");
    assert_eq!(String::from_utf8_lossy(&valid.stdout), "", "{stderr}");
}

#[test]
fn a_pack_without_metadata_does_not_run_and_states_the_pack() {
    let root = pack("without_metadata", VALID_TABLE);
    fs::remove_file(root.join("pack.mcmeta")).expect("the pack metadata");
    let output = run(&root);
    let stderr = String::from_utf8_lossy(&output.stderr);
    assert_eq!(output.status.code(), Some(2), "{stderr}");
    assert!(
        stderr.contains("data pack file/without_metadata is not available"),
        "{stderr}"
    );
}

#[test]
fn a_file_name_that_is_not_a_resource_name_is_printed_and_fails_the_run() {
    let root = pack("upper_case_file", VALID_TABLE);
    let tables = root.join("data/t/loot_table");
    fs::rename(tables.join("chest.json"), tables.join("Chest.json")).expect("the rename");
    let output = run(&root);
    let stderr = String::from_utf8_lossy(&output.stderr);
    let stdout = String::from_utf8_lossy(&output.stdout);
    assert_eq!(output.status.code(), Some(1), "{stderr}");
    assert!(
        stdout.contains("Chest.json in namespace t of data pack file/upper_case_file ignored"),
        "{stdout}{stderr}"
    );
}
