use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::{ResourceKey, ResourceLocation};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_protocol::item::{
    ComponentPatch, CreativeSlotLock, CustomData, CustomName, Damage, DecodeCtx, EncodeCtx,
    HashedPatchMap, ItemComponentKind, ItemComponentValue, Lore, MaxStackSize, Template,
    Unbreakable, decode_delimited_patch, encode_delimited_patch, hash_ops,
};
use mcrs_minecraft_protocol::text::Text;
use mcrs_minecraft_protocol::{Decode, Encode};

use crate::harness::TestLookup;

const PATCH_WIRE: &str = "07020110000a03000178000186a00800046e616d6500046d637273000b010800046c696e65060800056e616d656403070414130d";
const PATCH_DELIMITED_WIRE: &str = "070201011000170a03000178000186a00800046e616d6500046d637273000b08010800046c696e6506080800056e616d656403010704001400130d";
const HASHED_WIRE: &str = "060be39a13ec04c574b4c8016971cc99039915c56e06d2ee6d5e0044739cf202130d";
const TEMPLATE_WIRE: &str = "9a080306020110000a03000178000186a00800046e616d6500046d637273000b010800046c696e65060800056e616d6564030704130d";
const PARSED_PATCH_WIRE: &str = "02010110000a03000178000186a00800046e616d6500046d6372730003";

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

fn custom_data() -> CustomData {
    let mut tag = NbtCompound::new();
    tag.put_int("x", 100000);
    tag.put_string("name", "mcrs".into());
    CustomData(tag)
}

fn patch(with_transient: bool) -> ComponentPatch {
    let mut patch = ComponentPatch::EMPTY;
    patch.set(MaxStackSize(Bounded(16)));
    patch.set(custom_data());
    patch.set(Lore::new(vec![Text::text("line")]));
    patch.set(CustomName(Text::text("named")));
    patch.set(Damage(Bounded(7)));
    patch.set(Unbreakable);
    if with_transient {
        patch.set(CreativeSlotLock);
    }
    patch.remove(ItemComponentKind::RepairCost);
    patch.remove(ItemComponentKind::Enchantments);
    patch
}

fn wire(value: &impl EncodeCtx) -> Vec<u8> {
    let mut out = Vec::new();
    value.encode_ctx(&TestLookup::new(), &mut out).unwrap();
    out
}

#[test]
fn a_patch_matches_the_vanilla_stream_codec() {
    let patch = patch(true);
    let bytes = hex(PATCH_WIRE);
    assert_eq!(wire(&patch), bytes);
    let mut r = &bytes[..];
    assert_eq!(
        ComponentPatch::decode_ctx(&TestLookup::new(), &mut r).unwrap(),
        patch
    );
    assert!(r.is_empty());

    let delimited = hex(PATCH_DELIMITED_WIRE);
    let mut out = Vec::new();
    encode_delimited_patch(&patch, &TestLookup::new(), &mut out).unwrap();
    assert_eq!(out, delimited);
    let mut r = &delimited[..];
    assert_eq!(
        decode_delimited_patch(&TestLookup::new(), &mut r).unwrap(),
        patch
    );
    assert!(r.is_empty());
}

#[test]
fn a_parsed_patch_matches_vanilla_in_json_and_on_the_wire() {
    let parsed: ComponentPatch = serde_json::from_str(
        r#"{"max_stack_size":16,"custom_data":"{x:100000,name:\"mcrs\"}","!minecraft:damage":{}}"#,
    )
    .unwrap();
    assert_eq!(
        serde_json::to_string(&parsed).unwrap(),
        r#"{"minecraft:max_stack_size":16,"minecraft:custom_data":{"x":100000,"name":"mcrs"},"!minecraft:damage":{}}"#
    );
    assert_eq!(wire(&parsed), hex(PARSED_PATCH_WIRE));
}

fn hash(value: impl Into<ItemComponentValue>) -> i32 {
    let value = value.into();
    hash_ops::hash(&crate::harness::PersistentValue(&value)).unwrap()
}

#[test]
fn text_free_kinds_hash_like_vanilla() {
    assert_eq!(hash(MaxStackSize(Bounded(16))), 1769065625);
    assert_eq!(hash(custom_data()), 1148427506);
    assert_eq!(hash(Damage(Bounded(7))), -1726626450);
    assert_eq!(hash(Unbreakable), -982207288);
}

#[test]
fn text_kinds_hash_like_vanilla_and_the_hashed_map_matches_its_wire() {
    assert_eq!(hash(Lore::new(vec![Text::text("line")])), -476441620);
    assert_eq!(hash(CustomName(Text::text("named"))), -756126370);
    let hashed = HashedPatchMap::create(&patch(false)).unwrap();
    let vanilla = HashedPatchMap::decode(&mut &hex(HASHED_WIRE)[..]).unwrap();
    assert_eq!(sorted(&hashed), sorted(&vanilla));
    assert!(vanilla.matches(&patch(false)));
    assert_eq!(
        serde_json::to_string(&Template::new(diamond_sword(), 3, patch(false)).unwrap()).unwrap(),
        r#"{"id":"minecraft:diamond_sword","count":3,"components":{"minecraft:max_stack_size":16,"minecraft:custom_data":{"x":100000,"name":"mcrs"},"minecraft:lore":["line"],"minecraft:custom_name":"named","minecraft:damage":7,"minecraft:unbreakable":{},"!minecraft:repair_cost":{},"!minecraft:enchantments":{}}}"#
    );
}

