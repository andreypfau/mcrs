use mcrs_minecraft_protocol::item::EncodeCtx;
use mcrs_minecraft_protocol::item::decode_component_value;
use std::collections::{BTreeMap, HashMap};
use std::sync::LazyLock;

use mcrs_minecraft_core::ResourceLocation;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_protocol::item::harness::Sample;
use mcrs_minecraft_protocol::item::{
    AttackRange, BlockState, BrewingFuel, BucketEntityData, Compostable, ContainerLoot,
    CookingFuel, CustomData, CustomModelData, DebugStickState, Fireworks, Food, ItemComponentKind,
    ItemComponentValue, ItemDataComponent, ItemModel, LodestoneTracker, MapDecorations,
    NoteBlockSound, Profile, ProfileIdentity, Recipes, ResolvableFloat, ResolvableInt, SignText,
    SignTextBack, SignTextFront, TooltipDisplay, TooltipStyle, UseCooldown, UseEffects, Weapon,
    hash_ops,
};
use mcrs_minecraft_protocol::profile::Property;

use crate::harness::{TestLookup, from_json, from_nbt, hex, nbt_tree, persistent_json};

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

fn our_nbt(value: &ItemComponentValue) -> Vec<u8> {
    let mut out = Vec::new();
    mcrs_minecraft_nbt::to_bytes_unnamed(value, &mut out).unwrap();
    out
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
    assert_eq!(from_nbt(kind, &vanilla_nbt), value, "{label}: from NBT");

    let vanilla_wire = hex(row["wire"]);
    let mut wire = Vec::new();
    value.encode_ctx(&TestLookup::new(), &mut wire).unwrap();
    if kind.is_nbt_wire() {
        assert_eq!(nbt_tree(&wire), nbt_tree(&vanilla_wire), "{label}: wire");
    } else {
        assert_eq!(wire, vanilla_wire, "{label}: wire");
    }
    let mut r = &vanilla_wire[..];
    let back = decode_component_value(kind, &TestLookup::new(), &mut r).unwrap();
    assert!(r.is_empty(), "{label}: trailing wire bytes");
    assert_eq!(back, value, "{label}: from wire");

    assert_eq!(
        hash_ops::hash(&value).unwrap(),
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

fn json_error(value: impl Into<ItemComponentValue>) -> String {
    let mut out = Vec::new();
    value
        .into()
        .serialize_value(&mut serde_json::Serializer::new(&mut out))
        .unwrap_err()
        .to_string()
}

fn wire_error(kind: ItemComponentKind, wire: &str) -> String {
    let mut r = &hex(wire)[..];
    format!(
        "{:#}",
        decode_component_value(kind, &TestLookup::new(), &mut r).unwrap_err()
    )
}

#[test]
fn flat_records_match_vanilla() {
    check("use_effects_default", sample::<UseEffects>(0));
    check("use_effects_full", sample::<UseEffects>(1));
    check("cmd_empty", sample::<CustomModelData>(0));
    check("cmd_full", sample::<CustomModelData>(1));
    check("tooltip_default", sample::<TooltipDisplay>(0));
    check("tooltip_full", sample::<TooltipDisplay>(1));
    check("food_min", sample::<Food>(0));
    check("food_full", sample::<Food>(1));
    check("cooldown_min", sample::<UseCooldown>(0));
    check("cooldown_full", sample::<UseCooldown>(1));
    check("weapon_default", sample::<Weapon>(0));
    check("weapon_full", sample::<Weapon>(1));
    check("attack_range_default", sample::<AttackRange>(0));
    check("attack_range_full", sample::<AttackRange>(1));
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
    assert!(
        error::<Food>(r#"{"nutrition":-1,"saturation":0.6}"#)
            .starts_with(GOLDEN["food_neg"]["error"])
    );
    assert!(
        error::<Weapon>(r#"{"item_damage_per_attack":-1}"#)
            .starts_with("Value must be non-negative: -1")
    );
    for id in ["minecraft:nope", "nope"] {
        assert!(
            error::<TooltipDisplay>(&format!(r#"{{"hidden_components":["{id}"]}}"#)).starts_with(
                "Unknown registry key in ResourceKey[minecraft:root / minecraft:data_component_type]: minecraft:nope"
            )
        );
    }
    assert!(error::<UseCooldown>(r#"{"seconds":0}"#).starts_with("Value must be positive: 0.0"));
    assert!(
        error::<AttackRange>(r#"{"mob_factor":2.5}"#)
            .starts_with("Value 2.5 outside of range [0.0:2.0]")
    );
    assert!(
        error::<AttackRange>(r#"{"min_reach":65}"#)
            .starts_with("Value must be within range [0.0;64.0]: 65.0")
    );
    assert!(
        error::<UseEffects>(r#"{"speed_multiplier":-0.0}"#)
            .starts_with("Value -0.0 outside of range [0.0:1.0]")
    );
    assert!(
        error::<AttackRange>(r#"{"min_reach":-0.0}"#)
            .starts_with("Value must be within range [0.0;64.0]: -0.0")
    );
    assert!(
        error::<AttackRange>(r#"{"mob_factor":-0.0}"#)
            .starts_with("Value -0.0 outside of range [0.0:2.0]")
    );
    assert!(
        error::<Weapon>(r#"{"disable_blocking_for_seconds":-0.0}"#)
            .starts_with("Value must be non-negative: -0.0")
    );
    assert!(
        error::<Weapon>(r#"{"disable_blocking_for_seconds":1e40}"#)
            .starts_with("Value must be non-negative: ")
    );
    assert!(
        error::<UseCooldown>(r#"{"seconds":-0.0}"#).starts_with("Value must be positive: -0.0")
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
    for (wire, generation) in [
        ("015400026d65050000", 5),
        ("015400026d65ffffffff0f0000", -1),
    ] {
        let mut r = &hex(wire)[..];
        let err = decode_component_value(WrittenBookContent::KIND, &TestLookup::new(), &mut r)
            .err()
            .unwrap();
        assert_eq!(
            err.to_string(),
            format!("Generation was {generation}, but must be between 0 and 3")
        );
    }
    let big_page = format!(
        r#"{{"title":"T","author":"me","pages":["{}"]}}"#,
        "a".repeat(32766)
    );
    assert!(
        error::<WrittenBookContent>(&big_page)
            .starts_with("Component was too large: greater than max size 32767")
    );
    let page_of = |text: &str, count: usize| {
        format!(
            r#"{{"title":"T","author":"a","pages":["{}"]}}"#,
            text.repeat(count)
        )
    };
    for (text, fits) in [("\u{1F600}", 16382), ("\u{2028}", 5460)] {
        parse::<WrittenBookContent>(&page_of(text, fits));
        assert!(
            error::<WrittenBookContent>(&page_of(text, fits + 1))
                .contains("Component was too large: greater than max size 32767")
        );
    }

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
    assert_eq!(
        parse::<SignTextFront>(r#"{"messages":["a","b","c","d"],"filtered_messages":null}"#),
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
    let interleaved = |signature: Option<&str>| {
        vec![
            Property {
                name: "x".into(),
                value: "1".into(),
                signature: None,
            },
            Property {
                name: "x".into(),
                value: "2".into(),
                signature: None,
            },
            Property {
                name: "textures".into(),
                value: "t".into(),
                signature: signature.map(Into::into),
            },
        ]
    };
    let full = parse::<Profile>(
        r#"{"id":[1,2,3,4],"name":"Steve","properties":[{"name":"x","value":"1"},{"name":"textures","value":"t"},{"name":"x","value":"2"}]}"#,
    );
    let ProfileIdentity::Full(profile) = &full.profile else {
        panic!("{full:?}");
    };
    assert_eq!(profile.properties, interleaved(None));
    check("profile_interleaved", full);
    let partial = parse::<Profile>(
        r#"{"name":"Steve","properties":[{"name":"x","value":"1"},{"name":"textures","value":"t","signature":"s"},{"name":"x","value":"2"}]}"#,
    );
    let ProfileIdentity::Partial { properties, .. } = &partial.profile else {
        panic!("{partial:?}");
    };
    assert_eq!(*properties, interleaved(Some("s")));
    check("profile_partial_interleaved", partial);
    assert!(
        error::<Profile>(r#"{"name":"has space"}"#)
            .contains("Player name contained disallowed characters: 'has space'")
    );
    assert!(
        error::<Profile>(r#""abcdefghijklmnopq""#)
            .contains(r#"String "abcdefghijklmnopq" is too long: 17, expected range [0-16]"#)
    );
    let nine = r#"["1","2","3","4","5","6","7","8","9"]"#;
    let eighteen = parse::<Profile>(&format!(
        r#"{{"name":"Steve","properties":{{"a":{nine},"b":{nine}}}}}"#
    ));
    let ProfileIdentity::Partial { properties, .. } = &eighteen.profile else {
        panic!("{eighteen:?}");
    };
    assert_eq!(properties.len(), 18);
    assert_eq!(
        json_error(eighteen.clone()),
        "List is too long: 18, expected range [0-16]"
    );
    let mut wire = Vec::new();
    assert!(
        ItemComponentValue::from(eighteen)
            .encode_ctx(&TestLookup::new(), &mut wire)
            .is_err()
    );
    let name65 = "n".repeat(65);
    let err = wire_error(
        Profile::KIND,
        &format!("0001054e6f746368000141{}01620000000000", "6e".repeat(65)),
    );
    assert!(err.contains("expected <= 64, got 65"), "{err}");
    let mut long_name = Profile::named("Notch").unwrap();
    let ProfileIdentity::Partial { properties, .. } = &mut long_name.profile else {
        panic!("{long_name:?}");
    };
    properties.push(Property {
        name: name65,
        value: "b".into(),
        signature: None,
    });
    let err = ItemComponentValue::from(long_name)
        .encode_ctx(&TestLookup::new(), &mut wire)
        .unwrap_err();
    assert!(
        format!("{err:#}").contains("expected <= 64, got 65"),
        "{err:#}"
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
    let narrowed =
        parse::<FireworkExplosion>(r#"{"shape":"star","colors":[1.5,-2.7],"fade_colors":[3.9]}"#);
    assert_eq!(
        (&narrowed.colors[..], &narrowed.fade_colors[..]),
        (&[1, -2][..], &[3][..])
    );
    check("explosion_float_colors", narrowed);
    check("fireworks_empty", sample::<Fireworks>(0));
    check("fireworks_full", sample::<Fireworks>(1));
    let wrapped = parse::<Fireworks>(r#"{"flight_duration":300}"#);
    assert_eq!(wrapped.flight_duration, 44);
    check("fireworks_big", wrapped);
    let mut r = &hex("ac0200")[..];
    let from_wire = decode_component_value(Fireworks::KIND, &TestLookup::new(), &mut r).unwrap();
    assert!(r.is_empty());
    assert_eq!(
        Fireworks::from_value(&from_wire).unwrap().flight_duration,
        300
    );
    let mut wire = Vec::new();
    from_wire.encode_ctx(&TestLookup::new(), &mut wire).unwrap();
    assert_eq!(wire, hex("ac0200"));
    assert_eq!(
        json_error(from_wire),
        "Unsigned byte was too large: 300 > 255"
    );
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
    check("loot_min", sample::<ContainerLoot>(0));
    check("loot_full", sample::<ContainerLoot>(1));
    assert_eq!(
        parse::<ContainerLoot>(r#"{"loot_table":"minecraft:chests/simple_dungeon","seed":0}"#),
        sample::<ContainerLoot>(0)
    );
    for (label, json, seed) in [
        (
            "loot_float_seed",
            r#"{"loot_table":"chests/x","seed":1.5}"#,
            1,
        ),
        (
            "loot_big_seed",
            r#"{"loot_table":"chests/x","seed":1e19}"#,
            -8446744073709551616,
        ),
    ] {
        let loot = parse::<ContainerLoot>(json);
        assert_eq!(loot.seed, seed);
        check(label, loot);
    }
}

#[test]
fn fuel_matches_vanilla() {
    let int = |path: &str| ResolvableInt::reference(ResourceLocation::minecraft(path));
    let float = |path: &str| ResolvableFloat::reference(ResourceLocation::minecraft(path));
    check("compostable_const", sample::<Compostable>(0));
    check(
        "compostable_ref",
        Compostable {
            layers: int("some_provider"),
        },
    );
    check(
        "cooking_const",
        CookingFuel {
            burn_time: ResolvableInt::Constant(200),
            speed_multiplier: ResolvableFloat::Constant(1.5),
        },
    );
    check(
        "cooking_ref",
        CookingFuel {
            burn_time: int("i"),
            speed_multiplier: float("f"),
        },
    );
    check(
        "brewing_const",
        BrewingFuel {
            uses: ResolvableInt::Constant(20),
            speed_multiplier: ResolvableFloat::Constant(0.5),
        },
    );
    check(
        "brewing_ref",
        BrewingFuel {
            uses: int("i"),
            speed_multiplier: float("f"),
        },
    );
    assert_eq!(
        parse::<Compostable>(r#"{"layers":3}"#),
        sample::<Compostable>(0)
    );
}
