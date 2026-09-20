use std::collections::BTreeMap;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::item::ctx::MAX_NESTING;
use mcrs_minecraft_protocol::item::{
    BundleContents, ChargedProjectiles, Container, DecodeCtx, ItemComponentKind,
    ItemComponentValue, PotDecorations, Template, hash_ops,
};
use mcrs_minecraft_registry::RegistryLookup;
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
    assert_eq!(sparse.slots().len(), 4);
    assert!(sparse.slots()[1].is_none() && sparse.slots()[2].is_none());
    assert_eq!(
        sparse.slots()[3].as_ref().map(|t| t.0.item.as_str()),
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
    assert_eq!(trailing.slots().len(), 2);
    assert!(trailing.slots()[1].is_none());
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
fn bundles_in_bundles_stop_at_the_depth_bound_instead_of_overflowing() {
    let lookup = TestLookup::new();
    let bundle = lookup
        .id("item", &ResourceLocation::minecraft("bundle"))
        .unwrap() as u8;
    let bundle_contents = ItemComponentKind::BundleContents.wire_id() as u8;
    let wrapped = |levels: u32| {
        let mut wire = [bundle, 1, 1, 0, bundle_contents, 1].repeat(levels as usize);
        wire.extend([bundle, 1, 0, 0]);
        wire
    };
    let wire = wrapped(MAX_NESTING - 1);
    Template::decode_ctx(&lookup, &mut &wire[..]).unwrap();
    let error = Template::decode_ctx(&lookup, &mut &wrapped(MAX_NESTING)[..]).unwrap_err();
    assert_eq!(error.to_string(), "value nested deeper than 64 levels");
    let error = Template::decode_ctx(&lookup, &mut &wrapped(10_000)[..]).unwrap_err();
    assert_eq!(error.to_string(), "value nested deeper than 64 levels");
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
    let mut entries = entries;
    entries[5] = r#"{"slot":300,"item":"minecraft:stone"}"#.into();
    let too_long_with_bad_entry =
        serde_json::from_str::<Container>(&format!("[{}]", entries.join(","))).unwrap_err();
    assert!(
        too_long_with_bad_entry
            .to_string()
            .starts_with("List is too long: 257, expected range [0-256]"),
        "{too_long_with_bad_entry}"
    );
    let too_many_slots = Container::new(vec![None; 257]).unwrap_err();
    assert_eq!(
        too_many_slots.to_string(),
        "Got 257 items, but maximum is 256"
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

#[test]
fn wire_templates_are_checked_like_the_vanilla_constructor() {
    let lookup = TestLookup::new();
    let decode = |kind, wire: &str| {
        let mut r = &hex(wire)[..];
        ItemComponentValue::decode_ctx_value(kind, &lookup, &mut r)
    };
    let json = |value: &ItemComponentValue| {
        let mut out = Vec::new();
        value
            .serialize_value(&mut serde_json::Serializer::new(&mut out))
            .map(|()| String::from_utf8(out).unwrap())
            .map_err(|e| e.to_string())
    };

    let air = decode(ItemComponentKind::UseRemainder, "00010000").unwrap_err();
    assert_eq!(air.to_string(), "Item must not be minecraft:air");
    let zero = decode(ItemComponentKind::UseRemainder, "01000000").unwrap_err();
    assert_eq!(zero.to_string(), "Item must be non-empty");

    let hundred = decode(ItemComponentKind::UseRemainder, "01640000").unwrap();
    assert_eq!(
        json(&hundred),
        Err("Value must be within range [1;99]: 100".into())
    );
    let negative = decode(ItemComponentKind::ChargedProjectiles, "0101ffffffff0f0000").unwrap();
    assert_eq!(
        json(&negative),
        Err("Value must be within range [1;99]: -1".into())
    );
    let mut wire = Vec::new();
    negative.encode_ctx_value(&lookup, &mut wire).unwrap();
    assert_eq!(wire, hex("0101ffffffff0f0000"));

    for flag in ["02", "ff"] {
        let back = decode(
            ItemComponentKind::PotDecorations,
            &format!("{flag}01010000000000"),
        )
        .unwrap();
        assert_eq!(json(&back).unwrap(), r#"{"back":{"id":"minecraft:stone"}}"#);
    }
}
