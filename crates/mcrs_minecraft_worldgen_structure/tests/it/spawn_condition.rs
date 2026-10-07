use mcrs_minecraft_worldgen_structure::spawn_condition::SpawnSelector;
use mcrs_minecraft_worldgen_testing::corpus_set;

#[test]
fn selectors_write_back_as_read() {
    let cases = [
        r#"{"priority":0}"#,
        r##"{"condition":{"type":"minecraft:structure","structures":"#minecraft:cats_spawn_as_black"},"priority":1}"##,
    ];
    corpus_set().scope(|| {
        for json in cases {
            let selector: SpawnSelector = serde_json::from_str(json).unwrap();
            assert_eq!(serde_json::to_string(&selector).unwrap(), json);
        }
    });
}
