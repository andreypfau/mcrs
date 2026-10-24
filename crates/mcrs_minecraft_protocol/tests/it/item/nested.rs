use crate::item::harness::{WireCodec, decode_component, wire_kind};
use mcrs_minecraft_fixtures::fixture;

use mcrs_minecraft_item::keys::{DataComponentType, Item};
use mcrs_minecraft_protocol::item::{
    BundleContents, ChargedProjectiles, Container, ItemComponentValue, PotDecorations, Template,
    hash_ops,
};
use mcrs_minecraft_protocol::wire::MAX_NESTING;
use mcrs_minecraft_protocol::{Encode, ErrorKind, VarInt};
use mcrs_minecraft_registries::Named;
use serde::Deserialize;

use crate::item::harness::{
    from_json, from_nbt, hex, json_of, lookup, named, nbt_tree, persistent_json,
};

#[derive(Deserialize)]
struct Golden {
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
    serde_json::from_str(&fixture("item_nested").text()).unwrap()
}

#[test]
fn nested_kinds_match_vanilla_in_every_form() {
    let golden = golden();
    let lookup = lookup();
    for case in &golden.cases {
        let kind = DataComponentType::read(&case.kind).unwrap();
        let value = from_json(kind, &case.json);
        assert_eq!(persistent_json(&value), case.json, "{} json", case.name);

        let vanilla_nbt = hex(&case.nbt);
        let from_vanilla_nbt = from_nbt(kind, &vanilla_nbt);
        assert_eq!(from_vanilla_nbt, value, "{} from vanilla nbt", case.name);
        let mut nbt = Vec::new();
        mcrs_minecraft_nbt::to_bytes_unnamed(&named(&value), &mut nbt).unwrap();
        assert_eq!(
            nbt_tree(&nbt),
            nbt_tree(&vanilla_nbt),
            "{} nbt tree",
            case.name
        );

        let mut wire = Vec::new();
        value.encode_checked(&lookup, &mut wire).unwrap();
        assert_eq!(wire, hex(&case.wire), "{} wire", case.name);
        let mut r = &wire[..];
        let decoded = decode_component(kind, &lookup, &mut r).unwrap();
        assert!(r.is_empty(), "{} trailing bytes", case.name);
        assert_eq!(decoded, value, "{} wire round trip", case.name);

        assert_eq!(
            hash_ops::hash(&named(&value)).unwrap(),
            case.hash,
            "{} hash",
            case.name
        );
    }
}

#[test]
fn a_container_reads_sparse_slots_and_writes_the_dense_wire() {
    let golden = golden();
    let lookup = lookup();
    let sparse: Container<Named> = serde_json::from_str(
        r#"[{"slot":3,"item":"minecraft:apple"},{"slot":0,"item":{"id":"minecraft:stone","count":64}},{"slot":3,"item":{"id":"minecraft:diamond_sword","count":3,"components":{"max_stack_size":16,"damage":7,"custom_name":"named","unbreakable":{},"!repair_cost":{}}}}]"#,
    )
    .unwrap();
    assert_eq!(sparse.slots.0.len(), 4);
    assert!(sparse.slots.0[1].is_none() && sparse.slots.0[2].is_none());
    assert_eq!(
        sparse.slots.0[3]
            .as_ref()
            .map(|t| t.0.item.location().as_str()),
        Some("minecraft:diamond_sword")
    );
    let case = golden
        .cases
        .iter()
        .find(|case| case.name == "container_sparse")
        .unwrap();
    assert_eq!(serde_json::to_string(&sparse).unwrap(), case.json);

    let mut r = &hex("02010101000000")[..];
    let trailing = decode_component(DataComponentType::Container, &lookup, &mut r).unwrap();
    let ItemComponentValue::Container(trailing) = trailing else {
        panic!("not a container");
    };
    assert_eq!(trailing.slots.0.len(), 2);
    assert!(trailing.slots.0[1].is_none());
    assert_eq!(
        json_of(&trailing),
        r#"[{"slot":0,"item":{"id":"minecraft:stone"}}]"#
    );
    let mut wire = Vec::new();
    ItemComponentValue::Container(trailing)
        .encode_checked(&lookup, &mut wire)
        .unwrap();
    assert_eq!(wire, hex("02010101000000"));
}

