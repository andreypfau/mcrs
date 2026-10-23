use mcrs_minecraft_block::keys::Block;
use mcrs_minecraft_core::Id;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_core::{ResourceKey, rl};
use mcrs_minecraft_item::keys::{DataComponentType, Item};
use mcrs_minecraft_protocol::item::ctx::WireCtx;
use mcrs_minecraft_protocol::item::{
    ComponentMap, ComponentPatch, CreativeSlotLock, CustomData, CustomName, DecodeCtx, EncodeCtx,
    HashedPatchMap, HashedStack, ItemComponentValue, ItemStackValue, Lore, MaxStackSize,
    ProtoStack, RawDelimitedStack, RawStack, Template, Text, Unbreakable, hash_ops,
};
use mcrs_minecraft_protocol::{Decode, Encode, VarInt};
use mcrs_minecraft_registries::{IdSet, Named, Ref, Resolved};

use crate::item::harness::{
    ctx, custom_data, entry, json_of, list_set, named, one_set, read_json, registries, resolved,
    tag_set,
};

fn stack_size(n: i32) -> MaxStackSize {
    MaxStackSize(Bounded(n))
}

fn patch() -> ComponentPatch {
    let mut patch = ComponentPatch::EMPTY;
    patch.set(stack_size(16));
    patch.set(custom_data());
    patch.set(Lore::new(vec![Text::text("line")]));
    patch.remove(DataComponentType::Damage);
    patch
}

