//! Values produced by the vanilla 26.3-snapshot-10 classes for the flat,
//! text-bearing, NBT-on-the-wire and fuel components: each fixture row is
//! the vanilla JSON, NBT bytes, wire bytes and CRC32C hash of one value.

use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_protocol::item::harness::Sample;
use mcrs_minecraft_protocol::item::{
    BlockState, BrewingFuel, BucketEntityData, Compostable, CookingFuel, CustomData,
    CustomModelData, DebugStickState, Fireworks, ItemComponentValue, ItemDataComponent, ItemModel,
    LodestoneTracker, MapDecorations, NoteBlockSound, Profile, Recipes, ResolvableNumber, SignText,
    SignTextBack, SignTextFront, TooltipDisplay, TooltipStyle, UseEffects, hash_ops,
};

use crate::harness::{PersistentValue, TestLookup, from_json, persistent_json};

type Row = HashMap<&'static str, &'static str>;

static GOLDEN: LazyLock<HashMap<&'static str, Row>> = LazyLock::new(|| {
    let mut rows: HashMap<&str, Row> = HashMap::new();
    for line in include_str!("../fixtures/item/vanilla_records.txt").lines() {
        let (label, rest) = line.split_once(' ').unwrap();
        let (field, value) = rest.split_once(" = ").unwrap();
        rows.entry(label).or_default().insert(field, value);
    }
    rows
});

fn hex(text: &str) -> Vec<u8> {
    (0..text.len())
        .step_by(2)
        .map(|i| u8::from_str_radix(&text[i..i + 2], 16).unwrap())
        .collect()
}

/// Vanilla writes its compounds in hash order, so trees compare sorted.
fn sorted(tag: NbtTag) -> NbtTag {
    match tag {
        NbtTag::Compound(compound) => {
            let mut child_tags: Vec<(String, NbtTag)> = compound
                .child_tags
                .into_iter()
                .map(|(key, value)| (key, sorted(value)))
                .collect();
            child_tags.sort_by(|a, b| a.0.cmp(&b.0));
            NbtTag::Compound(NbtCompound { child_tags })
        }
        NbtTag::List(items) => NbtTag::List(items.into_iter().map(sorted).collect()),
        other => other,
    }
}

fn nbt_tree(bytes: &[u8]) -> NbtTag {
    sorted(mcrs_minecraft_nbt::from_bytes_unnamed(&mut std::io::Cursor::new(bytes)).unwrap())
}

fn our_nbt(value: &ItemComponentValue) -> Vec<u8> {
    let mut out = Vec::new();
    mcrs_minecraft_nbt::to_bytes_unnamed(&PersistentValue(value), &mut out).unwrap();
    out
}

fn from_nbt(value: &ItemComponentValue, bytes: &[u8]) -> ItemComponentValue {
    let mut cursor = std::io::Cursor::new(bytes);
    let mut d = mcrs_minecraft_nbt::deserializer::Deserializer::new(&mut cursor, false);
    let back = ItemComponentValue::deserialize_value(value.kind(), &mut d).unwrap();
    assert_eq!(cursor.position() as usize, bytes.len());
    back
}

fn check(label: &str, value: impl Into<ItemComponentValue>) {
    let value = value.into();
    check_read_as(label, value.clone(), value);
}

