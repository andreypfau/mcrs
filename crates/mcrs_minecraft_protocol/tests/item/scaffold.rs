use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::{HolderSet, ResourceKey, ResourceLocation};
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_protocol::item::{
    ComponentMap, ComponentPatch, CreativeSlotLock, CustomData, CustomName, DecodeCtx, EncodeCtx,
    HashedPatchMap, HashedSlot, ItemComponentKind, ItemComponentValue, ItemStackValue, Lore,
    MaxStackSize, RawDelimitedStack, RawStack, Slot, Template, Unbreakable, hash_ops,
};
use mcrs_minecraft_protocol::text::Text;
use mcrs_minecraft_protocol::{Decode, Encode, VarInt};
use mcrs_minecraft_registry::{ItemId, NoRegistries};
use serde::{Deserialize, Serialize};

use crate::harness::TestLookup;

fn stack_size(n: i32) -> MaxStackSize {
    MaxStackSize(Bounded(n))
}

fn custom_data() -> CustomData {
    let mut tag = NbtCompound::new();
    tag.put_int("x", 100000);
    tag.put_string("name", "mcrs".into());
    CustomData(tag)
}

fn patch() -> ComponentPatch {
    let mut patch = ComponentPatch::EMPTY;
    patch.set(stack_size(16));
    patch.set(custom_data());
    patch.set(Lore::new(vec![Text::text("line")]));
    patch.remove(ItemComponentKind::Damage);
    patch
}

