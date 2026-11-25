use std::io::Cursor;
use std::num::NonZeroU8;

use mcrs_minecraft_entity_attribute::ModifierOperation;
use mcrs_minecraft_fixtures::fixture;
use mcrs_minecraft_item_stack::AttackAnimation;
use mcrs_minecraft_item_stack::AttributeId;
use mcrs_minecraft_item_stack::AttributeModifier;
use mcrs_minecraft_item_stack::AttributeModifiers;
use mcrs_minecraft_item_stack::BlockId;
use mcrs_minecraft_item_stack::BlockSet;
use mcrs_minecraft_item_stack::BlockTagId;
use mcrs_minecraft_item_stack::Builtin;
use mcrs_minecraft_item_stack::BuiltinKey;
use mcrs_minecraft_item_stack::ComponentType;
use mcrs_minecraft_item_stack::Container;
use mcrs_minecraft_item_stack::CustomData;
use mcrs_minecraft_item_stack::CustomName;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::EnchantmentId;
use mcrs_minecraft_item_stack::EnchantmentLevel;
use mcrs_minecraft_item_stack::Enchantments;
use mcrs_minecraft_item_stack::Food;
use mcrs_minecraft_item_stack::HashError;
use mcrs_minecraft_item_stack::HashScratch;
use mcrs_minecraft_item_stack::HashedStack;
use mcrs_minecraft_item_stack::ItemModel;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Lore;
use mcrs_minecraft_item_stack::MapDecorations;
use mcrs_minecraft_item_stack::MaxDamage;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_stack::ModifierDisplay;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_stack::PatchEntry;
use mcrs_minecraft_item_stack::Rarity;
use mcrs_minecraft_item_stack::RepairCost;
use mcrs_minecraft_item_stack::SavedError;
use mcrs_minecraft_item_stack::SlotGroup;
use mcrs_minecraft_item_stack::SwingAnimation;
use mcrs_minecraft_item_stack::SwingKind;
use mcrs_minecraft_item_stack::Tool;
use mcrs_minecraft_item_stack::ToolRule;
use mcrs_minecraft_item_stack::TooltipDisplay;
use mcrs_minecraft_item_stack::Unbreakable;
use mcrs_minecraft_item_stack::WireError;
use mcrs_minecraft_item_stack::WireStack;
use mcrs_minecraft_item_stack::WireStackWithLength;
use mcrs_minecraft_item_stack::hash_stack;
use mcrs_minecraft_item_stack::read_saved;
use mcrs_minecraft_item_stack::read_wire;
use mcrs_minecraft_item_stack::read_wire_not_empty;
use mcrs_minecraft_item_stack::read_wire_with_length;
use mcrs_minecraft_item_stack::write_saved;
use mcrs_minecraft_item_stack::write_wire;
use mcrs_minecraft_item_stack::write_wire_not_empty;
use mcrs_minecraft_item_stack::write_wire_with_length;
use mcrs_minecraft_nbt::NbtBudget;
use mcrs_minecraft_nbt::NbtQuotaExceeded;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::deserializer::NbtReadHelper;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_nbt::with_nbt_budget;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_snbt::parse_compound;
use mcrs_minecraft_text::Color;
use mcrs_minecraft_text::IntoText;
use mcrs_minecraft_text::Text;
use mcrs_minecraft_wire::Decode;
use mcrs_minecraft_wire::ErrorKind;
use serde::Deserialize;

use crate::common::COMPONENT_TYPES;
use crate::common::GoldenSet;
use crate::common::golden_cases;
use crate::common::same;
use crate::common::stack;
use crate::common::unhex;
use crate::common::var_int;

const AIR: u16 = 0;
const STONE: u16 = 1;

const ITEMS: [(u16, &str); 9] = [
    (0, "air"),
    (1, "stone"),
    (678, "shulker_box"),
    (1030, "apple"),
    (1032, "arrow"),
    (1035, "diamond"),
    (1036, "emerald"),
    (1073, "diamond_sword"),
    (1418, "firework_rocket"),
];

struct Names;

impl Names {
    fn entries(table: NameTable) -> Vec<(u16, &'static str)> {
        let indexed = |paths: &[&'static str]| -> Vec<(u16, &'static str)> {
            (0..).zip(paths.iter().copied()).collect()
        };
        match table {
            NameTable::Item => ITEMS.to_vec(),
            NameTable::Enchantment => vec![(33, "sharpness"), (40, "unbreaking")],
            NameTable::Attribute => indexed(&["attack_damage"]),
            NameTable::Block => indexed(&["stone", "dirt"]),
            NameTable::BlockTag => indexed(&["mineable/pickaxe"]),
            NameTable::ComponentType => indexed(&COMPONENT_TYPES),
            NameTable::MapDecorationType => indexed(&["player", "red_x", "target_x", "banner_red"]),
            NameTable::SoundEvent => vec![
                (50, "item.armor.equip_generic"),
                (52, "item.armor.equip_iron"),
                (653, "entity.item.break"),
                (999, "item.shears.snip"),
            ],
            NameTable::EntityType => vec![(11, "bee"), (30, "cow"), (103, "pig"), (156, "zombie")],
            NameTable::EntityTypeTag => indexed(&["skeletons"]),
            NameTable::MobEffect => indexed(&["speed", "slowness", "haste"]),
            NameTable::Potion => indexed(&["water"]),
            NameTable::TrimMaterial => indexed(&["amethyst"]),
            NameTable::TrimPattern => indexed(&["bolt", "coast"]),
            NameTable::Instrument => vec![(4, "ponder_goat_horn")],
            NameTable::BannerPattern => vec![(4, "creeper")],
            NameTable::PaintingVariant => vec![(0, "alban"), (1, "aztec"), (24, "kebab")],
            _ => Vec::new(),
        }
    }
}

impl NameLookup for Names {
    fn id(&self, table: NameTable, name: ResourceLocation<'_>) -> Option<u16> {
        if name.namespace() != "minecraft" {
            return None;
        }
        Self::entries(table)
            .into_iter()
            .find(|(_, path)| *path == name.path())
            .map(|(id, _)| id)
    }

    fn name(&self, table: NameTable, id: u16) -> Option<ResourceLocation<'_>> {
        let (_, path) = Self::entries(table)
            .into_iter()
            .find(|(entry, _)| *entry == id)?;
        ResourceLocation::new("minecraft", path).ok()
    }
}

struct ItemsOnly;

impl NameLookup for ItemsOnly {
    fn id(&self, table: NameTable, name: ResourceLocation<'_>) -> Option<u16> {
        (table == NameTable::Item).then(|| Names.id(table, name))?
    }

