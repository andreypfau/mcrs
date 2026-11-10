use std::io::Cursor;
use std::num::NonZeroU8;

use mcrs_minecraft_fixtures::fixture;
use mcrs_minecraft_item_stack::AttributeId;
use mcrs_minecraft_item_stack::AttributeModifier;
use mcrs_minecraft_item_stack::AttributeModifiers;
use mcrs_minecraft_item_stack::BlockId;
use mcrs_minecraft_item_stack::BlockSet;
use mcrs_minecraft_item_stack::BlockTagId;
use mcrs_minecraft_item_stack::BuiltinKey;
use mcrs_minecraft_item_stack::ComponentType;
use mcrs_minecraft_item_stack::CustomName;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::EnchantmentId;
use mcrs_minecraft_item_stack::EnchantmentLevel;
use mcrs_minecraft_item_stack::Enchantments;
use mcrs_minecraft_item_stack::Food;
use mcrs_minecraft_item_stack::ItemModel;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::MaxDamage;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_stack::ModifierDisplay;
use mcrs_minecraft_item_stack::ModifierOperation;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_stack::PatchEntry;
use mcrs_minecraft_item_stack::Rarity;
use mcrs_minecraft_item_stack::RepairCost;
use mcrs_minecraft_item_stack::SavedError;
use mcrs_minecraft_item_stack::SlotGroup;
use mcrs_minecraft_item_stack::SlotStack;
use mcrs_minecraft_item_stack::Tool;
use mcrs_minecraft_item_stack::ToolRule;
use mcrs_minecraft_item_stack::TooltipDisplay;
use mcrs_minecraft_item_stack::Unbreakable;
use mcrs_minecraft_item_stack::read_saved;
use mcrs_minecraft_item_stack::read_saved_optional;
use mcrs_minecraft_item_stack::read_saved_with_slot;
use mcrs_minecraft_item_stack::write_saved;
use mcrs_minecraft_item_stack::write_saved_optional;
use mcrs_minecraft_item_stack::write_saved_with_slot;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::deserializer::NbtReadHelper;
use mcrs_minecraft_nbt::serializer::WriteAdaptor;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_snbt::parse_compound;
use mcrs_minecraft_snbt::parse_tag;
use mcrs_minecraft_snbt::write_compound;
use mcrs_minecraft_text::Text;
use serde::Deserialize;

use crate::common::stack;
use crate::common::unhex;

const AIR: u16 = 0;
const STONE: u16 = 1;
const DIAMOND_SWORD: u16 = 2;
const SHARPNESS: u16 = 0;
const UNBREAKING: u16 = 1;
const ATTACK_DAMAGE: u16 = 0;
const STONE_BLOCK: u16 = 0;
const DIRT_BLOCK: u16 = 1;
const MINEABLE_PICKAXE: u16 = 0;
const MAX_STACK_SIZE_TYPE: u16 = 0;
const DAMAGE_TYPE: u16 = 1;

/// Each name is in the namespace `minecraft`, and the numeric id of a name is its index.
struct Names;

impl Names {
    fn paths(table: NameTable) -> &'static [&'static str] {
        match table {
            NameTable::Item => &["air", "stone", "diamond_sword"],
            NameTable::Enchantment => &["sharpness", "unbreaking"],
            NameTable::Attribute => &["attack_damage"],
            NameTable::Block => &["stone", "dirt"],
            NameTable::BlockTag => &["mineable/pickaxe"],
            NameTable::ComponentType => {
                &["max_stack_size", "damage", "bundle_contents", "custom_data"]
            }
            _ => &[],
        }
    }
}

impl NameLookup for Names {
    fn id(&self, table: NameTable, name: ResourceLocation<'_>) -> Option<u16> {
        if name.namespace() != "minecraft" {
            return None;
        }
        let index = Self::paths(table)
            .iter()
            .position(|path| *path == name.path())?;
        u16::try_from(index).ok()
    }

    fn name(&self, table: NameTable, id: u16) -> Option<ResourceLocation<'_>> {
        let path = Self::paths(table).get(usize::from(id))?;
        ResourceLocation::new("minecraft", path).ok()
    }
}

fn saved(snbt: &str) -> NbtCompound {
    parse_compound(snbt).expect("the test input is valid SNBT")
}