#[test]
fn a_patch_reads_and_writes_the_vanilla_map() {
    let json = r#"{"minecraft:max_stack_size":16,"minecraft:custom_data":{"x":100000,"name":"mcrs"},"minecraft:lore":["line"],"!minecraft:damage":{}}"#;
    let patch = patch();
    assert_eq!(serde_json::to_string(&patch).unwrap(), json);
    assert_eq!(serde_json::from_str::<ComponentPatch>(json).unwrap(), patch);

    let bare = serde_json::from_str::<ComponentPatch>(r#"{"max_stack_size":3}"#).unwrap();
    assert_eq!(bare.get::<MaxStackSize>(), Some(&stack_size(3)));

    let snbt =
        serde_json::from_str::<ComponentPatch>(r#"{"custom_data":"{x:100000,name:\"mcrs\"}"}"#)
            .unwrap();
    assert_eq!(snbt.get::<CustomData>(), Some(&custom_data()));

    let unknown = serde_json::from_str::<ComponentPatch>(r#"{"minecraft:nope":1}"#).unwrap_err();
    assert!(
        unknown
            .to_string()
            .contains("No component with type: 'minecraft:nope'"),
        "{unknown}"
    );
    let transient =
        serde_json::from_str::<ComponentPatch>(r#"{"creative_slot_lock":{}}"#).unwrap_err();
    assert!(
        transient
            .to_string()
            .contains("'minecraft:creative_slot_lock' is not a persistent component"),
        "{transient}"
    );
    let duplicate =
        serde_json::from_str::<ComponentPatch>(r#"{"damage":1,"damage":2}"#).unwrap_err();
    assert!(
        duplicate.to_string().contains("Duplicate key"),
        "{duplicate}"
    );
    let removed_last =
        serde_json::from_str::<ComponentPatch>(r#"{"damage":1,"!damage":{}}"#).unwrap();
    assert_eq!(
        serde_json::to_string(&removed_last).unwrap(),
        r#"{"!minecraft:damage":{}}"#
    );
    let set_last = serde_json::from_str::<ComponentPatch>(r#"{"!damage":{},"damage":1}"#).unwrap();
    assert_eq!(
        serde_json::to_string(&set_last).unwrap(),
        r#"{"minecraft:damage":1}"#
    );
    let fraction = serde_json::from_str::<ComponentPatch>(r#"{"damage":1.5}"#).unwrap();
    assert_eq!(
        serde_json::to_string(&fraction).unwrap(),
        r#"{"minecraft:damage":1}"#
    );
    let out_of_range =
        serde_json::from_str::<ComponentPatch>(r#"{"max_stack_size":100}"#).unwrap_err();
    assert!(
        out_of_range
            .to_string()
            .contains("Value must be within range [1;99]: 100"),
        "{out_of_range}"
    );
}

#[test]
fn a_patch_survives_nbt() {
    let patch = patch();
    let mut bytes = Vec::new();
    mcrs_minecraft_nbt::to_bytes_unnamed(&patch, &mut bytes).unwrap();
    assert_eq!(bytes[0], mcrs_minecraft_nbt::COMPOUND_ID);
    let back: ComponentPatch =
        mcrs_minecraft_nbt::from_bytes_unnamed(std::io::Cursor::new(&bytes)).unwrap();
    assert_eq!(back, patch);
}

#[test]
fn transient_kinds_travel_on_the_wire_but_not_in_the_map() {
    let mut patch = ComponentPatch::EMPTY;
    patch.set(CreativeSlotLock);
    patch.set(Unbreakable);
    assert_eq!(
        serde_json::to_string(&patch).unwrap(),
        r#"{"minecraft:unbreakable":{}}"#
    );
    let mut wire = Vec::new();
    patch.encode_ctx(&NoRegistries, &mut wire).unwrap();
    assert_eq!(wire, [2, 0, 20, 4]);
    let mut r = &wire[..];
    assert_eq!(
        ComponentPatch::decode_ctx(&NoRegistries, &mut r).unwrap(),
        patch
    );
}

#[test]
fn the_wire_patch_counts_then_lists() {
    let patch = patch();
    let lookup = TestLookup::new();
    let mut wire = Vec::new();
    patch.encode_ctx(&lookup, &mut wire).unwrap();
    assert_eq!(&wire[..2], [3, 1]);
    assert_eq!(&wire[2..4], [1, 16]);
    assert_eq!(wire[4], 0);
    assert_eq!(wire[5], mcrs_minecraft_nbt::COMPOUND_ID);
    assert_eq!(*wire.last().unwrap(), 3);
    let mut r = &wire[..];
    assert_eq!(ComponentPatch::decode_ctx(&lookup, &mut r).unwrap(), patch);
    assert!(r.is_empty());

    let duplicated = [2, 0, 1, 5, 1, 9];
    let decoded = ComponentPatch::decode_ctx(&NoRegistries, &mut &duplicated[..]).unwrap();
    assert_eq!(decoded.added, vec![stack_size(9).into()]);
}

#[test]
fn a_slot_writes_a_var_int_count_and_an_empty_sentinel() {
    let lookup = TestLookup::new();
    let slot = Slot::new(ItemId(300), 200, patch());
    let mut wire = Vec::new();
    slot.encode_ctx(&lookup, &mut wire).unwrap();
    assert_eq!(&wire[..4], [0xC8, 0x01, 0xAC, 0x02]);
    let raw = RawStack::decode(&mut &wire[..]).unwrap();
    assert_eq!(raw.0, wire);
    assert_eq!(raw.resolve(&lookup).unwrap(), slot);
    assert_eq!(RawStack::from_slot(&slot, &lookup).unwrap(), raw);

    let mut empty = Vec::new();
    Slot::EMPTY.encode_ctx(&lookup, &mut empty).unwrap();
    assert_eq!(empty, [0]);
    assert_eq!(RawStack::EMPTY.0, empty);
    assert_eq!(RawStack::EMPTY.resolve(&NoRegistries).unwrap(), Slot::EMPTY);

    let mut trailing = wire.clone();
    trailing.push(7);
    let mut r = &trailing[..];
    assert_eq!(RawStack::decode(&mut r).unwrap(), raw);
    assert_eq!(r, [7]);
}

#[test]
fn a_delimited_stack_skips_what_a_value_leaves_unread() {
    let lookup = TestLookup::new();
    let slot = Slot::new(ItemId(1), 1, patch());
    let raw = RawDelimitedStack::from_slot(&slot, &lookup).unwrap();
    assert_eq!(raw.resolve(&lookup).unwrap(), slot);
    assert_eq!(RawDelimitedStack::decode(&mut &raw.0[..]).unwrap(), raw);

    let mut padded = vec![1, 1, 1, 0, 1, 3, 16, 0xAA, 0xBB];
    let padded_stack = RawDelimitedStack::decode(&mut &padded[..]).unwrap();
    assert_eq!(padded_stack.0, padded);
    let resolved = padded_stack.resolve(&lookup).unwrap();
    assert_eq!(
        resolved.components.get::<MaxStackSize>(),
        Some(&stack_size(16))
    );
    padded.push(0);
    assert!(RawDelimitedStack::decode(&mut &padded[..]).is_ok());

    let air = RawDelimitedStack(vec![1, 0, 0, 0].into());
    assert_eq!(air.resolve(&lookup).unwrap(), Slot::EMPTY);
    let too_many = RawDelimitedStack(vec![100, 1, 0, 0].into());
    assert!(
        too_many
            .resolve(&lookup)
            .unwrap_err()
            .to_string()
            .contains("[1;99]")
    );

    let with_component =
        |kind: u8, value: u8| RawDelimitedStack(vec![1, 1, 1, 0, kind, 1, value].into());
    for (kind, value, message) in [
        (1, 0, "[1;99]: 0"),
        (1, 100, "[1;99]: 100"),
        (2, 0, "[1;2147483647]: 0"),
    ] {
        let error = with_component(kind, value)
            .resolve(&lookup)
            .unwrap_err()
            .to_string();
        assert!(error.contains(message), "{error}");
    }
    assert!(with_component(2, 5).resolve(&lookup).is_ok());
    assert_eq!(RawDelimitedStack::default(), RawDelimitedStack::EMPTY);
    assert_eq!(RawStack::default(), RawStack::EMPTY);
}

#[test]
fn a_hashed_slot_writes_the_map_then_the_set() {
    let hashed = HashedSlot {
        id: ItemId(1),
        count: 3,
        components: HashedPatchMap {
            added: vec![(ItemComponentKind::MaxStackSize, 0x0102_0304)],
            removed: vec![ItemComponentKind::Damage],
        },
    };
    let mut wire = Vec::new();
    Some(hashed.clone()).encode(&mut wire).unwrap();
    assert_eq!(wire, [1, 1, 3, 1, 1, 1, 2, 3, 4, 1, 3]);
    assert_eq!(
        Option::<HashedSlot>::decode(&mut &wire[..]).unwrap(),
        Some(hashed)
    );

    let mut oversized = vec![1, 3, 0x81, 0x02];
    oversized.extend(std::iter::repeat_n([0, 0, 0, 0, 0], 257).flatten());
    assert!(HashedSlot::decode(&mut &oversized[..]).is_err());
}

#[test]
fn a_hashed_patch_collapses_repeated_kinds_like_a_hash_map() {
    let wire = [2, 1, 0, 0, 0, 1, 1, 0, 0, 0, 2, 3, 3, 3, 3];
    assert_eq!(
        HashedPatchMap::decode(&mut &wire[..]).unwrap(),
        HashedPatchMap {
            added: vec![(ItemComponentKind::MaxStackSize, 2)],
            removed: vec![ItemComponentKind::Damage],
        }
    );
}

#[test]
fn a_hashed_patch_matches_through_hash_ops() {
    let patch = patch();
    let hashed = HashedPatchMap::create(&patch).unwrap();
    assert_eq!(hashed.removed, vec![ItemComponentKind::Damage]);
    let (kind, hash) = hashed.added[0];
    assert_eq!(kind, ItemComponentKind::MaxStackSize);
    assert_eq!(hash, hash_ops::hash(&16i32).unwrap());
    assert!(hashed.matches(&patch));

    let mut other = patch.clone();
    other.set(stack_size(17));
    assert!(!hashed.matches(&other));
    other.set(stack_size(16));
    other.remove(ItemComponentKind::RepairCost);
    assert!(!hashed.matches(&other));
    let mut fewer = patch.clone();
    fewer.added.pop();
    assert!(!hashed.matches(&fewer));

    let mut transient = ComponentPatch::EMPTY;
    transient.set(CreativeSlotLock);
    assert!(HashedPatchMap::create(&transient).is_err());
    assert!(!HashedPatchMap::default().matches(&transient));
    assert_eq!(HashedSlot::create(&Slot::EMPTY).unwrap(), None);
}

fn stone() -> ResourceKey<mcrs_minecraft_protocol::item::ItemReg> {
    ResourceKey::from_location(ResourceLocation::minecraft("stone"))
}

#[test]
fn a_stack_value_always_writes_its_count_and_rejects_air() {
    let value = ItemStackValue {
        item: stone(),
        count: Bounded(1),
        components: ComponentPatch::EMPTY,
    };
    let json = r#"{"id":"minecraft:stone","count":1}"#;
    assert_eq!(serde_json::to_string(&value).unwrap(), json);
    assert_eq!(serde_json::from_str::<ItemStackValue>(json).unwrap(), value);
    assert_eq!(
        serde_json::from_str::<ItemStackValue>(r#"{"id":"minecraft:stone"}"#).unwrap(),
        value
    );
    let air = serde_json::from_str::<ItemStackValue>(r#"{"id":"minecraft:air"}"#).unwrap_err();
    assert_eq!(air.to_string(), "Item must not be minecraft:air");
    assert!(
        serde_json::from_str::<ItemStackValue>(r#"{"id":"minecraft:stone","extra":1}"#).is_err()
    );

    let lookup = TestLookup::new();
    let slot = Slot::from_value(&value, &lookup).unwrap();
    assert_eq!(slot, Slot::new(ItemId(1), 1, ComponentPatch::EMPTY));
    assert_eq!(slot.to_value(&lookup).unwrap(), value);
    assert!(Slot::EMPTY.to_value(&lookup).is_err());
}

#[derive(Serialize, Deserialize, PartialEq, Debug)]
struct Holds {
    #[serde(with = "mcrs_minecraft_protocol::item::stack::optional_stack")]
    item: Option<ItemStackValue>,
}

#[test]
fn an_optional_stack_is_an_empty_map_when_absent() {
    let none = Holds { item: None };
    assert_eq!(serde_json::to_string(&none).unwrap(), r#"{"item":{}}"#);
    assert_eq!(
        serde_json::from_str::<Holds>(r#"{"item":{}}"#).unwrap(),
        none
    );
    let some = Holds {
        item: Some(ItemStackValue {
            item: stone(),
            count: Bounded(2),
            components: ComponentPatch::EMPTY,
        }),
    };
    let json = r#"{"item":{"id":"minecraft:stone","count":2}}"#;
    assert_eq!(serde_json::to_string(&some).unwrap(), json);
    assert_eq!(serde_json::from_str::<Holds>(json).unwrap(), some);
}

#[test]
fn a_template_reads_a_bare_id_and_omits_defaults() {
    let bare = serde_json::from_str::<Template>(r#""minecraft:stone""#).unwrap();
    assert_eq!(
        bare,
        Template::new(stone(), 1, ComponentPatch::EMPTY).unwrap()
    );
    assert_eq!(
        serde_json::to_string(&bare).unwrap(),
        r#"{"id":"minecraft:stone"}"#
    );

    let mut components = ComponentPatch::EMPTY;
    components.set(CustomName(Text::text("named")));
    let full = Template::new(stone(), 3, components).unwrap();
    let json =
        r#"{"id":"minecraft:stone","count":3,"components":{"minecraft:custom_name":"named"}}"#;
    assert_eq!(serde_json::to_string(&full).unwrap(), json);
    assert_eq!(serde_json::from_str::<Template>(json).unwrap(), full);
    assert!(serde_json::from_str::<Template>(r#""minecraft:air""#).is_err());
    assert!(serde_json::from_str::<Template>(r#"{"id":"minecraft:stone","count":0}"#).is_err());

    let lookup = TestLookup::new();
    let mut wire = Vec::new();
    full.encode_ctx(&lookup, &mut wire).unwrap();
    assert_eq!(&wire[..4], [1, 3, 1, 0]);
    assert_eq!(Template::decode_ctx(&lookup, &mut &wire[..]).unwrap(), full);
    assert!(full.encode_ctx(&NoRegistries, &mut Vec::new()).is_err());
}

#[test]
fn a_component_map_applies_and_diffs_against_its_prototype() {
    let prototype = ComponentMap(vec![stack_size(64).into(), Lore::new(Vec::new()).into()]);
    let mut patch = ComponentPatch::EMPTY;
    patch.set(stack_size(64));
    patch.set(custom_data());
    patch.remove(ItemComponentKind::Lore);
    patch.remove(ItemComponentKind::Damage);
    let applied = prototype.apply(&patch);
    assert_eq!(applied.0, vec![stack_size(64).into(), custom_data().into()]);
    let normalised = prototype.diff(&applied);
    assert_eq!(
        normalised.added,
        vec![ItemComponentValue::CustomData(custom_data())]
    );
    assert_eq!(normalised.removed, vec![ItemComponentKind::Lore]);
    assert_eq!(prototype.diff(&prototype), ComponentPatch::EMPTY);

    let json = r#"{"minecraft:max_stack_size":64,"minecraft:lore":[]}"#;
    assert_eq!(serde_json::to_string(&prototype).unwrap(), json);
    assert_eq!(
        serde_json::from_str::<ComponentMap>(json).unwrap(),
        prototype
    );
    assert!(serde_json::from_str::<ComponentMap>(r#"{"!lore":{}}"#).is_err());
}

#[test]
fn a_one_entry_holder_set_writes_as_the_bare_entry() {
    let one: HolderSet<String> = HolderSet::List(vec!["minecraft:stone".into()]);
    assert_eq!(serde_json::to_string(&one).unwrap(), r#""minecraft:stone""#);
    let two: HolderSet<String> = HolderSet::List(vec!["a:b".into(), "c:d".into()]);
    assert_eq!(serde_json::to_string(&two).unwrap(), r#"["a:b","c:d"]"#);
    let always: HolderSet<String, true> = HolderSet::List(vec!["minecraft:stone".into()]);
    assert_eq!(
        serde_json::to_string(&always).unwrap(),
        r#"["minecraft:stone"]"#
    );

    let lookup = TestLookup::new();
    type Blocks = HolderSet<ResourceKey<mcrs_minecraft_protocol::item::BlockReg>>;
    let dirt = ResourceKey::from_location(ResourceLocation::minecraft("dirt"));
    let cases: [(Blocks, &[u8]); 4] = [
        (
            HolderSet::Tag(ResourceLocation::minecraft("logs")),
            &[
                0, 14, b'm', b'i', b'n', b'e', b'c', b'r', b'a', b'f', b't', b':', b'l', b'o',
                b'g', b's',
            ],
        ),
        (HolderSet::One(dirt.clone()), &[2, 1]),
        (
            HolderSet::List(vec![dirt.clone(), dirt.clone()]),
            &[3, 1, 1],
        ),
        (HolderSet::List(vec![]), &[1]),
    ];
    for (set, bytes) in cases {
        let mut wire = Vec::new();
        set.encode_ctx(&lookup, &mut wire).unwrap();
        assert_eq!(wire, bytes, "{set:?}");
        assert_eq!(Blocks::decode_ctx(&lookup, &mut &wire[..]).unwrap(), set);
    }
    let mut wire = Vec::new();
    Blocks::List(vec![dirt.clone()])
        .encode_ctx(&lookup, &mut wire)
        .unwrap();
    assert_eq!(wire, [2, 1]);
    assert_eq!(
        Blocks::decode_ctx(&lookup, &mut &wire[..]).unwrap(),
        HolderSet::One(dirt)
    );
}

#[test]
fn a_kind_is_a_var_int_on_the_wire_and_an_id_in_json() {
    let mut wire = Vec::new();
    ItemComponentKind::CushionColor.encode(&mut wire).unwrap();
    assert_eq!(wire, [121]);
    assert_eq!(
        ItemComponentKind::decode(&mut &wire[..]).unwrap(),
        ItemComponentKind::CushionColor
    );
    let mut unknown = Vec::new();
    VarInt(122).encode(&mut unknown).unwrap();
    let error = ItemComponentKind::decode(&mut &unknown[..]).unwrap_err();
    assert_eq!(error.to_string(), "unknown data component type 122");
    assert_eq!(
        serde_json::to_string(&ItemComponentKind::WolfVariant).unwrap(),
        r#""minecraft:wolf/variant""#
    );
    assert_eq!(
        serde_json::from_str::<ItemComponentKind>(r#""wolf/variant""#).unwrap(),
        ItemComponentKind::WolfVariant
    );
    assert_eq!(
        serde_json::from_str::<ItemComponentKind>(r#""nope""#)
            .unwrap_err()
            .to_string(),
        "No component with type: 'minecraft:nope'"
    );
}

#[test]
fn a_holder_id_below_zero_is_an_error_not_a_panic() {
    use mcrs_minecraft_protocol::item::{Holder, SoundEvent};
    let lookup = TestLookup::new();
    let min_var_int = [0x80, 0x80, 0x80, 0x80, 0x08];
    let error = Holder::<SoundEvent>::decode_ctx(&lookup, &mut &min_var_int[..]).unwrap_err();
    assert!(error.to_string().contains("has no id"), "{error}");
    let too_big = [0xFF, 0xFF, 0xFF, 0xFF, 0x07];
    assert!(Holder::<SoundEvent>::decode_ctx(&lookup, &mut &too_big[..]).is_err());
}

#[test]
fn a_wire_amplifier_clamps_to_a_byte() {
    use mcrs_minecraft_protocol::item::MobEffectDetails;
    let lookup = TestLookup::new();
    for (wire, amplifier) in [
        (vec![0xAC, 0x02, 100, 0, 1, 1, 0], 255u8),
        (vec![0xF9, 0xFF, 0xFF, 0xFF, 0x0F, 100, 0, 1, 1, 0], 0),
        (vec![200, 1, 100, 0, 1, 1, 0], 200),
    ] {
        let mut r = &wire[..];
        let details = MobEffectDetails::decode_ctx(&lookup, &mut r).unwrap();
        assert!(r.is_empty());
        assert_eq!(details.amplifier(), amplifier);
    }
}

#[test]
fn an_air_stack_is_empty_whatever_its_count() {
    let lookup = TestLookup::new();
    let air = Slot::new(ItemId(0), 5, patch());
    assert!(air.is_empty());
    let mut wire = Vec::new();
    air.encode_ctx(&lookup, &mut wire).unwrap();
    assert_eq!(wire, [0]);
    assert_eq!(HashedSlot::create(&air).unwrap(), None);
    let mut r: &[u8] = &[1, 0, 0, 0];
    assert_eq!(Slot::decode_ctx(&lookup, &mut r).unwrap(), Slot::EMPTY);
    assert!(r.is_empty());
}

#[test]
fn a_patch_compares_as_a_map() {
    let mut forward = ComponentPatch::EMPTY;
    forward.set(stack_size(16));
    forward.set(Unbreakable);
    forward.remove(ItemComponentKind::Damage);
    forward.remove(ItemComponentKind::Lore);
    let mut backward = ComponentPatch::EMPTY;
    backward.remove(ItemComponentKind::Lore);
    backward.remove(ItemComponentKind::Damage);
    backward.set(Unbreakable);
    backward.set(stack_size(16));
    assert_eq!(forward, backward);
    backward.set(stack_size(17));
    assert_ne!(forward, backward);
    backward.set(stack_size(16));
    backward.remove(ItemComponentKind::CustomName);
    assert_ne!(forward, backward);
}

#[test]
fn identifiers_read_with_the_default_namespace_everywhere() {
    use mcrs_minecraft_protocol::item::{
        EntityTypeReg, Holder, ResolvableFloat, ResolvableInt, SoundEvent, TypedEntityData,
    };
    let sound: Holder<SoundEvent> = serde_json::from_str(r#""entity.item.break""#).unwrap();
    assert_eq!(
        sound,
        Holder::reference(ResourceLocation::minecraft("entity.item.break"))
    );
    let logs: HolderSet<ResourceKey<EntityTypeReg>> = serde_json::from_str("\"#logs\"").unwrap();
    assert_eq!(logs, HolderSet::Tag(ResourceLocation::minecraft("logs")));
    let zombie: TypedEntityData<EntityTypeReg> =
        serde_json::from_str(r#"{"id":"zombie"}"#).unwrap();
    assert_eq!(zombie.id.location().as_str(), "minecraft:zombie");
    let layers: ResolvableInt = serde_json::from_str(r#""foo""#).unwrap();
    assert_eq!(
        layers,
        ResolvableInt::Reference(ResourceKey::from_location(ResourceLocation::minecraft(
            "foo"
        )))
    );
    assert!(serde_json::from_str::<ResolvableInt>(r#""Foo""#).is_err());

    assert_eq!(
        serde_json::from_str::<ResolvableFloat>("-1").unwrap(),
        ResolvableFloat::Constant(-1.0)
    );
    assert_eq!(
        serde_json::from_str::<ResolvableInt>("5.0").unwrap(),
        ResolvableInt::Constant(5)
    );
    assert_eq!(
        mcrs_minecraft_nbt::from_tag::<ResolvableFloat>(mcrs_minecraft_nbt::tag::NbtTag::Int(-1))
            .unwrap(),
        ResolvableFloat::Constant(-1.0)
    );
}

#[test]
fn color_channels_floor_and_mask_as_argb_does() {
    use mcrs_minecraft_protocol::item::{ArgbInt, RgbInt};
    for (json, expected) in [
        ("[2.0,0.5,-0.5,1.0]", -98432),
        ("[0.0,0.0,1.5,0.5]", 2130706558),
        ("[0.0,2.0,0.0,0.5]", 2130771456),
        ("[0.5,0.5,0.5,0.5]", 2139062143),
        ("4294967295", -1),
        ("1e10", 1410065408),
        ("1.9", 1),
    ] {
        assert_eq!(
            serde_json::from_str::<ArgbInt>(json).unwrap(),
            ArgbInt(expected),
            "{json}"
        );
    }
    assert_eq!(
        serde_json::from_str::<RgbInt>("[1.0,-0.5,0.5]").unwrap(),
        RgbInt(-32641)
    );
    assert_eq!(
        serde_json::from_str::<RgbInt>("4294967295").unwrap(),
        RgbInt(-1)
    );
    assert_eq!(serde_json::from_str::<RgbInt>("1.9").unwrap(), RgbInt(1));
}