#[test]
fn bundles_in_bundles_stop_at_the_depth_bound_instead_of_overflowing() {
    let lookup = lookup();
    let mut bundle = Vec::new();
    VarInt(i32::from(Item::Bundle.id().number()))
        .encode(&mut bundle)
        .unwrap();
    let bundle_contents = DataComponentType::BundleContents as u8;
    let wrapped = |levels: u32| {
        let mut wire = Vec::new();
        for _ in 0..levels {
            wire.extend(&bundle);
            wire.extend([1, 1, 0, bundle_contents, 1]);
        }
        wire.extend(&bundle);
        wire.extend([1, 0, 0]);
        wire
    };
    let wire = wrapped(MAX_NESTING - 1);
    Template::decode_checked(&lookup, &mut &wire[..]).unwrap();
    let error = Template::decode_checked(&lookup, &mut &wrapped(MAX_NESTING)[..]).unwrap_err();
    assert!(
        matches!(wire_kind(&error), ErrorKind::TooDeep { max: 64 }),
        "{error:#}"
    );
    let error = Template::decode_checked(&lookup, &mut &wrapped(10_000)[..]).unwrap_err();
    assert!(
        matches!(wire_kind(&error), ErrorKind::TooDeep { max: 64 }),
        "{error:#}"
    );
}