#[test]
fn a_patch_reads_and_writes_the_vanilla_map() {
    let json = r#"{"minecraft:max_stack_size":16,"minecraft:custom_data":{"x":100000,"name":"mcrs"},"minecraft:lore":["line"],"!minecraft:damage":{}}"#;
    let patch = patch();
    assert_eq!(json_of(&patch), json);
    assert_eq!(read_json::<ComponentPatch>(json).unwrap(), patch);

    let bare = read_json::<ComponentPatch>(r#"{"max_stack_size":3}"#).unwrap();
    assert_eq!(bare.get::<MaxStackSize>(), Some(&stack_size(3)));

    let snbt =
        read_json::<ComponentPatch>(r#"{"custom_data":"{x:100000,name:\"mcrs\"}"}"#).unwrap();
    assert_eq!(snbt.get::<CustomData>(), Some(&custom_data()));

    let unknown = read_json::<ComponentPatch>(r#"{"minecraft:nope":1}"#).unwrap_err();
    assert!(
        unknown
            .to_string()
            .contains("No component with type: 'minecraft:nope'"),
        "{unknown}"
    );
    let transient = read_json::<ComponentPatch>(r#"{"creative_slot_lock":{}}"#).unwrap_err();
    assert!(
        transient
            .to_string()
            .contains("'minecraft:creative_slot_lock' is not a persistent component"),
        "{transient}"
    );
    let duplicate = read_json::<ComponentPatch>(r#"{"damage":1,"damage":2}"#).unwrap_err();
    assert!(
        duplicate.to_string().contains("Duplicate key"),
        "{duplicate}"
    );
    let removed_last = read_json::<ComponentPatch>(r#"{"damage":1,"!damage":{}}"#).unwrap();
    assert_eq!(json_of(&removed_last), r#"{"!minecraft:damage":{}}"#);
    let set_last = read_json::<ComponentPatch>(r#"{"!damage":{},"damage":1}"#).unwrap();
    assert_eq!(json_of(&set_last), r#"{"minecraft:damage":1}"#);
    let fraction = read_json::<ComponentPatch>(r#"{"damage":1.5}"#).unwrap();
    assert_eq!(json_of(&fraction), r#"{"minecraft:damage":1}"#);
    let out_of_range = read_json::<ComponentPatch>(r#"{"max_stack_size":100}"#).unwrap_err();
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
    mcrs_minecraft_nbt::to_bytes_unnamed(&named(&patch), &mut bytes).unwrap();
    assert_eq!(bytes[0], mcrs_minecraft_nbt::COMPOUND_ID);
    let back: ComponentPatch<Named> =
        mcrs_minecraft_nbt::from_bytes_unnamed(std::io::Cursor::new(&bytes)).unwrap();
    assert_eq!(resolved::<ComponentPatch>(back).unwrap(), patch);
}

#[test]
fn transient_kinds_travel_on_the_wire_but_not_in_the_map() {
    let mut patch = ComponentPatch::EMPTY;
    patch.set(CreativeSlotLock);
    patch.set(Unbreakable);
    assert_eq!(json_of(&patch), r#"{"minecraft:unbreakable":{}}"#);
    let mut wire = Vec::new();
    patch.encode_ctx(&WireCtx::NONE, &mut wire).unwrap();
    assert_eq!(wire, [2, 0, 20, 4]);
    let mut r = &wire[..];
    assert_eq!(
        ComponentPatch::decode_ctx(&WireCtx::NONE, &mut r).unwrap(),
        patch
    );
}

#[test]
fn the_wire_patch_counts_then_lists() {
    let patch = patch();
    let lookup = ctx();
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
    let decoded = ComponentPatch::decode_ctx(&WireCtx::NONE, &mut &duplicated[..]).unwrap();
    assert_eq!(decoded.added, vec![stack_size(9).into()]);
}

#[test]
fn a_slot_writes_a_var_int_count_and_an_empty_sentinel() {
    let lookup = ctx();
    let slot = ProtoStack::new(Item::OpenEyeblossom.id(), 200, patch());
    let mut wire = Vec::new();
    slot.encode_ctx(&lookup, &mut wire).unwrap();
    assert_eq!(&wire[..4], [0xC8, 0x01, 0xAC, 0x02]);
    let raw = RawStack::decode(&mut &wire[..]).unwrap();
    assert_eq!(raw.0, wire);
    assert_eq!(raw.resolve(&lookup).unwrap(), slot);
    assert_eq!(RawStack::from_stack(&slot, &lookup).unwrap(), raw);

    let mut empty = Vec::new();
    ProtoStack::EMPTY.encode_ctx(&lookup, &mut empty).unwrap();
    assert_eq!(empty, [0]);
    assert_eq!(RawStack::EMPTY.0, empty);
    assert_eq!(
        RawStack::EMPTY.resolve(&WireCtx::NONE).unwrap(),
        ProtoStack::EMPTY
    );

    let mut trailing = wire.clone();
    trailing.push(7);
    let mut r = &trailing[..];
    assert_eq!(RawStack::decode(&mut r).unwrap(), raw);
    assert_eq!(r, [7]);
}

#[test]
fn a_delimited_stack_skips_what_a_value_leaves_unread() {
    let lookup = ctx();
    let slot = ProtoStack::new(Item::Stone.id(), 1, patch());
    let raw = RawDelimitedStack::from_stack(&slot, &lookup).unwrap();
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
    assert_eq!(air.resolve(&lookup).unwrap(), ProtoStack::EMPTY);
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
        (2, 0, "Value must be positive: 0"),
    ] {
        let error = with_component(kind, value)
            .resolve(&lookup)
            .unwrap_err()
            .to_string();
        assert!(error.contains(message), "{error}");
    }
    assert!(with_component(2, 5).resolve(&lookup).is_ok());
}

#[test]
fn a_hashed_slot_writes_the_map_then_the_set() {
    let hashed = HashedStack {
        id: Item::Stone.id(),
        count: 3,
        components: HashedPatchMap {
            added: vec![(DataComponentType::MaxStackSize, 0x0102_0304)],
            removed: vec![DataComponentType::Damage],
        },
    };
    let mut wire = Vec::new();
    Some(hashed.clone()).encode(&mut wire).unwrap();
    assert_eq!(wire, [1, 1, 3, 1, 1, 1, 2, 3, 4, 1, 3]);
    assert_eq!(
        Option::<HashedStack>::decode(&mut &wire[..]).unwrap(),
        Some(hashed)
    );

    let mut oversized = vec![1, 3, 0x81, 0x02];
    oversized.extend(std::iter::repeat_n([0, 0, 0, 0, 0], 257).flatten());
    assert!(HashedStack::decode(&mut &oversized[..]).is_err());
}

#[test]
fn a_hashed_patch_collapses_repeated_kinds_like_a_hash_map() {
    let wire = [2, 1, 0, 0, 0, 1, 1, 0, 0, 0, 2, 3, 3, 3, 3];
    assert_eq!(
        HashedPatchMap::decode(&mut &wire[..]).unwrap(),
        HashedPatchMap {
            added: vec![(DataComponentType::MaxStackSize, 2)],
            removed: vec![DataComponentType::Damage],
        }
    );
}

#[test]
fn a_hashed_patch_matches_through_hash_ops() {
    let patch = patch();
    let names = registries();
    let hashed = HashedPatchMap::create(&patch, names).unwrap();
    assert_eq!(hashed.removed, vec![DataComponentType::Damage]);
    let (kind, hash) = hashed.added[0];
    assert_eq!(kind, DataComponentType::MaxStackSize);
    assert_eq!(hash, hash_ops::hash(&16i32).unwrap());
    assert!(hashed.matches(&patch, names));

    let mut other = patch.clone();
    other.set(stack_size(17));
    assert!(!hashed.matches(&other, names));
    other.set(stack_size(16));
    other.remove(DataComponentType::RepairCost);
    assert!(!hashed.matches(&other, names));
    let mut fewer = patch.clone();
    fewer.added.pop();
    assert!(!hashed.matches(&fewer, names));

    let mut transient = ComponentPatch::EMPTY;
    transient.set(CreativeSlotLock);
    assert!(HashedPatchMap::create(&transient, names).is_err());
    assert!(!HashedPatchMap::default().matches(&transient, names));
    assert_eq!(
        HashedStack::create(&ProtoStack::EMPTY, &ctx()).unwrap(),
        None
    );
}

fn stone() -> ResourceKey<Item> {
    ResourceKey::from_location(rl!("minecraft:stone").to_arc())
}

#[test]
fn a_stack_value_always_writes_its_count_and_rejects_air() {
    let value = ItemStackValue::<Named> {
        item: stone(),
        count: Bounded(1),
        components: ComponentPatch::EMPTY,
    };
    let json = r#"{"id":"minecraft:stone","count":1}"#;
    assert_eq!(serde_json::to_string(&value).unwrap(), json);
    assert_eq!(
        serde_json::from_str::<ItemStackValue<Named>>(json).unwrap(),
        value
    );
    assert_eq!(
        serde_json::from_str::<ItemStackValue<Named>>(r#"{"id":"minecraft:stone"}"#).unwrap(),
        value
    );
    let air =
        serde_json::from_str::<ItemStackValue<Named>>(r#"{"id":"minecraft:air"}"#).unwrap_err();
    assert_eq!(air.to_string(), "Item must not be minecraft:air");
    assert!(
        serde_json::from_str::<ItemStackValue<Named>>(r#"{"id":"minecraft:stone","extra":1}"#)
            .is_err()
    );

    let value: ItemStackValue = resolved(value).unwrap();
    let slot = ProtoStack::from_value(&value);
    assert_eq!(
        slot,
        ProtoStack::new(Item::Stone.id(), 1, ComponentPatch::EMPTY)
    );
    assert_eq!(slot.to_value().unwrap(), value);
    assert!(ProtoStack::EMPTY.to_value().is_err());
}

#[test]
fn a_template_reads_a_bare_id_and_omits_defaults() {
    let bare = serde_json::from_str::<Template<Named>>(r#""minecraft:stone""#).unwrap();
    assert_eq!(
        bare,
        Template::<Named>::new(stone(), 1, ComponentPatch::EMPTY).unwrap()
    );
    assert_eq!(
        serde_json::to_string(&bare).unwrap(),
        r#"{"id":"minecraft:stone"}"#
    );

    let mut components = ComponentPatch::EMPTY;
    components.set(CustomName(Text::text("named")));
    let full = Template::new(Item::Stone.id(), 3, components).unwrap();
    let json =
        r#"{"id":"minecraft:stone","count":3,"components":{"minecraft:custom_name":"named"}}"#;
    assert_eq!(json_of(&full), json);
    assert_eq!(read_json::<Template>(json).unwrap(), full);
    assert!(serde_json::from_str::<Template<Named>>(r#""minecraft:air""#).is_err());
    assert!(
        serde_json::from_str::<Template<Named>>(r#"{"id":"minecraft:stone","count":0}"#).is_err()
    );

    let lookup = ctx();
    let mut wire = Vec::new();
    full.encode_ctx(&lookup, &mut wire).unwrap();
    assert_eq!(&wire[..4], [1, 3, 1, 0]);
    assert_eq!(Template::decode_ctx(&lookup, &mut &wire[..]).unwrap(), full);
    assert!(full.encode_ctx(&WireCtx::NONE, &mut Vec::new()).is_err());
}

#[test]
fn a_component_map_applies_and_diffs_against_its_prototype() {
    let prototype = ComponentMap(vec![stack_size(64).into(), Lore::new(Vec::new()).into()]);
    let mut patch = ComponentPatch::EMPTY;
    patch.set(stack_size(64));
    patch.set(custom_data());
    patch.remove(DataComponentType::Lore);
    patch.remove(DataComponentType::Damage);
    let applied = prototype.apply(&patch);
    assert_eq!(applied.0, vec![stack_size(64).into(), custom_data().into()]);
    let normalised = prototype.diff(&applied);
    assert_eq!(
        normalised.added,
        vec![ItemComponentValue::CustomData(custom_data())]
    );
    assert_eq!(normalised.removed, vec![DataComponentType::Lore]);
    assert_eq!(prototype.diff(&prototype), ComponentPatch::EMPTY);

    let json = r#"{"minecraft:max_stack_size":64,"minecraft:lore":[]}"#;
    assert_eq!(json_of(&prototype), json);
    assert_eq!(read_json::<ComponentMap>(json).unwrap(), prototype);
    assert!(read_json::<ComponentMap>(r#"{"!lore":{}}"#).is_err());
}

#[test]
fn a_one_entry_holder_set_is_the_bare_entry() {
    let one: IdSet<Block> = list_set(&["stone"]);
    assert_eq!(one, one_set::<Block>("stone"));
    assert_eq!(json_of(&one), r#""minecraft:stone""#);
    let two: IdSet<Block> = list_set(&["stone", "dirt"]);
    assert_eq!(json_of(&two), r#"["minecraft:stone","minecraft:dirt"]"#);

    let lookup = ctx();
    type Blocks = IdSet<Block>;
    let dirt: Id<Block> = entry("dirt");
    let d = u8::try_from(dirt.number()).unwrap();
    let cases: [(Blocks, &[u8]); 4] = [
        (
            tag_set("logs"),
            &[
                0, 14, b'm', b'i', b'n', b'e', b'c', b'r', b'a', b'f', b't', b':', b'l', b'o',
                b'g', b's',
            ],
        ),
        (IdSet::List(Box::new([dirt])), &[2, d]),
        (IdSet::List(Box::new([dirt, dirt])), &[3, d, d]),
        (IdSet::List(Box::new([])), &[1]),
    ];
    for (set, bytes) in cases {
        let mut wire = Vec::new();
        set.encode_ctx(&lookup, &mut wire).unwrap();
        assert_eq!(wire, bytes, "{set:?}");
        assert_eq!(Blocks::decode_ctx(&lookup, &mut &wire[..]).unwrap(), set);
    }
}

#[test]
fn a_direct_holder_set_encodes_from_a_registry_without_tags() {
    use mcrs_minecraft_sound::SoundEvent;
    use mcrs_minecraft_sound::keys::sound_event::ENTRIES;

    assert!(
        registries()
            .tags::<SoundEvent>()
            .is_none_or(|tags| tags.iter().next().is_none())
    );
    let lookup = ctx();
    let id = |n: usize| {
        let name = ENTRIES[n].as_static_str();
        entry::<SoundEvent>(name.strip_prefix("minecraft:").unwrap_or(name))
    };
    let (first, fourth) = (id(1), id(4));
    for (set, bytes) in [
        (IdSet::List(Box::new([first])), &[2, 1][..]),
        (IdSet::List(Box::new([fourth, first])), &[3, 4, 1][..]),
    ] {
        let mut wire = Vec::new();
        set.encode_ctx(&lookup, &mut wire).unwrap();
        assert_eq!(wire, bytes, "{set:?}");
    }
}

#[test]
fn a_holder_set_naming_what_the_receiver_lacks_fails_the_packet() {
    let lookup = ctx();
    let mut unknown_tag = vec![0, 13];
    unknown_tag.extend(b"minecraft:zzz");
    let mut unassigned = vec![2];
    let blocks = registries().entries::<Block>().unwrap().len();
    VarInt(i32::try_from(blocks).unwrap())
        .encode(&mut unassigned)
        .unwrap();
    for (what, wire) in [
        ("a tag the receiver never built", unknown_tag),
        ("a number the receiver never assigned", unassigned),
    ] {
        assert!(
            IdSet::<Block>::decode_ctx(&lookup, &mut &wire[..]).is_err(),
            "{what}"
        );
    }
    let stone = u8::try_from(entry::<Block>("stone").number()).unwrap();
    assert_eq!(
        IdSet::<Block>::decode_ctx(&lookup, &mut &[2u8, stone][..]).unwrap(),
        one_set("stone")
    );
}

#[test]
fn a_holder_set_decoded_without_registries_fails_the_packet() {
    assert!(IdSet::<Block>::decode_ctx(&WireCtx::NONE, &mut &[2u8, 1][..]).is_err());
}

#[test]
fn a_kind_is_a_var_int_on_the_wire() {
    let mut wire = Vec::new();
    DataComponentType::CushionColor.encode(&mut wire).unwrap();
    assert_eq!(wire, [121]);
    assert_eq!(
        DataComponentType::decode(&mut &wire[..]).unwrap(),
        DataComponentType::CushionColor
    );
    let mut unknown = Vec::new();
    VarInt(122).encode(&mut unknown).unwrap();
    let error = DataComponentType::decode(&mut &unknown[..]).unwrap_err();
    assert_eq!(error.to_string(), "unknown data component type 122");
}

#[test]
fn a_holder_id_below_zero_is_an_error_not_a_panic() {
    use mcrs_minecraft_sound::SoundEvent;
    type Sound = Ref<Resolved, SoundEvent, SoundEvent>;
    let lookup = ctx();
    let min_var_int = [0x80, 0x80, 0x80, 0x80, 0x08];
    let error = Sound::decode_ctx(&lookup, &mut &min_var_int[..]).unwrap_err();
    assert!(error.to_string().contains("is outside"), "{error}");
    let too_big = [0xFF, 0xFF, 0xFF, 0xFF, 0x07];
    assert!(Sound::decode_ctx(&lookup, &mut &too_big[..]).is_err());
}

#[test]
fn a_wire_amplifier_clamps_to_a_byte() {
    use mcrs_minecraft_protocol::item::MobEffectDetails;
    let lookup = ctx();
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
    let lookup = ctx();
    let air = ProtoStack::new(Item::Air.id(), 5, patch());
    assert!(air.is_empty());
    let mut wire = Vec::new();
    air.encode_ctx(&lookup, &mut wire).unwrap();
    assert_eq!(wire, [0]);
    assert_eq!(HashedStack::create(&air, &lookup).unwrap(), None);
    let mut r: &[u8] = &[1, 0, 0, 0];
    assert_eq!(
        ProtoStack::decode_ctx(&lookup, &mut r).unwrap(),
        ProtoStack::EMPTY
    );
    assert!(r.is_empty());
}

#[test]
fn a_patch_compares_as_a_map() {
    let mut forward = ComponentPatch::EMPTY;
    forward.set(stack_size(16));
    forward.set(Unbreakable);
    forward.remove(DataComponentType::Damage);
    forward.remove(DataComponentType::Lore);
    let mut backward = ComponentPatch::EMPTY;
    backward.remove(DataComponentType::Lore);
    backward.remove(DataComponentType::Damage);
    backward.set(Unbreakable);
    backward.set(stack_size(16));
    assert_eq!(forward, backward);
    backward.set(stack_size(17));
    assert_ne!(forward, backward);
    backward.set(stack_size(16));
    backward.remove(DataComponentType::CustomName);
    assert_ne!(forward, backward);
}

#[test]
fn identifiers_read_with_the_default_namespace_everywhere() {
    use mcrs_minecraft_entity::keys::EntityType;
    use mcrs_minecraft_protocol::item::{ResolvableFloat, ResolvableInt, TypedEntityData};
    use mcrs_minecraft_sound::SoundEvent;
    let sound: Ref<Resolved, SoundEvent, SoundEvent> = read_json(r#""entity.item.break""#).unwrap();
    assert_eq!(
        sound,
        Ref::Id(mcrs_minecraft_sound::keys::sound_event::ENTITY_ITEM_BREAK.id())
    );
    let logs: IdSet<Block> = read_json("\"#logs\"").unwrap();
    assert_eq!(logs, tag_set("logs"));
    let zombie: TypedEntityData<EntityType> = serde_json::from_str(r#"{"id":"zombie"}"#).unwrap();
    assert_eq!(zombie.id.location().as_str(), "minecraft:zombie");
    let layers: ResolvableInt = serde_json::from_str(r#""foo""#).unwrap();
    assert_eq!(
        layers,
        ResolvableInt::Reference(ResourceKey::from_location(rl!("minecraft:foo").to_arc()))
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
