//! Values produced by the vanilla 26.3-snapshot-10 codecs of the kinds that
//! embed item stack templates, through `JsonOps`, `NbtOps`, the stream codec
//! and `HashOps.CRC32C_INSTANCE`.

use std::collections::BTreeMap;

use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::item::{
    BundleContents, ChargedProjectiles, Container, ItemComponentKind, ItemComponentValue,
    PotDecorations, Template, hash_ops,
};
use serde::Deserialize;

use crate::harness::{PersistentValue, TestLookup, from_json, persistent_json};

#[derive(Deserialize)]
struct Golden {
    lookup: BTreeMap<String, BTreeMap<String, u32>>,
    cases: Vec<Case>,
}

#[derive(Deserialize)]
struct Case {
    name: String,
    kind: String,
    json: String,
    nbt: String,
    wire: String,
    hash: i32,
}

fn golden() -> Golden {
    serde_json::from_str(include_str!("../fixtures/item/nested_golden.json")).unwrap()
}

fn lookup(golden: &Golden) -> TestLookup {
    let mut lookup = TestLookup::new();
    for (registry, entries) in &golden.lookup {
        let registry: &'static str = Box::leak(registry.clone().into_boxed_str());
        let entries: Vec<(&str, u32)> = entries.iter().map(|(k, v)| (k.as_str(), *v)).collect();
        lookup.registry_with_ids(registry, &entries);
    }
    lookup
}

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

/// Vanilla writes compounds in hash order, so trees are compared with every
/// compound sorted by key.
fn sorted(tag: NbtTag) -> NbtTag {
    match tag {
        NbtTag::Compound(mut compound) => {
            compound.child_tags = compound
                .child_tags
                .into_iter()
                .map(|(key, value)| (key, sorted(value)))
                .collect();
            compound.child_tags.sort_by(|a, b| a.0.cmp(&b.0));
            NbtTag::Compound(compound)
        }
        NbtTag::List(list) => NbtTag::List(list.into_iter().map(sorted).collect()),
        other => other,
    }
}

fn nbt_tree(bytes: &[u8]) -> NbtTag {
    sorted(mcrs_minecraft_nbt::from_bytes_unnamed(&mut std::io::Cursor::new(bytes)).unwrap())
}

#[test]
fn nested_kinds_match_vanilla_in_every_form() {
    let golden = golden();
    let lookup = lookup(&golden);
    for case in &golden.cases {
        let kind = ItemComponentKind::from_id(&case.kind).unwrap();
        let value = from_json(kind, &case.json);
        assert_eq!(persistent_json(&value), case.json, "{} json", case.name);

        let vanilla_nbt = hex(&case.nbt);
        let mut cursor = std::io::Cursor::new(&vanilla_nbt[..]);
        let mut d = mcrs_minecraft_nbt::deserializer::Deserializer::new(&mut cursor, false);
        let from_vanilla_nbt = ItemComponentValue::deserialize_value(kind, &mut d).unwrap();
        assert_eq!(from_vanilla_nbt, value, "{} from vanilla nbt", case.name);
        let mut nbt = Vec::new();
        mcrs_minecraft_nbt::to_bytes_unnamed(&PersistentValue(&value), &mut nbt).unwrap();
        assert_eq!(
            nbt_tree(&nbt),
            nbt_tree(&vanilla_nbt),
            "{} nbt tree",
            case.name
        );

        let mut wire = Vec::new();
        value.encode_ctx_value(&lookup, &mut wire).unwrap();
        assert_eq!(wire, hex(&case.wire), "{} wire", case.name);
        let mut r = &wire[..];
        let decoded = ItemComponentValue::decode_ctx_value(kind, &lookup, &mut r).unwrap();
        assert!(r.is_empty(), "{} trailing bytes", case.name);
        assert_eq!(decoded, value, "{} wire round trip", case.name);

        assert_eq!(
            hash_ops::hash(&PersistentValue(&value)).unwrap(),
            case.hash,
            "{} hash",
            case.name
        );
    }
}