#[test]
fn errors_read_like_vanilla() {
    let slot =
        serde_json::from_str::<Container<Named>>(r#"[{"slot":256,"item":"minecraft:stone"}]"#)
            .unwrap_err();
    assert!(
        slot.to_string()
            .starts_with("Value 256 outside of range [0:255]"),
        "{slot}"
    );
    let air = serde_json::from_str::<Container<Named>>(r#"[{"slot":1,"item":"minecraft:air"}]"#)
        .unwrap_err();
    assert!(
        air.to_string().contains("Item must not be minecraft:air"),
        "{air}"
    );
    let missing = serde_json::from_str::<Container<Named>>(r#"[{"slot":1}]"#).unwrap_err();
    assert!(
        missing.to_string().contains("missing field `item`"),
        "{missing}"
    );

    let entries: Vec<String> = (0..257)
        .map(|i| format!(r#"{{"slot":{},"item":"minecraft:stone"}}"#, i % 256))
        .collect();
    let too_long =
        serde_json::from_str::<Container<Named>>(&format!("[{}]", entries.join(","))).unwrap_err();
    assert!(
        too_long
            .to_string()
            .starts_with("List is too long: 257, expected range [0-256]"),
        "{too_long}"
    );
    let mut entries = entries;
    entries[5] = r#"{"slot":300,"item":"minecraft:stone"}"#.into();
    let too_long_with_bad_entry =
        serde_json::from_str::<Container<Named>>(&format!("[{}]", entries.join(","))).unwrap_err();
    assert!(
        too_long_with_bad_entry
            .to_string()
            .starts_with("List is too long: 257, expected range [0-256]"),
        "{too_long_with_bad_entry}"
    );
    let too_many_slots = Container::<Named>::new(vec![None; 257]).unwrap_err();
    assert_eq!(
        too_many_slots.to_string(),
        "Got 257 items, but maximum is 256"
    );

    let arrows = vec![r#""minecraft:arrow""#; 1025].join(",");
    let too_many =
        serde_json::from_str::<ChargedProjectiles<Named>>(&format!("[{arrows}]")).unwrap_err();
    assert!(
        too_many
            .to_string()
            .starts_with("List is too long: 1025, expected range [0-1024]"),
        "{too_many}"
    );

    let zero =
        serde_json::from_str::<BundleContents<Named>>(r#"[{"id":"minecraft:stone","count":0}]"#)
            .unwrap_err();
    assert!(
        zero.to_string()
            .starts_with("Value must be within range [1;99]: 0"),
        "{zero}"
    );
    let unknown =
        serde_json::from_str::<PotDecorations<Named>>(r#"{"top":"minecraft:stone"}"#).unwrap_err();
    assert!(
        unknown.to_string().contains("unknown field `top`"),
        "{unknown}"
    );
    let _: Template<Named> = serde_json::from_str(r#""minecraft:stone""#).unwrap();
}

#[test]
fn wire_templates_are_checked_like_the_vanilla_constructor() {
    let lookup = lookup();
    let decode = |kind, wire: &str| {
        let mut r = &hex(wire)[..];
        decode_component(kind, &lookup, &mut r)
    };
    let json = |value: &ItemComponentValue| {
        let mut out = Vec::new();
        named(value)
            .serialize_value(&mut serde_json::Serializer::new(&mut out))
            .map(|()| String::from_utf8(out).unwrap())
            .map_err(|e| e.to_string())
    };

    let air = decode(DataComponentType::UseRemainder, "00010000").unwrap_err();
    assert_eq!(
        wire_kind(&air).to_string(),
        "Item must not be minecraft:air"
    );
    let zero = decode(DataComponentType::UseRemainder, "01000000").unwrap_err();
    assert_eq!(wire_kind(&zero).to_string(), "Item must be non-empty");

    let hundred = decode(DataComponentType::UseRemainder, "01640000").unwrap();
    assert_eq!(
        json(&hundred),
        Err("Value must be within range [1;99]: 100".into())
    );
    let negative = decode(DataComponentType::ChargedProjectiles, "0101ffffffff0f0000").unwrap();
    assert_eq!(
        json(&negative),
        Err("Value must be within range [1;99]: -1".into())
    );
    let mut wire = Vec::new();
    negative.encode_checked(&lookup, &mut wire).unwrap();
    assert_eq!(wire, hex("0101ffffffff0f0000"));

    for flag in ["02", "ff"] {
        let back = decode(
            DataComponentType::PotDecorations,
            &format!("{flag}01010000000000"),
        )
        .unwrap();
        assert_eq!(json(&back).unwrap(), r#"{"back":{"id":"minecraft:stone"}}"#);
    }
}

#[test]
fn block_predicates_inside_their_own_matchers_stop_at_the_depth_bound() {
    let can_break = DataComponentType::CanBreak as u8;
    let wrapped = |levels: u32| {
        let mut wire = Vec::new();
        for _ in 0..levels {
            wire.extend([1, 0, 0, 0, 1, can_break]);
        }
        wire.extend([1, 0, 0, 0, 0, 0]);
        wire.extend(std::iter::repeat_n(0, levels as usize));
        wire
    };
    let lookup = lookup();
    decode_component(
        DataComponentType::CanBreak,
        &lookup,
        &mut &wrapped(MAX_NESTING - 1)[..],
    )
    .unwrap();
    for levels in [MAX_NESTING, 10_000] {
        let error = decode_component(
            DataComponentType::CanBreak,
            &lookup,
            &mut &wrapped(levels)[..],
        )
        .unwrap_err();
        assert!(
            matches!(wire_kind(&error), ErrorKind::TooDeep { max: 64 }),
            "{error:#}"
        );
    }
}

#[test]
fn hidden_effects_stop_at_the_depth_bound() {
    use mcrs_minecraft_protocol::Decode;
    use mcrs_minecraft_protocol::item::MobEffectDetails;

    let wrapped = |levels: u32| {
        let mut wire = [0, 0, 0, 0, 0, 1].repeat(levels as usize);
        wire.extend([0, 0, 0, 0, 0, 0]);
        wire
    };
    MobEffectDetails::decode(&mut &wrapped(MAX_NESTING)[..]).unwrap();
    let error = MobEffectDetails::decode(&mut &wrapped(10_000)[..]).unwrap_err();
    assert!(
        matches!(error.kind(), ErrorKind::TooDeep { max: 64 }),
        "{error}"
    );
}
