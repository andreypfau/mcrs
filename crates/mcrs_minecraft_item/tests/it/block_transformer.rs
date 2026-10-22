use mcrs_minecraft_item::block_transformer::BlockTransformer;
use mcrs_minecraft_registries::Named;
use mcrs_minecraft_worldgen_testing::{json_files, reencode, vanilla_dir};

#[test]
fn corpus_round_trips() {
    let files = json_files(&vanilla_dir().join("block_transformer"));
    for path in &files {
        let bytes = std::fs::read(path).unwrap();
        let raw: serde_json::Value = serde_json::from_slice(&bytes).unwrap();
        let parsed: BlockTransformer<Named> =
            serde_json::from_slice(&bytes).unwrap_or_else(|e| panic!("{}: {e}", path.display()));
        assert_eq!(reencode(&parsed), raw, "{}", path.display());
    }
    assert_eq!(files.len(), 3);
}

#[test]
fn an_empty_transformer_is_refused_with_the_list_bounds() {
    let error = serde_json::from_str::<BlockTransformer<Named>>("[]")
        .unwrap_err()
        .to_string();
    assert!(
        error.contains("List is too short: 0, expected range [1-200]"),
        "{error}"
    );
}