#[test]
fn a_bare_item_id_reads_as_a_plain_template() {
    let golden = golden();
    let plain = golden
        .cases
        .iter()
        .find(|case| case.name == "use_remainder_plain")
        .unwrap();
    let value = from_json(ItemComponentKind::UseRemainder, r#""minecraft:stone""#);
    assert_eq!(
        value,
        from_json(ItemComponentKind::UseRemainder, &plain.json)
    );
    assert_eq!(persistent_json(&value), plain.json);
}

#[test]
fn a_container_reads_sparse_slots_and_writes_the_dense_wire() {
    let golden = golden();
    let lookup = lookup(&golden);
    let sparse: Container = serde_json::from_str(
        r#"[{"slot":3,"item":"minecraft:apple"},{"slot":0,"item":{"id":"minecraft:stone","count":64}},{"slot":3,"item":{"id":"minecraft:diamond_sword","count":3,"components":{"max_stack_size":16,"damage":7,"custom_name":"named","unbreakable":{},"!repair_cost":{}}}}]"#,
    )
    .unwrap();
    assert_eq!(sparse.0.len(), 4);
    assert!(sparse.0[1].is_none() && sparse.0[2].is_none());
    assert_eq!(
        sparse.0[3].as_ref().map(|t| t.0.item.as_str()),
        Some("minecraft:diamond_sword")
    );
    let case = golden
        .cases
        .iter()
        .find(|case| case.name == "container_sparse")
        .unwrap();
    assert_eq!(persistent_json(&sparse.clone().into()), case.json);

    let mut r = &hex("02010101000000")[..];
    let trailing =
        ItemComponentValue::decode_ctx_value(ItemComponentKind::Container, &lookup, &mut r)
            .unwrap();
    let ItemComponentValue::Container(trailing) = trailing else {
        panic!("not a container");
    };
    assert_eq!(trailing.0.len(), 2);
    assert!(trailing.0[1].is_none());
    assert_eq!(
        serde_json::to_string(&trailing).unwrap(),
        r#"[{"slot":0,"item":{"id":"minecraft:stone"}}]"#
    );
    let mut wire = Vec::new();
    ItemComponentValue::Container(trailing)
        .encode_ctx_value(&lookup, &mut wire)
        .unwrap();
    assert_eq!(wire, hex("02010101000000"));
}

#[test]
fn errors_read_like_vanilla() {
    let slot = serde_json::from_str::<Container>(r#"[{"slot":256,"item":"minecraft:stone"}]"#)
        .unwrap_err();
    assert!(
        slot.to_string()
            .starts_with("Value 256 outside of range [0:255]"),
        "{slot}"
    );
    let air =
        serde_json::from_str::<Container>(r#"[{"slot":1,"item":"minecraft:air"}]"#).unwrap_err();
    assert!(
        air.to_string().contains("Item must not be minecraft:air"),
        "{air}"
    );
    let missing = serde_json::from_str::<Container>(r#"[{"slot":1}]"#).unwrap_err();
    assert!(
        missing.to_string().contains("missing field `item`"),
        "{missing}"
    );

    let entries: Vec<String> = (0..257)
        .map(|i| format!(r#"{{"slot":{},"item":"minecraft:stone"}}"#, i % 256))
        .collect();
    let too_long =
        serde_json::from_str::<Container>(&format!("[{}]", entries.join(","))).unwrap_err();
    assert!(
        too_long
            .to_string()
            .starts_with("List is too long: 257, expected range [0-256]"),
        "{too_long}"
    );

    let arrows = vec![r#""minecraft:arrow""#; 1025].join(",");
    let too_many = serde_json::from_str::<ChargedProjectiles>(&format!("[{arrows}]")).unwrap_err();
    assert!(
        too_many
            .to_string()
            .starts_with("List is too long: 1025, expected range [0-1024]"),
        "{too_many}"
    );

    let zero = serde_json::from_str::<BundleContents>(r#"[{"id":"minecraft:stone","count":0}]"#)
        .unwrap_err();
    assert!(
        zero.to_string()
            .starts_with("Value must be within range [1;99]: 0"),
        "{zero}"
    );
    let unknown =
        serde_json::from_str::<PotDecorations>(r#"{"top":"minecraft:stone"}"#).unwrap_err();
    assert!(
        unknown.to_string().contains("unknown field `top`"),
        "{unknown}"
    );
    let _: Template = serde_json::from_str(r#""minecraft:stone""#).unwrap();
}
