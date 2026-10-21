use mcrs_minecraft_registries::{KeyOr, KeySet};

use crate::support::{Damage, Item};

#[test]
fn a_reference_is_written_in_the_normal_form() {
    let sets = [
        (r#""stone""#, r#""minecraft:stone""#),
        (r##""#logs""##, r##""#minecraft:logs""##),
        (r#"["t:a"]"#, r#""t:a""#),
        ("[]", "[]"),
        (r#"["a","t:b"]"#, r#"["minecraft:a","t:b"]"#),
    ];
    for (file, written) in sets {
        let set: KeySet<Item> =
            serde_json::from_str(file).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert_eq!(serde_json::to_string(&set).unwrap(), written, "{file}");
    }

    let refs = [
        (r#""a""#, r#""minecraft:a""#),
        (
            r#"{"message_id":"a","exhaustion":0.5}"#,
            r#"{"message_id":"a","exhaustion":0.5}"#,
        ),
    ];
    for (file, written) in refs {
        let value: KeyOr<Item, Damage> =
            serde_json::from_str(file).unwrap_or_else(|e| panic!("{file}: {e}"));
        assert_eq!(serde_json::to_string(&value).unwrap(), written, "{file}");
    }
}

#[test]
fn a_string_that_is_not_a_key_is_refused_rather_than_read_another_way() {
    assert!(serde_json::from_str::<KeySet<Item>>(r##"["#logs"]"##).is_err());
    assert!(serde_json::from_str::<KeyOr<Item, Damage>>(r#""Not A Key""#).is_err());
}
