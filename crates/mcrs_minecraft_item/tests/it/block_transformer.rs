use mcrs_minecraft_item::block_transformer::BlockTransformer;
use mcrs_minecraft_worldgen_testing::corpus_set;

#[test]
fn corpus_round_trips() {
    let dir = std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../assets/minecraft/block_transformer");
    let mut count = 0;
    for entry in std::fs::read_dir(dir).unwrap() {
        let path = entry.unwrap().path();
        let text = std::fs::read_to_string(&path).unwrap();
        corpus_set().scope(|| {
            let parsed: BlockTransformer = serde_json::from_str(&text)
                .unwrap_or_else(|e| panic!("{}: {e}", path.display()));
            let original: serde_json::Value = serde_json::from_str(&text).unwrap();
            assert_eq!(
                serde_json::to_value(&parsed).unwrap(),
                original,
                "{}",
                path.display()
            );
        });
        count += 1;
    }
    assert_eq!(count, 3);
}