fn with_components(components: &str) -> NbtCompound {
    saved(&format!(
        "{{id:\"minecraft:stone\",components:{{{components}}}}}"
    ))
}

/// Equal tags, where the order of the keys of a map has no meaning.
fn same(left: &NbtTag, right: &NbtTag) -> bool {
    match (left, right) {
        (NbtTag::Compound(left), NbtTag::Compound(right)) => same_map(left, right),
        (NbtTag::List(left), NbtTag::List(right)) => {
            left.len() == right.len()
                && left
                    .iter()
                    .zip(right)
                    .all(|(left, right)| same(left, right))
        }
        _ => left == right,
    }
}

fn same_map(left: &NbtCompound, right: &NbtCompound) -> bool {
    left.child_tags.len() == right.child_tags.len()
        && left
            .child_tags
            .iter()
            .all(|(key, value)| right.get(key).is_some_and(|other| same(value, other)))
}

fn assert_same(actual: &NbtCompound, expected: &NbtCompound, case: &str) {
    assert!(
        same_map(actual, expected),
        "{case}: wrote {}, expected {}",
        write_compound(actual),
        write_compound(expected)
    );
}

fn tag_of(bytes: &[u8]) -> NbtTag {
    NbtTag::deserialize(&mut NbtReadHelper::new(Cursor::new(bytes)))
        .expect("the golden holds one NBT value")
}

fn bytes_of(tag: &NbtTag) -> Vec<u8> {
    let mut writer = WriteAdaptor::new(Vec::new());
    tag.serialize(&mut writer)
        .expect("a tag writes to a vector");
    writer.into_inner()
}

fn modifier(id: &str, amount: f64, operation: ModifierOperation) -> AttributeModifier {
    let id = ResourceLocationBuf::read(id).expect("the test id is an identifier");
    AttributeModifier::new(AttributeId(ATTACK_DAMAGE), id, amount, operation)
}

#[test]
fn a_saved_stack_with_each_decoded_type_reads_to_its_value_and_writes_back() {
    let nbt = saved(
        r##"{id:"minecraft:diamond_sword",count:2,components:{
            "minecraft:max_stack_size":16,
            "minecraft:max_damage":1561,
            "minecraft:damage":7,
            "minecraft:unbreakable":{},
            "minecraft:item_model":"minecraft:diamond_sword",
            "minecraft:rarity":"epic",
            "minecraft:enchantments":{"minecraft:sharpness":5,"minecraft:unbreaking":255},
            "minecraft:attribute_modifiers":[
                {type:"minecraft:attack_damage",id:"minecraft:base_attack_damage",amount:6.0d,operation:"add_value",slot:"mainhand"},
                {type:"minecraft:attack_damage",id:"test:hidden",amount:-2.5d,operation:"add_multiplied_total",display:{type:"hidden"}}],
            "minecraft:tooltip_display":{hide_tooltip:1b,hidden_components:["minecraft:damage","minecraft:max_stack_size"]},
            "minecraft:repair_cost":3,
            "minecraft:food":{nutrition:4,saturation:1.8000001f,can_always_eat:1b},
            "minecraft:tool":{rules:[
                {blocks:"#minecraft:mineable/pickaxe",speed:8.0f,correct_for_drops:1b},
                {blocks:["minecraft:stone","minecraft:dirt"]},
                {blocks:"minecraft:dirt",correct_for_drops:0b}],
                default_mining_speed:2.5f,damage_per_block:0,can_destroy_blocks_in_creative:0b}
        }}"##,
    );

    let level = |enchantment: u16, level: u8| EnchantmentLevel {
        enchantment: EnchantmentId(enchantment),
        level: NonZeroU8::new(level).expect("the test levels are not zero"),
    };
    let mut base = modifier(
        "minecraft:base_attack_damage",
        6.0,
        ModifierOperation::AddValue,
    );
    base.slot = SlotGroup::Mainhand;
    let mut hidden = modifier("test:hidden", -2.5, ModifierOperation::AddMultipliedTotal);
    hidden.display = ModifierDisplay::Hidden;
    let mut tooltip = TooltipDisplay::hiding([
        ComponentType(DAMAGE_TYPE),
        ComponentType(MAX_STACK_SIZE_TYPE),
    ]);
    tooltip.hide_tooltip = true;
    let mut tag_rule = ToolRule::new(BlockSet::Tag(BlockTagId(MINEABLE_PICKAXE)))
        .with_speed(8.0)
        .expect("8 is a valid speed");
    tag_rule.correct_for_drops = Some(true);
    let list_rule = ToolRule::new(BlockSet::Blocks(
        vec![BlockId(STONE_BLOCK), BlockId(DIRT_BLOCK)].into(),
    ));
    let mut block_rule = ToolRule::new(BlockSet::Blocks(vec![BlockId(DIRT_BLOCK)].into()));
    block_rule.correct_for_drops = Some(false);
    let mut tool = Tool::new(vec![tag_rule, list_rule, block_rule]);
    tool.default_mining_speed = 2.5;
    tool.damage_per_block = 0;
    tool.can_destroy_blocks_in_creative = false;
    let model = ResourceLocationBuf::read("minecraft:diamond_sword").expect("an identifier");
    let expected = stack(DIAMOND_SWORD, 2)
        .set(MaxStackSize(16))
        .set(MaxDamage(1561))
        .set(Damage(7))
        .set(Unbreakable)
        .set(ItemModel::new(model))
        .set(Rarity::Epic)
        .set(Enchantments::new([level(SHARPNESS, 5), level(UNBREAKING, 255)]).expect("two types"))
        .set(AttributeModifiers {
            modifiers: vec![base, hidden].into(),
        })
        .set(tooltip)
        .set(RepairCost(3))
        .set(Food {
            nutrition: 4,
            saturation: 1.800_000_1,
            can_always_eat: true,
        })
        .set(tool);

    assert_eq!(read_saved(&nbt, &Names), Ok(expected.clone()));
    let written = write_saved(&expected, &Names).expect("each value is in range");
    assert_same(&written, &nbt, "each decoded type");
}