fn diamond_sword() -> ResourceKey<mcrs_minecraft_protocol::item::ItemReg> {
    ResourceKey::from_location(ResourceLocation::minecraft("diamond_sword"))
}

/// Vanilla emits its hash maps in hash-iteration order, so the entries are
/// compared as sets.
fn sorted(map: &HashedPatchMap) -> (Vec<(ItemComponentKind, i32)>, Vec<ItemComponentKind>) {
    let mut added = map.added.clone();
    added.sort();
    let mut removed = map.removed.clone();
    removed.sort();
    (added, removed)
}

#[test]
fn the_hashed_map_wire_is_the_vanilla_layout() {
    let hashed = HashedPatchMap::decode(&mut &hex(HASHED_WIRE)[..]).unwrap();
    let expected = HashedPatchMap {
        added: vec![
            (ItemComponentKind::MaxStackSize, 1769065625),
            (ItemComponentKind::CustomData, 1148427506),
            (ItemComponentKind::Lore, -476441620),
            (ItemComponentKind::CustomName, -756126370),
            (ItemComponentKind::Damage, -1726626450),
            (ItemComponentKind::Unbreakable, -982207288),
        ],
        removed: vec![
            ItemComponentKind::RepairCost,
            ItemComponentKind::Enchantments,
        ],
    };
    assert_eq!(sorted(&hashed), sorted(&expected));
    let mut out = Vec::new();
    hashed.encode(&mut out).unwrap();
    assert_eq!(out, hex(HASHED_WIRE));
}

#[test]
fn a_template_matches_the_vanilla_stream_codec_and_json() {
    let mut lookup_items = TestLookup::new();
    lookup_items.registry_with_ids("item", &[("diamond_sword", 1050)]);
    let template = Template::new(diamond_sword(), 3, patch(false)).unwrap();
    let mut out = Vec::new();
    template.encode_ctx(&lookup_items, &mut out).unwrap();
    assert_eq!(out, hex(TEMPLATE_WIRE));
    assert_eq!(
        Template::decode_ctx(&lookup_items, &mut &out[..]).unwrap(),
        template
    );

    let plain: Template = serde_json::from_str(r#""minecraft:stone""#).unwrap();
    assert_eq!(
        serde_json::to_string(&plain).unwrap(),
        r#"{"id":"minecraft:stone"}"#
    );
    assert_eq!(plain.0.count.0, 1);
    assert!(plain.0.components.is_empty());
}

#[test]
fn errors_read_like_vanilla() {
    let transient =
        serde_json::from_str::<ComponentPatch>(r#"{"creative_slot_lock":{}}"#).unwrap_err();
    assert!(
        transient
            .to_string()
            .starts_with("'minecraft:creative_slot_lock' is not a persistent component"),
        "{transient}"
    );
    let unknown = serde_json::from_str::<ComponentPatch>(r#"{"nope":{}}"#).unwrap_err();
    assert!(
        unknown
            .to_string()
            .starts_with("No component with type: 'minecraft:nope'"),
        "{unknown}"
    );
    let range = serde_json::from_str::<ComponentPatch>(r#"{"max_stack_size":100}"#).unwrap_err();
    assert!(
        range
            .to_string()
            .starts_with("Value must be within range [1;99]: 100"),
        "{range}"
    );
}

/// `show_icon` is always written.
#[test]
fn a_mob_effect_instance_matches_vanilla_with_show_icon_resolved() {
    use mcrs_minecraft_protocol::item::MobEffectInstance;

    for (input, json, snbt, expected_hash) in [
        (
            r#"{"id":"minecraft:speed","amplifier":1,"duration":100}"#,
            r#"{"id":"minecraft:speed","amplifier":1,"duration":100,"show_icon":true}"#,
            "{amplifier:1b,duration:100,id:\"minecraft:speed\",show_icon:1b}",
            1513404545,
        ),
        (
            r#"{"id":"minecraft:speed","duration":100}"#,
            r#"{"id":"minecraft:speed","duration":100,"show_icon":true}"#,
            "{duration:100,id:\"minecraft:speed\",show_icon:1b}",
            112340837,
        ),
    ] {
        let effect: MobEffectInstance = serde_json::from_str(input).unwrap();
        assert_eq!(serde_json::to_string(&effect).unwrap(), json);
        let explicit: MobEffectInstance = serde_json::from_str(json).unwrap();
        assert_eq!(effect, explicit);
        let mut nbt = mcrs_minecraft_nbt::to_nbt_compound(&effect).unwrap();
        nbt.child_tags.sort_by(|a, b| a.0.cmp(&b.0));
        assert_eq!(nbt.to_string(), snbt);
        assert_eq!(hash_ops::hash(&effect).unwrap(), expected_hash);
    }
}