    fn name(&self, table: NameTable, id: u16) -> Option<ResourceLocation<'_>> {
        (table == NameTable::Item).then(|| Names.name(table, id))?
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn wire(stack: Option<&ItemStack>) -> Vec<u8> {
    let mut output = Vec::new();
    write_wire(stack, &Names, &mut output).expect("the test stack has a wire form");
    output
}

fn wire_with_length(stack: Option<&ItemStack>) -> Vec<u8> {
    let mut output = Vec::new();
    write_wire_with_length(stack, &Names, &mut output).expect("the test stack has a wire form");
    output
}

fn read_all<T>(
    bytes: &[u8],
    read: impl Fn(&mut &[u8], &Names) -> Result<T, WireError>,
) -> Result<T, WireError> {
    let mut input = bytes;
    let value = read(&mut input, &Names)?;
    assert!(input.is_empty(), "{} bytes are left", input.len());
    Ok(value)
}

fn level(enchantment: u16, level: u8) -> EnchantmentLevel {
    EnchantmentLevel {
        enchantment: EnchantmentId(enchantment),
        level: NonZeroU8::new(level).expect("the test levels are not zero"),
    }
}

fn modifiers() -> AttributeModifiers {
    let id = ResourceLocationBuf::read("minecraft:base").expect("an identifier");
    let mut modifier = AttributeModifier::new(AttributeId(0), id, 6.0, ModifierOperation::AddValue);
    modifier.slot = SlotGroup::Mainhand;
    modifier.display = ModifierDisplay::Hidden;
    AttributeModifiers {
        modifiers: vec![modifier].into(),
    }
}

fn tooltip() -> TooltipDisplay {
    let mut display = TooltipDisplay::hiding([ComponentType(3), ComponentType(1)]);
    display.hide_tooltip = true;
    display
}

fn tool() -> Tool {
    let mut tag_rule = ToolRule::new(BlockSet::Tag(BlockTagId(0)))
        .with_speed(8.0)
        .expect("8 is a valid speed");
    tag_rule.correct_for_drops = Some(true);
    let list_rule = ToolRule::new(BlockSet::Blocks(vec![BlockId(0), BlockId(1)].into()));
    let mut tool = Tool::new(vec![tag_rule, list_rule]);
    tool.default_mining_speed = 2.5;
    tool.damage_per_block = 0;
    tool.can_destroy_blocks_in_creative = false;
    tool
}

fn one_entry_cases() -> Vec<(&'static str, ItemStack, &'static str, &'static str)> {
    let stone = || stack(STONE, 1);
    let model = ResourceLocationBuf::read("minecraft:stone").expect("an identifier");
    vec![
        ("max_stack_size", stone().set(MaxStackSize(16)), "01", "10"),
        ("max_damage", stone().set(MaxDamage(1561)), "02", "990c"),
        ("damage", stone().set(Damage(7)), "03", "07"),
        ("unbreakable", stone().set(Unbreakable), "04", ""),
        (
            "item_model",
            stone().set(ItemModel::new(model)),
            "0a",
            "0f 6d696e6563726166743a73746f6e65",
        ),
        ("rarity", stone().set(Rarity::Epic), "0c", "03"),
        (
            "enchantments",
            stone().set(Enchantments::new([level(33, 5), level(40, 255)]).expect("two types")),
            "0d",
            "02 21 05 28 ff01",
        ),
        (
            "attribute_modifiers",
            stone().set(modifiers()),
            "10",
            "01 00 0e 6d696e6563726166743a62617365 4018000000000000 00 01 01",
        ),
        (
            "tooltip_display",
            stone().set(tooltip()),
            "12",
            "01 02 03 01",
        ),
        ("repair_cost", stone().set(RepairCost(3)), "13", "03"),
        (
            "food",
            stone().set(Food {
                nutrition: 4,
                saturation: 1.800_000_1,
                can_always_eat: true,
            }),
            "17",
            "04 3fe66667 01",
        ),
        (
            "tool",
            stone().set(tool()),
            "1c",
            "02 00 1a 6d696e6563726166743a6d696e6561626c652f7069636b617865 01 41000000 01 01
             03 00 01 00 00 40200000 00 00",
        ),
        (
            "map_decorations",
            stone().set(MapDecorations::new([]).expect("no key repeats")),
            "31",
            "0a 00",
        ),
    ]
}

#[test]
fn each_decoded_type_writes_the_bytes_of_the_specification_and_reads_back() {
    for (name, stack, type_id, component) in one_entry_cases() {
        let component = unhex(component);
        let bare = [unhex("01 01 01 00"), unhex(type_id), component.clone()].concat();
        let length = u8::try_from(component.len()).expect("the test components are short");
        let prefixed = [
            unhex("01 01 01 00"),
            unhex(type_id),
            vec![length],
            component,
        ]
        .concat();

        assert_eq!(hex(&wire(Some(&stack))), hex(&bare), "{name}");
        assert_eq!(
            hex(&wire_with_length(Some(&stack))),
            hex(&prefixed),
            "{name}"
        );
        assert_eq!(
            read_all(&bare, read_wire),
            Ok(Some(stack.clone())),
            "{name}"
        );
        assert_eq!(
            read_all(&bare, read_wire_not_empty).as_ref(),
            Ok(&stack),
            "{name}"
        );
        assert_eq!(
            read_all(&prefixed, read_wire_with_length),
            Ok(Some(stack)),
            "{name}"
        );
    }
}

#[test]
fn removals_follow_the_values_and_the_empty_stack_is_one_byte() {
    let stack = stack(STONE, 2)
        .set(Damage(7))
        .unset::<RepairCost>()
        .unset::<MaxStackSize>();
    let bare = unhex("02 01 01 02 03 07 01 13");
    let prefixed = unhex("02 01 01 02 03 01 07 01 13");
    assert_eq!(hex(&wire(Some(&stack))), hex(&bare));
    assert_eq!(hex(&wire_with_length(Some(&stack))), hex(&prefixed));
    assert_eq!(read_all(&bare, read_wire), Ok(Some(stack.clone())));
    assert_eq!(read_all(&prefixed, read_wire_with_length), Ok(Some(stack)));

    assert_eq!(wire(None), [0]);
    assert_eq!(wire_with_length(None), [0]);
    assert_eq!(wire(Some(&self::stack(AIR, 1))), [0]);
    assert_eq!(read_all(&[0], read_wire), Ok(None));
    assert_eq!(read_all(&[0], read_wire_with_length), Ok(None));
    assert_eq!(read_all(&unhex("01 00 01 00 03 07"), read_wire), Ok(None));

    let air_with_patch = unhex("01 00 01 00 03 07");
    let mut input = air_with_patch.as_slice();
    assert_eq!(
        read_wire_not_empty(&mut input, &Names),
        Err(WireError::EmptyStack)
    );
    assert!(
        input.is_empty(),
        "the reader reads the patch before the error"
    );
}

#[test]
fn a_repeated_enchantment_on_the_wire_keeps_the_last_level() {
    type Read = fn(&mut &[u8], &Names) -> Result<Option<ItemStack>, WireError>;
    let cases: [(&str, Read); 2] = [
        ("01 01 01 00 0d 02 21 01 21 03", read_wire),
        ("01 01 01 00 0d 05 02 21 01 21 03", read_wire_with_length),
    ];
    let expected = stack(STONE, 1).set(Enchantments::new([level(33, 3)]).expect("one type"));
    for (bytes, read) in cases {
        assert_eq!(
            read_all(&unhex(bytes), read),
            Ok(Some(expected.clone())),
            "{bytes}"
        );
    }
}

#[test]
fn the_count_of_each_wire_form_follows_the_table_of_the_count_rules() {
    let empty = Err(WireError::EmptyStack);
    let over = Err(WireError::CountOutOfRange);
    let cases: [(&str, i32, Result<Option<u8>, WireError>); 8] = [
        ("00", 0, Ok(None)),
        ("ffffffff0f", -1, Ok(None)),
        ("01", 1, Ok(Some(1))),
        ("63", 99, Ok(Some(99))),
        ("64", 100, over),
        ("ff01", 255, over),
        ("8002", 256, over),
        ("ffffffff07", i32::MAX, over),
    ];
    for (count, raw, expected) in cases {
        let bytes = [unhex(count), unhex("01 00 00")].concat();
        let mut input = bytes.as_slice();
        let frame = WireStack::decode(&mut input).expect("a count does not change the bounds");
        let end = if raw > 0 {
            bytes.len()
        } else {
            count.len() / 2
        };
        assert_eq!(frame.bytes(), &bytes[..end], "{count}");
        assert_eq!(frame.count(), raw, "{count}");
        let count_of = |stack: Option<ItemStack>| stack.map(|stack| stack.count.get());
        assert_eq!(frame.read(&Names).map(count_of), expected, "{count}");
        let mut input = bytes.as_slice();
        assert_eq!(
            read_wire(&mut input, &Names).map(count_of),
            expected,
            "{count}"
        );
        let mut input = bytes.as_slice();
        let read = read_wire_with_length(&mut input, &Names).map(count_of);
        assert_eq!(read, expected, "{count}");
        let mut input = bytes.as_slice();
        let not_empty =
            read_wire_not_empty(&mut input, &Names).map(|stack| Some(stack.count.get()));
        let expected = expected.and_then(|count| count.map(Some).ok_or(WireError::EmptyStack));
        assert_eq!(not_empty, expected, "{count}");
    }

    let mut output = vec![0xaa];
    assert_eq!(
        write_wire_not_empty(&stack(AIR, 1), &Names, &mut output),
        empty
    );

    let written = WireStack::write(Some(&stack(STONE, 7)), &Names).expect("stone has a name");
    assert_eq!(written.bytes(), unhex("07 01 00 00"));
    assert_eq!(written.count(), 7);
    assert_eq!(
        written
            .read_not_empty(&Names)
            .map(|stack| stack.count.get()),
        Ok(7)
    );
    let air = WireStack::write(Some(&stack(AIR, 1)), &Names).expect("air has a name");
    assert_eq!(air, WireStack::EMPTY);
}

#[test]
fn the_count_of_an_item_template_in_the_wire_form_is_1_to_99() {
    let bad = Err(WireError::BadValue(BuiltinKey::BundleContents));
    let cases: [(&str, Result<u8, WireError>); 8] = [
        ("00", Err(WireError::EmptyStack)),
        ("ffffffff0f", bad),
        ("01", Ok(1)),
        ("63", Ok(99)),
        ("64", Err(WireError::CountOutOfRange)),
        ("ff01", Err(WireError::CountOutOfRange)),
        ("8002", Err(WireError::CountOutOfRange)),
        ("ffffffff07", Err(WireError::CountOutOfRange)),
    ];
    for (count, expected) in cases {
        let value = [unhex("01 01"), unhex(count), unhex("00 00")].concat();
        let bytes = stone_with("bundle_contents", &value);
        let read = read_all(&bytes, read_wire).map(|stack| {
            let stack = stack.expect("a stone");
            let Some(PatchEntry::Value(Builtin::BundleContents(contents))) = stack.patch().first()
            else {
                panic!("the stone holds a bundle");
            };
            contents.0.first().map(|template| template.count.get())
        });
        assert_eq!(read, expected.map(Some), "{count}");
    }
}

fn saved(snbt: &str) -> NbtCompound {
    parse_compound(snbt).expect("the test input is valid SNBT")
}

#[derive(Clone, Copy, Debug)]
enum Nesting {
    Bundle,
    Container,
}

impl Nesting {
    fn component(self) -> &'static str {
        match self {
            Self::Bundle => "bundle_contents",
            Self::Container => "container",
        }
    }