#[test]
fn the_writer_gives_the_canonical_form_of_what_the_reader_accepts() {
    let cases: [(&str, &str); 9] = [
        (r#"{id:"stone"}"#, r#"{id:"minecraft:stone",count:1}"#),
        (
            r#"{id:"stone",count:3b,components:{"minecraft:max_stack_size":64.5f,"minecraft:max_damage":10000000000.0d,"minecraft:damage":-0.5d,"minecraft:repair_cost":4294967298L,"minecraft:enchantments":{sharpness:3s},"minecraft:attribute_modifiers":[{type:"attack_damage",id:"base",amount:2,operation:"add_value"}],"minecraft:tooltip_display":{hide_tooltip:2b},"minecraft:food":{nutrition:4.9f,saturation:1,can_always_eat:0.5f},"minecraft:tool":{rules:[{blocks:"stone",speed:2b,correct_for_drops:-1}],default_mining_speed:3L,damage_per_block:2.9d,can_destroy_blocks_in_creative:0L}}}"#,
            r#"{id:"minecraft:stone",count:3,components:{"minecraft:max_stack_size":64,"minecraft:max_damage":2147483647,"minecraft:damage":0,"minecraft:repair_cost":2,"minecraft:enchantments":{"minecraft:sharpness":3},"minecraft:attribute_modifiers":[{type:"minecraft:attack_damage",id:"minecraft:base",amount:2.0d,operation:"add_value"}],"minecraft:tooltip_display":{hide_tooltip:1b},"minecraft:food":{nutrition:4,saturation:1.0f,can_always_eat:1b},"minecraft:tool":{rules:[{blocks:"minecraft:stone",speed:2.0f,correct_for_drops:1b}],default_mining_speed:3.0f,damage_per_block:2,can_destroy_blocks_in_creative:0b}}}"#,
        ),
        (
            r#"{id:"stone",components:{damage:3,"!minecraft:damage":{},"!repair_cost":{},"minecraft:repair_cost":5}}"#,
            r#"{id:"minecraft:stone",count:1,components:{"!minecraft:damage":{},"minecraft:repair_cost":5}}"#,
        ),
        (
            r#"{id:"minecraft:stone",count:5,components:{"minecraft:bundle_contents":[{id:"minecraft:stone"}],"!custom_data":{x:1},repair_cost:2,"!damage":{x:1},"minecraft:custom_name":"named"}}"#,
            r#"{id:"minecraft:stone",count:5,components:{"!minecraft:damage":{},"minecraft:custom_name":"named","minecraft:repair_cost":2,"minecraft:bundle_contents":[{id:"minecraft:stone"}],"!minecraft:custom_data":{}}}"#,
        ),
        (
            r#"{id:"minecraft:stone",count:1,components:{"minecraft:unbreakable":{x:1}}}"#,
            r#"{id:"minecraft:stone",count:1,components:{"minecraft:unbreakable":{}}}"#,
        ),
        (
            r#"{id:"minecraft:stone",count:1,components:{"minecraft:attribute_modifiers":[{type:"attack_damage",id:"base",amount:1.5d,operation:"add_multiplied_base",slot:"any",display:{type:"default"}}],"minecraft:food":{nutrition:0,saturation:0.5f,can_always_eat:0b}}}"#,
            r#"{id:"minecraft:stone",count:1,components:{"minecraft:attribute_modifiers":[{type:"minecraft:attack_damage",id:"minecraft:base",amount:1.5d,operation:"add_multiplied_base"}],"minecraft:food":{nutrition:0,saturation:0.5f}}}"#,
        ),
        (
            r#"{id:"minecraft:stone",count:1,components:{"minecraft:tool":{rules:[{blocks:["stone"]}],default_mining_speed:1.0f,damage_per_block:1,can_destroy_blocks_in_creative:1b},"minecraft:enchantments":{unbreaking:1}}}"#,
            r#"{id:"minecraft:stone",count:1,components:{"minecraft:tool":{rules:[{blocks:"minecraft:stone"}]},"minecraft:enchantments":{"minecraft:unbreaking":1}}}"#,
        ),
        (
            r#"{id:"minecraft:stone",count:1,components:{"minecraft:tooltip_display":{hide_tooltip:0b,hidden_components:["minecraft:damage","damage","max_stack_size"]},"minecraft:item_model":"stone"}}"#,
            r#"{id:"minecraft:stone",count:1,components:{"minecraft:tooltip_display":{hidden_components:["minecraft:damage","minecraft:max_stack_size"]},"minecraft:item_model":"minecraft:stone"}}"#,
        ),
        (
            r#"{id:"minecraft:stone",count:1,components:{"minecraft:attribute_modifiers":[{type:"minecraft:attack_damage",id:"test:shown",amount:1.0d,operation:"add_value",display:{type:"override",value:{text:"shown",bold:1b}}}]}}"#,
            r#"{id:"minecraft:stone",count:1,components:{"minecraft:attribute_modifiers":[{type:"minecraft:attack_damage",id:"test:shown",amount:1.0d,operation:"add_value",display:{type:"override",value:{text:"shown",bold:1b}}}]}}"#,
        ),
    ];
    for (input, canonical) in cases {
        let stack = read_saved(&saved(input), &Names).expect(input);
        let written = write_saved(&stack, &Names).expect(input);
        assert_same(&written, &saved(canonical), input);
    }
}

#[test]
fn the_patch_keeps_one_order_whatever_the_order_of_the_saved_keys() {
    let forward = with_components(
        r#""minecraft:bundle_contents":[],"!minecraft:custom_data":{},"minecraft:repair_cost":2,"!minecraft:damage":{},"minecraft:custom_name":"named""#,
    );
    let backward = with_components(
        r#""minecraft:custom_name":"named","!minecraft:damage":{},"minecraft:repair_cost":2,"!minecraft:custom_data":{},"minecraft:bundle_contents":[]"#,
    );

    let stack = read_saved(&forward, &Names).expect("each key is valid");
    assert_eq!(read_saved(&backward, &Names).as_ref(), Ok(&stack));

    let undecoded: Vec<&str> = stack
        .patch()
        .iter()
        .filter_map(|entry| match entry {
            PatchEntry::Undecoded(value) => Some(value.name.as_str()),
            _ => None,
        })
        .collect();
    assert_eq!(undecoded, ["minecraft:bundle_contents"]);

    let written = write_saved(&stack, &Names).expect("each entry has a saved form");
    let Some(NbtTag::Compound(patch)) = written.get("components") else {
        panic!("the writer wrote no patch: {}", write_compound(&written));
    };
    let keys: Vec<&str> = patch
        .child_tags
        .iter()
        .map(|(key, _)| key.as_str())
        .collect();
    assert_eq!(
        keys,
        [
            "!minecraft:damage",
            "minecraft:custom_name",
            "minecraft:repair_cost",
            "minecraft:bundle_contents",
            "!minecraft:custom_data",
        ]
    );
}

#[test]
fn the_saved_count_is_an_int_from_1_to_99_and_an_absent_count_is_1() {
    let cases: [(&str, Result<u8, SavedError>); 12] = [
        ("", Ok(1)),
        (",count:1", Ok(1)),
        (",count:99", Ok(99)),
        (",count:0", Err(SavedError::BadCount)),
        (",count:-1", Err(SavedError::BadCount)),
        (",count:100", Err(SavedError::BadCount)),
        (",count:257", Err(SavedError::BadCount)),
        (",count:1b", Ok(1)),
        (",count:64.5f", Ok(64)),
        (",count:4294967298L", Ok(2)),
        (",count:0.5f", Err(SavedError::BadCount)),
        (",count:\"1\"", Err(SavedError::BadCount)),
    ];
    for (count, expected) in cases {
        let nbt = saved(&format!("{{id:\"minecraft:stone\"{count}}}"));
        let read = read_saved(&nbt, &Names).map(|stack| stack.count.get());
        assert_eq!(read, expected, "count `{count}`");
    }

    assert!(write_saved(&stack(STONE, 99), &Names).is_ok());
    assert_eq!(
        write_saved(&stack(STONE, 100), &Names),
        Err(SavedError::BadCount)
    );
}

#[test]
fn the_reader_names_the_rule_that_a_saved_stack_breaks() {
    let frames: [(&str, SavedError); 6] = [
        (r#"{count:1}"#, SavedError::BadItemId),
        (r#"{id:"Not An Id"}"#, SavedError::BadItemId),
        (r#"{id:7}"#, SavedError::BadItemId),
        (r#"{id:"minecraft:emerald"}"#, SavedError::UnknownItem),
        (r#"{id:"air"}"#, SavedError::AirItem),
        (r#"{id:"stone",components:[]}"#, SavedError::BadPatch),
    ];
    for (input, expected) in frames {
        assert_eq!(read_saved(&saved(input), &Names), Err(expected), "{input}");
    }

    let unknown =
        |component: BuiltinKey, table: NameTable| SavedError::UnknownEntry { component, table };
    let patches: [(&str, SavedError); 23] = [
        (r#""Bad Key":1"#, SavedError::BadKey),
        (r#""!Bad Key":{}"#, SavedError::BadKey),
        (
            r#""minecraft:no_such_type":1"#,
            SavedError::UnknownComponentType,
        ),
        (
            r#""!minecraft:no_such_type":{}"#,
            SavedError::UnknownComponentType,
        ),
        (r#""!minecraft:damage":1b"#, SavedError::BadRemovalValue),
        (r#"damage:1,"minecraft:damage":2"#, SavedError::RepeatedKey),
        (
            r#""!bundle_contents":{},"!minecraft:bundle_contents":{}"#,
            SavedError::RepeatedKey,
        ),
        (
            r#""minecraft:damage":-1.5f"#,
            SavedError::OutOfRange(BuiltinKey::Damage),
        ),
        (
            r#""minecraft:damage":"7""#,
            SavedError::BadValue(BuiltinKey::Damage),
        ),
        (
            r#""minecraft:unbreakable":1b"#,
            SavedError::BadValue(BuiltinKey::Unbreakable),
        ),
        (
            r#""minecraft:rarity":"legendary""#,
            SavedError::BadValue(BuiltinKey::Rarity),
        ),
        (
            r#""minecraft:item_model":"Not An Id""#,
            SavedError::BadValue(BuiltinKey::ItemModel),
        ),
        (
            r#""minecraft:food":{nutrition:1}"#,
            SavedError::BadValue(BuiltinKey::Food),
        ),
        (
            r#""minecraft:tooltip_display":{hide_tooltip:"true"}"#,
            SavedError::BadValue(BuiltinKey::TooltipDisplay),
        ),
        (
            r#""minecraft:attribute_modifiers":[{type:"attack_damage",id:"base",amount:1.0d,operation:"add_value",display:{type:"override"}}]"#,
            SavedError::BadValue(BuiltinKey::AttributeModifiers),
        ),
        (
            r#""minecraft:enchantments":{"minecraft:sharpness":0}"#,
            SavedError::OutOfRange(BuiltinKey::Enchantments),
        ),
        (
            r#""minecraft:enchantments":{"minecraft:sharpness":256}"#,
            SavedError::OutOfRange(BuiltinKey::Enchantments),
        ),
        (
            r#""minecraft:food":{nutrition:-1,saturation:0.0f}"#,
            SavedError::OutOfRange(BuiltinKey::Food),
        ),
        (
            r#""minecraft:tool":{rules:[{blocks:"stone",speed:0.0f}]}"#,
            SavedError::OutOfRange(BuiltinKey::Tool),
        ),
        (
            r#""minecraft:enchantments":{"minecraft:mending":1}"#,
            unknown(BuiltinKey::Enchantments, NameTable::Enchantment),
        ),
        (
            r##""minecraft:tool":{rules:[{blocks:"#minecraft:no_such_tag"}]}"##,
            unknown(BuiltinKey::Tool, NameTable::BlockTag),
        ),
        (
            r#""minecraft:tool":{rules:[{blocks:["stone","bedrock"]}]}"#,
            unknown(BuiltinKey::Tool, NameTable::Block),
        ),
        (
            r#""minecraft:tooltip_display":{hidden_components:["minecraft:no_such_type"]}"#,
            unknown(BuiltinKey::TooltipDisplay, NameTable::ComponentType),
        ),
    ];
    for (components, expected) in patches {
        let nbt = with_components(components);
        assert_eq!(read_saved(&nbt, &Names), Err(expected), "{components}");
    }
}

#[test]
fn the_writer_names_the_rule_that_a_stack_breaks() {
    let unnamed_enchantment = Enchantments::new([EnchantmentLevel {
        enchantment: EnchantmentId(9),
        level: NonZeroU8::MIN,
    }])
    .expect("one enchantment");
    let cases: [(ItemStack, SavedError); 5] = [
        (stack(999, 1), SavedError::UnnamedId(NameTable::Item)),
        (stack(AIR, 1), SavedError::AirItem),
        (
            stack(STONE, 1).set(MaxStackSize(100)),
            SavedError::OutOfRange(BuiltinKey::MaxStackSize),
        ),
        (
            stack(STONE, 1).set(Damage(-1)),
            SavedError::OutOfRange(BuiltinKey::Damage),
        ),
        (
            stack(STONE, 1).set(unnamed_enchantment),
            SavedError::UnnamedId(NameTable::Enchantment),
        ),
    ];
    for (stack, expected) in cases {
        assert_eq!(write_saved(&stack, &Names), Err(expected), "{stack:?}");
    }
}

#[test]
fn the_optional_form_is_an_empty_map_and_the_slot_is_an_unsigned_byte() {
    assert_eq!(read_saved_optional(&NbtCompound::new(), &Names), Ok(None));
    assert_eq!(write_saved_optional(None, &Names), Ok(NbtCompound::new()));
    let stone = stack(STONE, 3);
    let written = write_saved_optional(Some(&stone), &Names).expect("a stone has a saved form");
    assert_eq!(
        read_saved_optional(&written, &Names),
        Ok(Some(stone.clone()))
    );

    let slots: [(&str, Result<u8, SavedError>); 4] = [
        (r#"{id:"stone"}"#, Ok(0)),
        (r#"{id:"stone",Slot:5b}"#, Ok(5)),
        (r#"{id:"stone",Slot:-1b}"#, Ok(255)),
        (r#"{id:"stone",Slot:5}"#, Err(SavedError::BadSlot)),
    ];
    for (input, expected) in slots {
        let read = read_saved_with_slot(&saved(input), &Names).map(|slotted| slotted.slot);
        assert_eq!(read, expected, "{input}");
    }
    let slotted = SlotStack {
        slot: 255,
        stack: stone,
    };
    let written = write_saved_with_slot(&slotted, &Names).expect("a stone has a saved form");
    assert_same(
        &written,
        &saved(r#"{id:"minecraft:stone",count:3,Slot:-1b}"#),
        "slot 255",
    );
}

#[derive(Deserialize)]
struct NestedGolden {
    cases: Vec<NestedCase>,
}

#[derive(Deserialize)]
struct NestedCase {
    name: String,
    nbt: String,
}

#[test]
fn a_stack_that_the_game_saved_reads_to_its_value_and_writes_back() {
    let golden: NestedGolden =
        serde_json::from_str(&fixture("item_nested").text()).expect("the golden is JSON");
    let case = golden
        .cases
        .iter()
        .find(|case| case.name == "use_remainder_full")
        .expect("the golden has the case");
    let NbtTag::Compound(nbt) = tag_of(&unhex(&case.nbt)) else {
        panic!("the case is not a map");
    };

    let read = read_saved(&nbt, &Names).expect("the game wrote a valid stack");

    let decoded = stack(DIAMOND_SWORD, 3)
        .set(MaxStackSize(16))
        .set(Damage(7))
        .set(Unbreakable)
        .set(CustomName(Text::text("named")))
        .unset::<RepairCost>();
    assert_eq!(read, decoded);

    let written = write_saved(&read, &Names).expect("the stack has a saved form");
    assert_same(&written, &nbt, "use_remainder_full");
}

#[derive(Deserialize)]
struct PlainGolden {
    values: Vec<PlainValue>,
    errors: Vec<PlainError>,
}

#[derive(Deserialize)]
struct PlainValue {
    kind: String,
    /// Absent for a component type that has no saved form.
    nbt: Option<String>,
}

#[derive(Deserialize)]
struct PlainError {
    kind: String,
    input: String,
}

const PLAIN_KINDS: [(&str, BuiltinKey); 6] = [
    ("max_stack_size", BuiltinKey::MaxStackSize),
    ("max_damage", BuiltinKey::MaxDamage),
    ("damage", BuiltinKey::Damage),
    ("unbreakable", BuiltinKey::Unbreakable),
    ("rarity", BuiltinKey::Rarity),
    ("repair_cost", BuiltinKey::RepairCost),
];

fn plain_kind(kind: &str) -> Option<BuiltinKey> {
    PLAIN_KINDS
        .iter()
        .find(|(name, _)| *name == kind)
        .map(|(_, component)| *component)
}

fn with_component(kind: &str, value: NbtTag) -> NbtCompound {
    let mut patch = NbtCompound::new();
    patch.put(&format!("minecraft:{kind}"), value);
    let mut nbt = saved(r#"{id:"minecraft:stone"}"#);
    nbt.put("components", NbtTag::Compound(patch));
    nbt
}

#[test]
fn the_components_that_the_game_saved_decode_and_write_the_same_bytes() {
    let golden: PlainGolden =
        serde_json::from_str(&fixture("item_plain").text()).expect("the golden is JSON");

    let mut checked = 0;
    for value in &golden.values {
        if plain_kind(&value.kind).is_none() {
            continue;
        }
        let hex = value.nbt.as_ref().expect("each of the six types is saved");
        let bytes = unhex(hex);
        let nbt = with_component(&value.kind, tag_of(&bytes));
        let read = read_saved(&nbt, &Names).expect("the game wrote a valid component");
        assert!(
            matches!(read.patch(), [PatchEntry::Value(_)]),
            "{} {hex} is not decoded",
            value.kind
        );
        let written = write_saved(&read, &Names).expect("the component has a saved form");
        let Some(NbtTag::Compound(patch)) = written.get("components") else {
            panic!("the writer wrote no patch for {}", value.kind);
        };
        let key = format!("minecraft:{}", value.kind);
        let written = patch.get(&key).expect("the writer wrote the component");
        assert_eq!(bytes_of(written), bytes, "{} {hex}", value.kind);
        checked += 1;
    }
    assert!(checked >= 17, "the golden lost cases: {checked}");

    let mut rejected = 0;
    for error in &golden.errors {
        let Some(component) = plain_kind(&error.kind) else {
            continue;
        };
        let input = parse_tag(&error.input).expect("the rejected input is valid SNBT");
        let expected = match component {
            BuiltinKey::Unbreakable | BuiltinKey::Rarity => SavedError::BadValue(component),
            _ => SavedError::OutOfRange(component),
        };
        let nbt = with_component(&error.kind, input);
        assert_eq!(
            read_saved(&nbt, &Names),
            Err(expected),
            "{} {}",
            error.kind,
            error.input
        );
        rejected += 1;
    }
    assert!(rejected >= 8, "the golden lost error cases: {rejected}");
}