/// `read` is what vanilla's own JSON output reads back as, when that is not
/// the value it was written from.
fn check_read_as(label: &str, value: ItemComponentValue, read: ItemComponentValue) {
    let row = &GOLDEN[label];
    let kind = value.kind();

    assert_eq!(persistent_json(&value), row["json"], "{label}: JSON");
    assert_eq!(from_json(kind, row["json"]), read, "{label}: from JSON");

    let vanilla_nbt = hex(row["nbt"]);
    assert_eq!(
        nbt_tree(&our_nbt(&value)),
        nbt_tree(&vanilla_nbt),
        "{label}: NBT"
    );
    assert_eq!(from_nbt(&value, &vanilla_nbt), value, "{label}: from NBT");

    let vanilla_wire = hex(row["wire"]);
    let mut wire = Vec::new();
    value
        .encode_ctx_value(&TestLookup::new(), &mut wire)
        .unwrap();
    if kind.is_nbt_wire() {
        assert_eq!(nbt_tree(&wire), nbt_tree(&vanilla_wire), "{label}: wire");
    } else {
        assert_eq!(wire, vanilla_wire, "{label}: wire");
    }
    let mut r = &vanilla_wire[..];
    let back = ItemComponentValue::decode_ctx_value(kind, &TestLookup::new(), &mut r).unwrap();
    assert!(r.is_empty(), "{label}: trailing wire bytes");
    assert_eq!(back, value, "{label}: from wire");

    assert_eq!(
        hash_ops::hash(&PersistentValue(&value)).unwrap(),
        row["hash"].parse::<i32>().unwrap(),
        "{label}: hash"
    );
}

fn sample<T: Sample>(index: usize) -> T {
    T::samples().swap_remove(index)
}

fn parse<T: ItemDataComponent>(json: &str) -> T {
    T::from_value(&from_json(T::KIND, json)).unwrap().clone()
}

fn error<T: ItemDataComponent>(json: &str) -> String {
    let mut d = serde_json::Deserializer::from_str(json);
    ItemComponentValue::deserialize_value(T::KIND, &mut d)
        .err()
        .unwrap_or_else(|| panic!("{} accepted {json}", T::KIND))
        .to_string()
}