    fn wire_list(self) -> Vec<u8> {
        match self {
            Self::Bundle => vec![0x01],
            Self::Container => vec![0x01, 0x01],
        }
    }

    fn saved_list(self, item: NbtCompound) -> NbtTag {
        let element = match self {
            Self::Bundle => item,
            Self::Container => {
                let mut slot = NbtCompound::new();
                slot.put("slot", NbtTag::Int(0));
                slot.put("item", NbtTag::Compound(item));
                slot
            }
        };
        NbtTag::List(vec![NbtTag::Compound(element)])
    }

    fn json_list(self, item: &str) -> String {
        match self {
            Self::Bundle => format!("[{item}]"),
            Self::Container => format!(r#"[{{"slot":0,"item":{item}}}]"#),
        }
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Readers {
    All,
    WireOnly,
}

#[derive(Clone, Copy, Debug)]
struct Component {
    kind: &'static str,
    wire: &'static str,
    saved: &'static str,
}

#[derive(Debug)]
struct Chain {
    nesting: Nesting,
    levels: usize,
    innermost: Option<Component>,
}

impl Chain {
    fn wire(&self) -> (Vec<u8>, Vec<u8>) {
        let header = with_one_component(self.nesting.component(), "");
        let list = self.nesting.wire_list();
        let stone = match self.innermost {
            Some(component) => with_one_component(component.kind, component.wire),
            None => unhex("01 01 00 00"),
        };
        let nested = [header.clone(), list.clone()]
            .concat()
            .repeat(self.levels - 1);
        let body = [list, nested, stone].concat();
        let bare = [header.clone(), body.clone()].concat();
        let prefixed = [header, var_int(body.len()), body].concat();
        (bare, prefixed)
    }

    fn innermost_patch(&self) -> String {
        self.innermost
            .map(|component| format!(r#""minecraft:{}":{}"#, component.kind, component.saved))
            .unwrap_or_default()
    }

    fn saved(&self) -> NbtCompound {
        let mut item = saved(&format!(
            r#"{{id:"minecraft:stone",components:{{{}}}}}"#,
            self.innermost_patch()
        ));
        for _ in 0..self.levels {
            let mut components = NbtCompound::new();
            components.put(
                &format!("minecraft:{}", self.nesting.component()),
                self.nesting.saved_list(item),
            );
            item = saved(r#"{id:"minecraft:stone"}"#);
            item.put("components", NbtTag::Compound(components));
        }
        item
    }

    fn json(&self) -> String {
        let mut item = format!(
            r#"{{"id":"minecraft:stone","components":{{{}}}}}"#,
            self.innermost_patch()
        );
        for _ in 0..self.levels {
            item = format!(
                r#"{{"id":"minecraft:stone","components":{{"minecraft:{}":{}}}}}"#,
                self.nesting.component(),
                self.nesting.json_list(&item)
            );
        }
        item
    }
}

#[derive(Clone, Copy, PartialEq)]
enum Expect {
    Valid,
    TooDeep,
}

struct Row {
    chain: Chain,
    readers: Readers,
    expect: Expect,
}

fn read_json(json: &str) -> Result<ItemStack, SavedError> {
    let mut deserializer = serde_json::Deserializer::from_str(json);
    deserializer.disable_recursion_limit();
    let compound = NbtCompound::deserialize(&mut deserializer).expect("the chain is valid JSON");
    read_saved(&compound, &Names)
}

const TEXT_WITH_LIST: Component = Component {
    kind: "custom_name",
    wire: "0a 08 0004 74657874 0001 63 09 0005 6578747261 08 00000001 0001 64 00",
    saved: r#"{"text":"c","extra":["d"]}"#,
};

const TEXT_WITH_WRAPPER: Component = Component {
    kind: "custom_name",
    wire: "0a 08 0004 74657874 0001 61 09 0005 6578747261 0a 00000002 08 0000 0001 62 00 08 0004 74657874 0001 63 01 0004 626f6c64 01 00 00",
    saved: r#"{"text":"a","extra":["b",{"text":"c","bold":true}]}"#,
};

fn depth_rows() -> [Row; 12] {
    let row = |nesting, levels, innermost, readers, expect| Row {
        chain: Chain {
            nesting,
            levels,
            innermost,
        },
        readers,
        expect,
    };
    let max_stack_size = Component {
        kind: "max_stack_size",
        wire: "40",
        saved: "64",
    };
    let lore = Component {
        kind: "lore",
        wire: "00",
        saved: "[]",
    };
    let custom_data = Component {
        kind: "custom_data",
        wire: "0a 0a 0001 61 00 00",
        saved: r#"{"a":{}}"#,
    };
    let mixed_list = Component {
        kind: "custom_data",
        wire: "0a 09 0001 61 0a 00000002 03 0000 000186a0 00 08 0000 0001 78 00 00",
        saved: r#"{"a":[100000,"x"]}"#,
    };
    [
        row(Nesting::Bundle, 170, None, Readers::All, Expect::Valid),
        row(
            Nesting::Bundle,
            170,
            Some(max_stack_size),
            Readers::All,
            Expect::Valid,
        ),
        row(
            Nesting::Bundle,
            170,
            Some(lore),
            Readers::All,
            Expect::TooDeep,
        ),
        row(Nesting::Bundle, 171, None, Readers::All, Expect::TooDeep),
        row(Nesting::Container, 127, None, Readers::All, Expect::Valid),
        row(
            Nesting::Container,
            127,
            Some(custom_data),
            Readers::All,
            Expect::Valid,
        ),
        row(Nesting::Container, 128, None, Readers::All, Expect::TooDeep),
        row(
            Nesting::Container,
            127,
            Some(mixed_list),
            Readers::All,
            Expect::TooDeep,
        ),
        row(
            Nesting::Container,
            127,
            Some(TEXT_WITH_LIST),
            Readers::All,
            Expect::Valid,
        ),
        row(
            Nesting::Container,
            127,
            Some(TEXT_WITH_WRAPPER),
            Readers::All,
            Expect::TooDeep,
        ),
        row(
            Nesting::Bundle,
            100_000,
            None,
            Readers::WireOnly,
            Expect::TooDeep,
        ),
        row(
            Nesting::Container,
            100_000,
            None,
            Readers::WireOnly,
            Expect::TooDeep,
        ),
    ]
}

#[test]
fn each_reader_refuses_a_stack_deeper_than_512_nbt_levels() {
    for Row {
        chain,
        readers,
        expect,
    } in depth_rows()
    {
        let label = format!("{chain:?}");
        let (bare, prefixed) = chain.wire();
        let from_bare = read_all(&bare, read_wire);
        let from_prefixed = read_all(&prefixed, read_wire_with_length);
        let from_saved = (readers == Readers::All).then(|| read_saved(&chain.saved(), &Names));
        let from_json = (readers == Readers::All).then(|| read_json(&chain.json()));
        if expect == Expect::TooDeep {
            assert_eq!(from_bare, Err(WireError::TooDeep), "{label}");
            assert_eq!(from_prefixed, Err(WireError::TooDeep), "{label}");
            for from_saved in [from_saved, from_json].into_iter().flatten() {
                assert_eq!(from_saved, Err(SavedError::TooDeep), "{label}");
            }
            continue;
        }
        let read = from_bare
            .expect("the stack is not too deep")
            .expect("the stack is not air");
        assert_eq!(from_prefixed.as_ref(), Ok(&Some(read.clone())), "{label}");
        assert_eq!(from_saved, Some(Ok(read.clone())), "{label}");
        assert_eq!(from_json, Some(Ok(read.clone())), "{label}");
        assert_eq!(hex(&wire(Some(&read))), hex(&bare), "{label}");
        assert_eq!(
            hex(&wire_with_length(Some(&read))),
            hex(&prefixed),
            "{label}"
        );
        let written = write_saved(&read, &Names).expect("the stack has a saved form");
        assert_eq!(read_saved(&written, &Names).as_ref(), Ok(&read), "{label}");
        assert!(hash(&read).is_ok_and(|hashed| hashed.is_some()), "{label}");

        let deeper = Container::new(vec![Some(read)]).expect("one slot");
        let deeper = stack(STONE, 1).set(deeper);
        assert_eq!(
            write_wire(Some(&deeper), &Names, &mut Vec::new()),
            Err(WireError::TooDeep),
            "{label}"
        );
        assert_eq!(
            write_saved(&deeper, &Names),
            Err(SavedError::TooDeep),
            "{label}"
        );
        assert_eq!(hash(&deeper).err(), Some(HashError::TooDeep), "{label}");
    }

    for bundle_levels in [0, 1] {
        let mut undecoded = saved(r#"{id:"minecraft:stone"}"#);
        let mut components = NbtCompound::new();
        components.put(
            "minecraft:entity_data",
            NbtTag::Compound(nested_compound(600)),
        );
        undecoded.put("components", NbtTag::Compound(components));
        for _ in 0..bundle_levels {
            let mut components = NbtCompound::new();
            components.put(
                "minecraft:bundle_contents",
                Nesting::Bundle.saved_list(undecoded),
            );
            undecoded = saved(r#"{id:"minecraft:stone"}"#);
            undecoded.put("components", NbtTag::Compound(components));
        }
        assert_eq!(
            read_saved(&undecoded, &Names),
            Err(SavedError::TooDeep),
            "{bundle_levels}"
        );
    }

    let custom_data = stack(STONE, 1).set(CustomData(nested_compound(511)));
    assert_eq!(write_saved(&custom_data, &Names), Err(SavedError::TooDeep));
}

fn nested_compound(levels: usize) -> NbtCompound {
    let mut compound = NbtCompound::new();
    for _ in 1..levels {
        let mut outer = NbtCompound::new();
        outer.put("a", NbtTag::Compound(compound));
        compound = outer;
    }
    compound
}

type Read = fn(&mut &[u8], &Names) -> Result<Option<ItemStack>, WireError>;

fn error_reads() -> [(&'static str, Read, WireError); 15] {
    let bare: Read = read_wire;
    let prefixed: Read = read_wire_with_length;
    [
        ("01", bare, WireError::UnexpectedEnd),
        ("01 01 ffffffff0f 00", bare, WireError::Malformed),
        ("01 e807 00 00", bare, WireError::UnknownItem),
        ("01 01 01 00 7a", bare, WireError::UnknownComponentType(122)),
        (
            "01 01 01 00 3c 0a00",
            bare,
            WireError::UnsupportedComponentType(ComponentType(60)),
        ),
        (
            "01 01 01 00 06 63",
            bare,
            WireError::BadValue(BuiltinKey::CustomName),
        ),
        (
            "01 01 02 00 03 07 03 08",
            bare,
            WireError::RepeatedComponentType(ComponentType(3)),
        ),
        (
            "01 01 01 00 03 02 07 00",
            prefixed,
            WireError::BadLength(ComponentType(3)),
        ),
        (
            "01 01 01 00 0c 04",
            bare,
            WireError::BadValue(BuiltinKey::Rarity),
        ),
        (
            "01 01 01 00 0d 01 21 00",
            bare,
            WireError::OutOfRange(BuiltinKey::Enchantments),
        ),
        (
            "01 01 01 00 35 01 01 00 00 00",
            bare,
            WireError::UnknownEntry {
                component: BuiltinKey::PotionContents,
                table: NameTable::Potion,
            },
        ),
        (
            "01 01 01 00 37 01 03 05",
            bare,
            WireError::UnknownEntry {
                component: BuiltinKey::SuspiciousStewEffects,
                table: NameTable::MobEffect,
            },
        ),
        (
            "01 01 01 00 0d 03 01 09 01",
            prefixed,
            WireError::UnknownEntry {
                component: BuiltinKey::Enchantments,
                table: NameTable::Enchantment,
            },
        ),
        (
            "01 01 01 00 47 8002 00",
            bare,
            WireError::OutOfRange(BuiltinKey::Fireworks),
        ),
        (
            "01 01 01 00 6d 00",
            bare,
            WireError::BadValue(BuiltinKey::PaintingVariant),
        ),
    ]
}

#[test]
fn each_error_names_the_rule_that_the_input_breaks() {
    for (bytes, read, expected) in error_reads() {
        let bytes = unhex(bytes);
        assert_eq!(
            read(&mut bytes.as_slice(), &Names),
            Err(expected),
            "{expected:?}"
        );
    }

    let with_entity_data = saved(r#"{id:"stone",components:{"minecraft:entity_data":{x:1}}}"#);
    let writes: [(ItemStack, WireError); 2] = [
        (stack(999, 1), WireError::UnnamedId(NameTable::Item)),
        (
            read_saved(&with_entity_data, &Names).expect("a saved stack with entity data"),
            WireError::SavedValueOnly,
        ),
    ];
    for (stack, expected) in writes {
        let written = write_wire(Some(&stack), &Names, &mut Vec::new());
        assert_eq!(written, Err(expected), "{expected:?}");
    }
    let damaged = stack(STONE, 1).set(Damage(1));
    assert_eq!(
        write_wire(Some(&damaged), &ItemsOnly, &mut Vec::new()),
        Err(WireError::UnregisteredType)
    );

    let wire_only = read_all(&unhex("01 01 01 00 3c 02 0a00"), read_wire_with_length)
        .expect("entity data with a length prefix")
        .expect("a stone");
    assert_eq!(
        write_saved(&wire_only, &Names),
        Err(SavedError::WireValueOnly(ComponentType(60)))
    );
}

#[derive(Deserialize)]
struct PlainGolden {
    values: Vec<PlainValue>,
}

#[derive(Deserialize)]
struct PlainValue {
    kind: String,
    nbt: Option<String>,
    wire: Option<String>,
}

const DECODED_PLAIN: [&str; 32] = [
    "max_stack_size",
    "max_damage",
    "damage",
    "unbreakable",
    "repair_cost",
    "rarity",
    "attack_animation",
    "interact_animation",
    "map_id",
    "map_decorations",
    "profile",
    "dyed_color",
    "custom_model_data",
    "enchantment_glint_override",
    "block_state",
    "lodestone_tracker",
    "trim",
    "potion_contents",
    "custom_data",
    "suspicious_stew_effects",
    "ominous_bottle_amplifier",
    "instrument",
    "charged_projectiles",
    "bundle_contents",
    "firework_explosion",
    "fireworks",
    "banner_patterns",
    "container",
    "bees",
    "container_loot",
    "stored_enchantments",
    "painting/variant",
];

fn assert_same_component(kind: &str, written: &[u8], expected: &[u8], header: usize) {
    if kind != "map_decorations" {
        assert_eq!(hex(written), hex(expected), "{kind}");
        return;
    }
    let tree = |bytes: &[u8]| {
        let mut reader = NbtReadHelper::new(Cursor::new(bytes));
        let tag = NbtTag::deserialize(&mut reader).expect("the component is one NBT value");
        assert_eq!(
            reader.position().expect("a cursor knows its position"),
            u64::try_from(bytes.len()).expect("a buffer length fits in 64 bits"),
            "{kind}: the component has no bytes after its NBT value"
        );
        tag
    };
    let (written_header, written_tree) = written.split_at(header);
    let (expected_header, expected_tree) = expected.split_at(header);
    assert_eq!(hex(written_header), hex(expected_header), "{kind}");
    assert_eq!(written_tree.len(), expected_tree.len(), "{kind}");
    assert!(
        same(&tree(written_tree), &tree(expected_tree)),
        "{kind}: wrote {}, expected {}",
        hex(written_tree),
        hex(expected_tree)
    );
}

#[test]
fn the_wire_components_that_the_game_wrote_read_to_the_saved_value_and_write_back() {
    let golden: PlainGolden =
        serde_json::from_str(&fixture("item_plain").text()).expect("the golden is JSON");
    let mut decoded = 0;
    let mut kept = 0;
    for value in &golden.values {
        let Some(type_id) = COMPONENT_TYPES.iter().position(|path| *path == value.kind) else {
            continue;
        };
        let (Some(nbt), Some(component)) = (&value.nbt, &value.wire) else {
            continue;
        };
        let type_id = u8::try_from(type_id).expect("the type ids are below 128");
        let component = unhex(component);
        let header = [unhex("01 01 01 00"), vec![type_id]].concat();
        let length = var_int(component.len());
        let prefixed = [header.clone(), length.clone(), component.clone()].concat();
        let read = read_all(&prefixed, read_wire_with_length).expect("the game wrote it");
        assert_same_component(
            &value.kind,
            &wire_with_length(read.as_ref()),
            &prefixed,
            header.len() + length.len(),
        );
        if !DECODED_PLAIN.contains(&value.kind.as_str()) {
            kept += 1;
            continue;
        }

        let bare = [header.clone(), component].concat();
        let tag = NbtTag::deserialize(&mut NbtReadHelper::new(Cursor::new(unhex(nbt))))
            .expect("the golden holds one NBT value");
        let mut patch = NbtCompound::new();
        patch.put(&format!("minecraft:{}", value.kind), tag);
        let mut saved = saved(r#"{id:"minecraft:stone"}"#);
        saved.put("components", NbtTag::Compound(patch));
        let from_saved = read_saved(&saved, &Names).expect("the game saved a valid component");
        if write_wire(Some(&from_saved), &Names, &mut Vec::new())
            == Err(WireError::OutOfRange(BuiltinKey::LodestoneTracker))
        {
            assert_ne!(
                read,
                Some(from_saved),
                "STK-SHP-039: the game wrote low bits"
            );
            decoded += 1;
            continue;
        }

        assert_eq!(
            read_all(&bare, read_wire),
            Ok(Some(from_saved.clone())),
            "{}",
            value.kind
        );
        assert_eq!(read, Some(from_saved), "{}", value.kind);
        assert_same_component(&value.kind, &wire(read.as_ref()), &bare, header.len());
        decoded += 1;
    }
    assert_eq!((decoded, kept), (113, 73));
}

#[derive(Deserialize)]
struct DecodeGolden {
    decodes: Vec<PlainDecode>,
    decode_errors: Vec<PlainDecodeError>,
}

#[derive(Deserialize)]
struct PlainDecode {
    kind: String,
    wire: String,
    json: String,
}

#[derive(Deserialize)]
struct PlainDecodeError {
    kind: String,
    wire: String,
}

fn with_one_component(kind: &str, component: &str) -> Vec<u8> {
    let type_id = COMPONENT_TYPES
        .iter()
        .position(|path| *path == kind)
        .expect("the kind has a type id");
    let type_id = u8::try_from(type_id).expect("the type ids are below 128");
    [unhex("01 01 01 00"), vec![type_id], unhex(component)].concat()
}

const TYPED_KINDS: [(&str, BuiltinKey); 9] = [
    ("profile", BuiltinKey::Profile),
    ("potion_contents", BuiltinKey::PotionContents),
    ("container", BuiltinKey::Container),
    ("bees", BuiltinKey::Bees),
    ("charged_projectiles", BuiltinKey::ChargedProjectiles),
    ("fireworks", BuiltinKey::Fireworks),
    ("bundle_contents", BuiltinKey::BundleContents),
    ("stored_enchantments", BuiltinKey::StoredEnchantments),
    ("painting/variant", BuiltinKey::PaintingVariant),
];

fn typed_kind(kind: &str) -> Option<BuiltinKey> {
    TYPED_KINDS
        .iter()
        .find(|(path, _)| *path == kind)
        .map(|(_, key)| *key)
}

#[test]
fn a_container_drops_the_empty_slots_at_its_end() {
    let cases = [
        ("02 01 8808 01 00 00 00", "01 01 8808 01 00 00"),
        (
            "02 01 8808 01 00 00 01 8808 01 00 00",
            "02 01 8808 01 00 00 01 8808 01 00 00",
        ),
        ("03 00 01 8808 01 00 00 00", "02 00 01 8808 01 00 00"),
        ("02 00 00", "00"),
    ];
    for (input, expected) in cases {
        let read = read_all(&with_one_component("container", input), read_wire)
            .expect("a valid container")
            .expect("a stone");
        assert_eq!(
            hex(&wire(Some(&read))),
            hex(&with_one_component("container", expected)),
            "{input}"
        );
    }
}

#[test]
fn the_own_wire_forms_that_the_game_decoded_read_to_its_saved_form_and_write_back() {
    let golden: DecodeGolden =
        serde_json::from_str(&fixture("item_plain").text()).expect("the golden is JSON");

    let mut decoded = 0;
    for case in &golden.decodes {
        if typed_kind(&case.kind).is_none() {
            continue;
        }
        let bare = with_one_component(&case.kind, &case.wire);
        let read = read_all(&bare, read_wire)
            .expect("the game decoded it")
            .expect("a stone");
        let from_json = read_saved(
            &saved(&format!(
                r#"{{id:"minecraft:stone",components:{{"minecraft:{}":{}}}}}"#,
                case.kind, case.json
            )),
            &Names,
        )
        .expect("the game wrote a valid saved form");
        let written_back = if read == from_json {
            wire(Some(&from_json))
        } else {
            bare
        };
        assert_eq!(hex(&wire(Some(&read))), hex(&written_back), "{}", case.wire);
        let written = write_saved(&read, &Names).expect("the component has a saved form");
        assert_eq!(read_saved(&written, &Names), Ok(from_json), "{}", case.wire);
        decoded += 1;
    }
    assert_eq!(decoded, 8);

    let mut rejected = 0;
    for case in &golden.decode_errors {
        let Some(key) = typed_kind(&case.kind) else {
            continue;
        };
        let expected = match key {
            BuiltinKey::Container | BuiltinKey::ChargedProjectiles | BuiltinKey::Fireworks => {
                WireError::OutOfRange(key)
            }
            BuiltinKey::BundleContents => WireError::EmptyStack,
            BuiltinKey::StoredEnchantments => WireError::UnknownEntry {
                component: key,
                table: NameTable::Enchantment,
            },
            BuiltinKey::PaintingVariant => WireError::UnknownEntry {
                component: key,
                table: NameTable::PaintingVariant,
            },
            _ => WireError::BadValue(key),
        };
        let bare = with_one_component(&case.kind, &case.wire);
        assert_eq!(read_all(&bare, read_wire), Err(expected), "{}", case.wire);
        rejected += 1;
    }
    assert_eq!(rejected, 8);
}

#[test]
fn the_equippable_components_that_the_game_wrote_read_and_write_back_in_the_saved_and_the_wire_form()
 {
    let golden = fixture("item_registry_refs").text();
    let mut is_equippable = false;
    let mut nbt: Option<NbtTag> = None;
    let mut checked = 0;
    for line in golden.lines() {
        if let Some(sample) = line.strip_prefix("sample minecraft:") {
            is_equippable = sample.starts_with("equippable ");
            nbt = None;
        } else if !is_equippable {
            continue;
        } else if let Some(bytes) = line.strip_prefix("  nbt ") {
            let bytes = unhex(bytes);
            nbt = Some(
                NbtTag::deserialize(&mut NbtReadHelper::new(Cursor::new(bytes)))
                    .expect("the golden holds one NBT value"),
            );
        } else if let Some(component) = line.strip_prefix("  wire ") {
            let nbt = nbt
                .take()
                .expect("the sample has its NBT before its wire form");
            let bare = with_one_component("equippable", component);
            let read = read_all(&bare, read_wire).expect("the game wrote it");
            assert_eq!(hex(&wire(read.as_ref())), hex(&bare), "{component}");

            let mut patch = NbtCompound::new();
            patch.put("minecraft:equippable", nbt);
            let mut stone = saved(r#"{id:"minecraft:stone"}"#);
            stone.put("components", NbtTag::Compound(patch.clone()));
            let from_saved = read_saved(&stone, &Names).expect("the game saved a valid component");
            assert_eq!(read, Some(from_saved.clone()), "{component}");

            let written = write_saved(&from_saved, &Names).expect("the component has a saved form");
            assert!(
                written
                    .get("components")
                    .is_some_and(|written| same(written, &NbtTag::Compound(patch))),
                "{component}"
            );
            checked += 1;
        }
    }
    assert_eq!(checked, 5);
}

#[test]
fn the_records_and_holders_that_the_game_wrote_read_and_write_back_in_the_saved_and_the_wire_form()
{
    for golden in golden_cases(GoldenSet::Changed) {
        let case = golden.case;
        let kind = golden.kind;
        let nbt = NbtTag::deserialize(&mut NbtReadHelper::new(Cursor::new(unhex(&golden.nbt))))
            .expect("the golden holds one NBT value");
        let bare = with_one_component(kind, &golden.wire);
        let read = read_all(&bare, read_wire).expect("the game wrote it");
        assert_eq!(hex(&wire(read.as_ref())), hex(&bare), "{case}");

        let mut patch = NbtCompound::new();
        patch.put(&format!("minecraft:{kind}"), nbt);
        let mut stone = saved(r#"{id:"minecraft:stone"}"#);
        stone.put("components", NbtTag::Compound(patch.clone()));
        let from_saved = read_saved(&stone, &Names).expect("the game saved a valid component");
        assert_eq!(read, Some(from_saved.clone()), "{case}");

        let written = write_saved(&from_saved, &Names).expect("the component has a saved form");
        assert!(
            written
                .get("components")
                .is_some_and(|written| same(written, &NbtTag::Compound(patch))),
            "{case}"
        );
    }
}

#[test]
fn a_partial_profile_with_a_name_and_a_uuid_differs_from_the_full_profile_it_saves_as() {
    let partial = read_all(
        &with_one_component(
            "profile",
            "00 01 05 5374657665 01 00000001 00000002 00000003 00000004 00 00 00 00 00",
        ),
        read_wire,
    )
    .expect("the stack is valid")
    .expect("a stone");
    let full = read_saved(
        &saved(r#"{id:"minecraft:stone",components:{"minecraft:profile":{id:[I;1,2,3,4],name:"Steve"}}}"#),
        &Names,
    )
    .expect("the profile is valid");

    assert_ne!(partial, full);
    let written = write_saved(&partial, &Names).expect("the profile has a saved form");
    assert_eq!(read_saved(&written, &Names), Ok(full));
}

#[derive(Clone, Copy)]
enum Form {
    Bare,
    LengthPrefixed,
}

#[test]
fn the_stacks_of_the_inventory_packets_read_and_write_back() {
    let golden = fixture("inventory_packets").text();
    let packet = |name: &str| -> Vec<u8> {
        let line = golden
            .lines()
            .find_map(|line| line.strip_prefix(name)?.strip_prefix(' '))
            .expect("the golden has the packet");
        unhex(line)
    };
    let packets: [(&str, usize, usize, Form); 8] = [
        ("container_set_slot", 4, 1, Form::Bare),
        ("container_set_slot_empty", 4, 1, Form::Bare),
        ("container_set_content", 3, 47, Form::Bare),
        ("container_set_content_chest", 3, 64, Form::Bare),
        ("set_cursor_item", 0, 1, Form::Bare),
        ("set_player_inventory", 1, 1, Form::Bare),
        ("set_creative_mode_slot", 2, 1, Form::LengthPrefixed),
        ("set_creative_mode_slot_empty", 2, 1, Form::LengthPrefixed),
    ];
    let mut decoded = 0;
    for (name, header, stacks, form) in packets {
        let bytes = packet(name);
        let mut input = bytes.get(header..).expect("the packet has its header");
        for _ in 0..stacks {
            let before = input;
            let read = match form {
                Form::Bare => read_wire(&mut input, &Names),
                Form::LengthPrefixed => read_wire_with_length(&mut input, &Names),
            };
            let stack = read.unwrap_or_else(|error| panic!("{name}: {error}"));
            let consumed = before.len() - input.len();
            let (written, reread) = match form {
                Form::Bare => {
                    let written = wire(stack.as_ref());
                    let reread = read_all(&written, read_wire);
                    (written, reread)
                }
                Form::LengthPrefixed => {
                    let written = wire_with_length(stack.as_ref());
                    let reread = read_all(&written, read_wire_with_length);
                    (written, reread)
                }
            };
            let named = stack.as_ref().is_some_and(|stack| {
                stack
                    .patch()
                    .iter()
                    .any(|entry| matches!(entry, PatchEntry::Value(Builtin::CustomName(_))))
            });
            if name == "set_creative_mode_slot" || named {
                assert_eq!(written.len(), consumed, "{name}");
                assert_eq!(reread, Ok(stack), "{name}");
            } else {
                assert_eq!(hex(&written), hex(&before[..consumed]), "{name}");
            }
            decoded += 1;
        }
    }
    assert_eq!(decoded, 117);
}

fn hash(stack: &ItemStack) -> Result<Option<HashedStack>, HashError> {
    hash_stack(Some(stack), &Names, &mut HashScratch::default())
}

#[test]
fn a_lore_has_at_most_256_lines_in_each_form() {
    for (count, length) in [(256, "8002"), (257, "8102")] {
        let stack = stack(STONE, 1).set(Lore {
            lines: vec![Text::text("a"); count].into(),
        });
        let lines = vec![r#""a""#; count].join(",");
        let saved = saved(&format!(
            r#"{{id:"stone",components:{{"minecraft:lore":[{lines}]}}}}"#
        ));
        let mut wire = unhex(&format!("01 01 01 00 0b {length}"));
        for _ in 0..count {
            wire.extend(unhex("08 0001 61"));
        }

        let fits = count <= 256;
        let saved_error = (!fits).then_some(SavedError::OutOfRange(BuiltinKey::Lore));
        let wire_error = (!fits).then_some(WireError::OutOfRange(BuiltinKey::Lore));
        let hash_error = (!fits).then_some(HashError::OutOfRange(BuiltinKey::Lore));
        assert_eq!(read_saved(&saved, &Names).err(), saved_error, "{count}");
        assert_eq!(write_saved(&stack, &Names).err(), saved_error, "{count}");
        assert_eq!(read_all(&wire, read_wire).err(), wire_error, "{count}");
        let written = write_wire(Some(&stack), &Names, &mut Vec::new());
        assert_eq!(written.err(), wire_error, "{count}");
        assert_eq!(hash(&stack).err(), hash_error, "{count}");
    }
}

#[test]
fn a_writer_measures_the_depth_past_a_component_that_only_the_wire_form_holds() {
    let wire_only = read_all(&unhex("01 01 01 00 3c 02 0a00"), read_wire_with_length)
        .expect("entity data with a length prefix")
        .expect("a stone");
    let deepest = Chain {
        nesting: Nesting::Container,
        levels: 127,
        innermost: None,
    };
    let deepest = read_all(&deepest.wire().0, read_wire)
        .expect("a stack of 512 levels")
        .expect("a stone");
    let slots = Container::new(vec![Some(wire_only), Some(deepest)]).expect("two slots");
    let outer = stack(STONE, 1).set(slots);
    assert_eq!(
        write_wire(Some(&outer), &Names, &mut Vec::new()),
        Err(WireError::TooDeep)
    );
}

#[test]
fn a_named_colour_equals_its_rgb_value_and_keeps_its_spelling() {
    let named = stack(STONE, 1).set(CustomName(Text::text("x").color(Color::RED)));
    let rgb = Text::text("x").color(Color::rgb(0xff, 0x55, 0x55));
    let rgb = stack(STONE, 1).set(CustomName(rgb));
    assert_eq!(named, rgb);
    assert_ne!(hash(&named), hash(&rgb));

    for (stack, spelling) in [(&named, "red"), (&rgb, "#FF5555")] {
        let written = write_saved(stack, &Names).expect("the name has a saved form");
        let Some(NbtTag::Compound(patch)) = written.get("components") else {
            panic!("the stack has a patch");
        };
        let Some(NbtTag::Compound(name)) = patch.get("minecraft:custom_name") else {
            panic!("the name is a map");
        };
        let color = name.get("color");
        assert_eq!(color, Some(&NbtTag::String(spelling.into())));

        let from_saved = read_saved(&written, &Names).expect("the written form reads");
        let from_wire = read_all(&wire(Some(stack)), read_wire)
            .expect("the written form reads")
            .expect("the stack is not empty");
        assert_eq!(hash(&from_saved), hash(stack), "{spelling}");
        assert_eq!(hash(&from_wire), hash(stack), "{spelling}");
    }
}

fn stone_with(kind: &str, value: &[u8]) -> Vec<u8> {
    let type_id = COMPONENT_TYPES
        .iter()
        .position(|path| *path == kind)
        .expect("the kind has a type id");
    let type_id = u8::try_from(type_id).expect("the type ids are below 128");
    [unhex("01 01 01 00"), vec![type_id], value.to_vec()].concat()
}

fn byte_array_under_a(length: usize) -> Vec<u8> {
    let declared = i32::try_from(length).expect("the test lengths fit in 32 bits");
    let mut nbt = vec![10, 7, 0, 1, b'a'];
    nbt.extend_from_slice(&declared.to_be_bytes());
    nbt.resize(nbt.len() + length, 0);
    nbt.push(0);
    stone_with("custom_data", &nbt)
}

fn container_of_two(length: usize) -> Vec<u8> {
    let template = [unhex("01 8808 01"), byte_array_under_a(length).split_off(2)].concat();
    stone_with("container", &[vec![2], template.clone(), template].concat())
}

#[test]
fn one_wire_stack_shares_one_nbt_budget_with_the_stacks_inside_it() {
    let cases = [
        (byte_array_under_a(2_097_148), true),
        (byte_array_under_a(2_097_149), false),
        (container_of_two(1_048_572), true),
        (container_of_two(1_048_573), false),
    ];
    let budget = NbtBudget {
        bytes: 2_097_152,
        tags: 262_144,
    };
    for (index, (bytes, passes)) in cases.iter().enumerate() {
        let read = with_nbt_budget(budget, || read_all(bytes, read_wire));
        if *passes {
            assert!(matches!(read, Ok(Ok(Some(_)))), "{index}");
        } else {
            assert_eq!(read, Err(NbtQuotaExceeded), "{index}");
        }
    }
}

fn decode_with_length(bytes: &[u8]) -> Result<Vec<u8>, WireError> {
    let mut input = bytes;
    let frame = WireStackWithLength::decode(&mut input).map_err(|error| match error.kind() {
        ErrorKind::UnexpectedEnd => WireError::UnexpectedEnd,
        ErrorKind::Other(other) => *other
            .downcast_ref::<WireError>()
            .expect("the frame gives a wire error"),
        other => panic!("the frame gives {other:?}"),
    })?;
    assert!(input.is_empty(), "{} bytes are left", input.len());
    Ok(frame.bytes().to_vec())
}

#[test]
fn a_length_prefixed_frame_checks_each_value_against_the_shape_of_its_type() {
    let charge = ComponentType(7);
    let cases = [
        (
            with_one_component("minimum_attack_charge", "04 3f800000"),
            None,
        ),
        (
            with_one_component("minimum_attack_charge", "05 3f800000 00"),
            Some(WireError::BadLength(charge)),
        ),
        (
            with_one_component("minimum_attack_charge", "03 3f8000"),
            Some(WireError::BadLength(charge)),
        ),
        (
            with_one_component("damage", "01 8001"),
            Some(WireError::BadLength(ComponentType(3))),
        ),
        (
            with_one_component("use_effects", "06 02 00 3f800000"),
            Some(WireError::BadShape(ComponentType(5))),
        ),
        (
            unhex("01 01 01 00 7a 00"),
            Some(WireError::UnknownComponentType(122)),
        ),
    ];
    for (index, (bytes, error)) in cases.iter().enumerate() {
        let expected = error.map_or_else(|| Ok(bytes.clone()), Err);
        assert_eq!(decode_with_length(bytes), expected, "{index}");
    }
}

#[test]
fn a_length_prefixed_frame_charges_its_values_to_the_nbt_budget_in_force() {
    let mut nbt = vec![10, 7, 0, 1, b'a', 0, 0, 0, 100];
    nbt.resize(nbt.len() + 100, 0);
    nbt.push(0);
    let length = u8::try_from(nbt.len()).expect("the value is shorter than 128 bytes");
    let bytes = stone_with("custom_data", &[vec![length], nbt].concat());
    let cases = [(1_024, true), (64, false)];
    for (limit, passes) in cases {
        let budget = NbtBudget {
            bytes: limit,
            tags: 262_144,
        };
        let decoded = with_nbt_budget(budget, || decode_with_length(&bytes));
        if passes {
            assert_eq!(decoded, Ok(Ok(bytes.clone())), "{limit}");
        } else {
            assert_eq!(decoded, Err(NbtQuotaExceeded), "{limit}");
        }
    }
}

#[test]
fn swing_animation_rejects_invalid_kind_and_negative_duration_on_wire() {
    let invalid_kind = with_one_component("attack_animation", "0306");
    assert_eq!(
        read_all(&invalid_kind, read_wire),
        Err(WireError::BadValue(BuiltinKey::AttackAnimation))
    );
    let negative_wire = with_one_component("attack_animation", "01ffffffff0f");
    assert_eq!(
        read_all(&negative_wire, read_wire),
        Err(WireError::OutOfRange(BuiltinKey::AttackAnimation))
    );
    let air_negative = unhex("01 00 01 00 28 01 ffffffff0f");
    assert_eq!(read_all(&air_negative, read_wire), Ok(None));
    let negative_stack = stack(STONE, 1).set(AttackAnimation(SwingAnimation {
        kind: SwingKind::Whack,
        duration: -1,
    }));
    assert_eq!(
        write_wire(Some(&negative_stack), &Names, &mut Vec::new()),
        Err(WireError::OutOfRange(BuiltinKey::AttackAnimation))
    );
}
