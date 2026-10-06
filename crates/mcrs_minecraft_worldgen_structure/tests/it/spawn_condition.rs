use mcrs_minecraft_worldgen_structure::spawn_condition::{DoubleBounds, SpawnSelector};
use mcrs_minecraft_worldgen_testing::corpus_set;
use serde::Serialize;

type Case = (fn(&str) -> String, &'static str);

#[test]
fn bounds_and_selectors_write_back_as_read() {
    fn codec<T: serde::de::DeserializeOwned + Serialize>(json: &str) -> String {
        serde_json::to_string(&serde_json::from_str::<T>(json).unwrap()).unwrap()
    }
    let cases: &[Case] = &[
        (codec::<DoubleBounds>, "0.9"),
        (codec::<DoubleBounds>, r#"{"min":0.9}"#),
        (codec::<DoubleBounds>, r#"{"min":0.1,"max":0.5}"#),
        (codec::<DoubleBounds>, "{}"),
        (codec::<SpawnSelector>, r#"{"priority":0}"#),
        (
            codec::<SpawnSelector>,
            r##"{"condition":{"type":"minecraft:structure","structures":"#minecraft:cats_spawn_as_black"},"priority":1}"##,
        ),
    ];
    corpus_set().scope(|| {
        for (codec, json) in cases {
            assert_eq!(codec(json), *json);
        }
    });
    assert!(serde_json::from_str::<DoubleBounds>(r#"{"min":2,"max":1}"#).is_err());
}