#[test]
fn flat_records_match_vanilla() {
    check("use_effects_default", sample::<UseEffects>(0));
    check("use_effects_full", sample::<UseEffects>(1));
    check("cmd_empty", sample::<CustomModelData>(0));
    check("cmd_full", sample::<CustomModelData>(1));
    check("tooltip_default", sample::<TooltipDisplay>(0));
    check("tooltip_full", sample::<TooltipDisplay>(1));
    check("food_min", sample::<mcrs_minecraft_protocol::item::Food>(0));
    check(
        "food_full",
        sample::<mcrs_minecraft_protocol::item::Food>(1),
    );
    check(
        "cooldown_min",
        sample::<mcrs_minecraft_protocol::item::UseCooldown>(0),
    );
    check(
        "cooldown_full",
        sample::<mcrs_minecraft_protocol::item::UseCooldown>(1),
    );
    check(
        "weapon_default",
        sample::<mcrs_minecraft_protocol::item::Weapon>(0),
    );
    check(
        "weapon_full",
        sample::<mcrs_minecraft_protocol::item::Weapon>(1),
    );
    check(
        "attack_range_default",
        sample::<mcrs_minecraft_protocol::item::AttackRange>(0),
    );
    check(
        "attack_range_full",
        sample::<mcrs_minecraft_protocol::item::AttackRange>(1),
    );
    check("block_state_empty", BlockState::default());
    check(
        "block_state_one",
        BlockState(BTreeMap::from([(
            "facing".to_string(),
            "north".to_string(),
        )])),
    );

    assert_eq!(
        parse::<UseEffects>(
            r#"{"can_sprint":false,"interact_vibrations":true,"speed_multiplier":0.2}"#
        ),
        UseEffects::default()
    );
    assert_eq!(
        parse::<CustomModelData>(
            r#"{"floats":[1.5,-2.0],"flags":[true,false],"strings":["a","b"],"colors":[16711680,[0.0,1.0,0.0]]}"#
        ),
        sample::<CustomModelData>(1)
    );
    assert_eq!(
        parse::<TooltipDisplay>(
            r#"{"hide_tooltip":true,"hidden_components":["minecraft:enchantments","lore","minecraft:enchantments","creative_slot_lock"]}"#
        ),
        sample::<TooltipDisplay>(1)
    );

    assert!(
        error::<UseEffects>(r#"{"speed_multiplier":1.5}"#)
            .starts_with("Value 1.5 outside of range [0.0:1.0]")
    );
    error::<mcrs_minecraft_protocol::item::Food>(r#"{"nutrition":-1,"saturation":0.6}"#);
    assert!(
        error::<mcrs_minecraft_protocol::item::UseCooldown>(r#"{"seconds":0}"#)
            .starts_with("Value must be positive: 0.0")
    );
    assert!(
        error::<mcrs_minecraft_protocol::item::AttackRange>(r#"{"mob_factor":2.5}"#)
            .starts_with("Value 2.5 outside of range [0.0:2.0]")
    );
    assert!(
        error::<mcrs_minecraft_protocol::item::AttackRange>(r#"{"min_reach":65}"#)
            .starts_with("Value must be within range [0.0;64.0]: 65.0")
    );
}

#[test]
fn text_bearing_records_match_vanilla() {
    use mcrs_minecraft_protocol::item::{WritableBookContent, WrittenBookContent};

    check(
        "item_model",
        ItemModel(ResourceLocation::minecraft("stone")),
    );
    check(
        "tooltip_style",
        TooltipStyle(ResourceLocation::new("custom", "style")),
    );
    check(
        "note_block_sound",
        NoteBlockSound(ResourceLocation::minecraft("block.bell.use")),
    );
    assert_eq!(
        parse::<NoteBlockSound>(r#""block.bell.use""#),
        NoteBlockSound(ResourceLocation::minecraft("block.bell.use"))
    );

    check("writable_empty", sample::<WritableBookContent>(0));
    check("writable_full", sample::<WritableBookContent>(1));
    assert_eq!(
        parse::<WritableBookContent>(r#"{"pages":["hello",{"raw":"raw","filtered":"filtered"}]}"#),
        sample::<WritableBookContent>(1)
    );
    check("written_min", sample::<WrittenBookContent>(0));
    check("written_full", sample::<WrittenBookContent>(1));
    assert_eq!(
        parse::<WrittenBookContent>(r#"{"title":"T","author":"me"}"#),
        sample::<WrittenBookContent>(0)
    );
    assert!(
        error::<WrittenBookContent>(r#"{"title":"T","author":"me","generation":4}"#)
            .starts_with("Value must be within range [0;3]: 4")
    );

    check("sign_empty", SignTextFront(SignText::default()));
    check("sign_same_filtered", SignTextFront(sample::<SignText>(1)));
    check("sign_full", SignTextBack(sample::<SignText>(2)));
    assert_eq!(
        parse::<SignTextFront>(
            r#"{"messages":["a","b","c","d"],"filtered_messages":["a","b","c","d"],"color":"black","has_glowing_text":false}"#
        ),
        SignTextFront(sample::<SignText>(1))
    );
    assert_eq!(
        parse::<SignTextFront>(r#"{"messages":["a","b","c","d"],"filtered_messages":["a","b"]}"#),
        SignTextFront(sample::<SignText>(1))
    );
    error::<SignTextFront>(r#"{"messages":["a","b","c"]}"#);
}

#[test]
fn profiles_match_vanilla() {
    check("profile_name", sample::<Profile>(0));
    check("profile_full", sample::<Profile>(1));
    check("profile_id", sample::<Profile>(2));
    check("profile_partial_props", sample::<Profile>(3));
    assert_eq!(parse::<Profile>(r#""Notch""#), sample::<Profile>(0));
    assert_eq!(
        parse::<Profile>(
            r#"{"name":"Steve","properties":{"textures":["v1","v2"]},"model":"wide"}"#
        ),
        sample::<Profile>(3)
    );
    assert!(
        error::<Profile>(r#"{"name":"has space"}"#)
            .contains("Player name contained disallowed characters: 'has space'")
    );
    assert!(
        error::<Profile>(r#""abcdefghijklmnopq""#)
            .contains(r#"String "abcdefghijklmnopq" is too long: 17, expected range [0-16]"#)
    );
}

#[test]
fn lodestone_and_fireworks_match_vanilla() {
    use mcrs_minecraft_protocol::item::FireworkExplosion;

    check("lodestone_default", sample::<LodestoneTracker>(0));
    check("lodestone_full", sample::<LodestoneTracker>(1));
    assert_eq!(
        parse::<LodestoneTracker>(r#"{"tracked":true}"#),
        LodestoneTracker::default()
    );
    error::<LodestoneTracker>(r#"{"target":{"dimension":"minecraft:overworld","pos":[1,2]}}"#);

    check("explosion_min", sample::<FireworkExplosion>(0));
    check("explosion_full", sample::<FireworkExplosion>(1));
    check("fireworks_empty", sample::<Fireworks>(0));
    check("fireworks_full", sample::<Fireworks>(1));
    let wrapped = parse::<Fireworks>(r#"{"flight_duration":300}"#);
    assert_eq!(wrapped.flight_duration(), 44);
    check("fireworks_big", wrapped);
}

#[test]
fn nbt_wire_records_match_vanilla() {
    let mut cod = NbtCompound::new();
    cod.put_byte("Health", 3);
    cod.put_string("id", "minecraft:cod".into());
    check("bucket_data", BucketEntityData(CustomData(cod)));
    let mut health = NbtCompound::new();
    health.put_float("Health", 3.0);
    let snbt = parse::<BucketEntityData>(r#""{Health:3.0f}""#);
    assert_eq!(snbt, BucketEntityData(CustomData(health)));
    let mut whole = NbtCompound::new();
    whole.put_byte("Health", 3);
    check_read_as(
        "bucket_snbt",
        snbt.into(),
        BucketEntityData(CustomData(whole)).into(),
    );

    check("map_dec_empty", sample::<MapDecorations>(0));
    check("map_dec_one", sample::<MapDecorations>(1));
    check("debug_empty", sample::<DebugStickState>(0));
    check("debug_one", sample::<DebugStickState>(1));
    check("recipes_empty", sample::<Recipes>(0));
    check("recipes_two", sample::<Recipes>(1));
    assert_eq!(
        parse::<Recipes>(r#"["minecraft:stone","oak_planks"]"#),
        sample::<Recipes>(1)
    );
    check(
        "loot_min",
        sample::<mcrs_minecraft_protocol::item::ContainerLoot>(0),
    );
    check(
        "loot_full",
        sample::<mcrs_minecraft_protocol::item::ContainerLoot>(1),
    );
    assert_eq!(
        parse::<mcrs_minecraft_protocol::item::ContainerLoot>(
            r#"{"loot_table":"minecraft:chests/simple_dungeon","seed":0}"#
        ),
        sample::<mcrs_minecraft_protocol::item::ContainerLoot>(0)
    );
}

#[test]
fn fuel_matches_vanilla() {
    let reference = |path: &str| ResolvableNumber::reference(ResourceLocation::minecraft(path));
    check("compostable_const", sample::<Compostable>(0));
    check(
        "compostable_ref",
        Compostable {
            layers: reference("some_provider"),
        },
    );
    check(
        "cooking_const",
        CookingFuel {
            burn_time: ResolvableNumber::Constant(200.0),
            speed_multiplier: ResolvableNumber::Constant(1.5),
        },
    );
    check(
        "cooking_ref",
        CookingFuel {
            burn_time: reference("i"),
            speed_multiplier: reference("f"),
        },
    );
    check(
        "brewing_const",
        BrewingFuel {
            uses: ResolvableNumber::Constant(20.0),
            speed_multiplier: ResolvableNumber::Constant(0.5),
        },
    );
    check(
        "brewing_ref",
        BrewingFuel {
            uses: reference("i"),
            speed_multiplier: reference("f"),
        },
    );
    assert_eq!(
        parse::<Compostable>(r#"{"layers":3}"#),
        sample::<Compostable>(0)
    );
}
