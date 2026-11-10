use std::io::Cursor;
use std::num::NonZeroU8;

use mcrs_minecraft_fixtures::fixture;
use mcrs_minecraft_item_stack::AttributeId;
use mcrs_minecraft_item_stack::AttributeModifier;
use mcrs_minecraft_item_stack::AttributeModifiers;
use mcrs_minecraft_item_stack::BlockId;
use mcrs_minecraft_item_stack::BlockSet;
use mcrs_minecraft_item_stack::BlockTagId;
use mcrs_minecraft_item_stack::Builtin;
use mcrs_minecraft_item_stack::BuiltinKey;
use mcrs_minecraft_item_stack::ComponentType;
use mcrs_minecraft_item_stack::Count;
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
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::Lore;
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
use mcrs_minecraft_item_stack::Tool;
use mcrs_minecraft_item_stack::ToolRule;
use mcrs_minecraft_item_stack::TooltipDisplay;
use mcrs_minecraft_item_stack::Unbreakable;
use mcrs_minecraft_item_stack::WireError;
use mcrs_minecraft_item_stack::hash_stack;
use mcrs_minecraft_item_stack::read_saved;
use mcrs_minecraft_item_stack::read_wire;
use mcrs_minecraft_item_stack::read_wire_not_empty;
use mcrs_minecraft_item_stack::read_wire_with_length;
use mcrs_minecraft_item_stack::write_saved;
use mcrs_minecraft_item_stack::write_wire;
use mcrs_minecraft_item_stack::write_wire_not_empty;
use mcrs_minecraft_item_stack::write_wire_with_length;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::deserializer::NbtReadHelper;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_snbt::parse_compound;
use mcrs_minecraft_text::Color;
use mcrs_minecraft_text::IntoText;
use mcrs_minecraft_text::Text;
use serde::Deserialize;

const AIR: u16 = 0;
const STONE: u16 = 1;

/// The type ids of the specification, from 0 to 28.
const COMPONENT_TYPES: [&str; 29] = [
    "custom_data",
    "max_stack_size",
    "max_damage",
    "damage",
    "unbreakable",
    "use_effects",
    "custom_name",
    "minimum_attack_charge",
    "damage_type",
    "item_name",
    "item_model",
    "lore",
    "rarity",
    "enchantments",
    "can_place_on",
    "can_break",
    "attribute_modifiers",
    "custom_model_data",
    "tooltip_display",
    "repair_cost",
    "creative_slot_lock",
    "enchantment_glint_override",
    "intangible_projectile",
    "food",
    "consumable",
    "use_remainder",
    "use_cooldown",
    "damage_resistant",
    "tool",
];

/// The item ids of the golden `inventory_packets`.
const ITEMS: [(u16, &str); 6] = [
    (0, "air"),
    (1, "stone"),
    (1030, "apple"),
    (1035, "diamond"),
    (1036, "emerald"),
    (1073, "diamond_sword"),
];

/// Each name is in the namespace `minecraft`.
struct Names;

impl Names {
    fn entries(table: NameTable) -> Vec<(u16, &'static str)> {
        let indexed = |paths: &[&'static str]| -> Vec<(u16, &'static str)> {
            (0..).zip(paths.iter().copied()).collect()
        };
        match table {
            NameTable::Item => ITEMS.to_vec(),
            NameTable::Enchantment => indexed(&["sharpness", "unbreaking"]),
            NameTable::Attribute => indexed(&["attack_damage"]),
            NameTable::Block => indexed(&["stone", "dirt"]),
            NameTable::BlockTag => indexed(&["mineable/pickaxe"]),
            NameTable::ComponentType => indexed(&COMPONENT_TYPES),
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

/// A table with the items only.
struct ItemsOnly;

impl NameLookup for ItemsOnly {
    fn id(&self, table: NameTable, name: ResourceLocation<'_>) -> Option<u16> {
        (table == NameTable::Item).then(|| Names.id(table, name))?
    }

    fn name(&self, table: NameTable, id: u16) -> Option<ResourceLocation<'_>> {
        (table == NameTable::Item).then(|| Names.name(table, id))?
    }
}

fn stack(kind: u16, count: u8) -> ItemStack {
    let count = Count::new(count).expect("the test counts are not zero");
    ItemStack::new(Kind(kind), count)
}

fn unhex(hex: &str) -> Vec<u8> {
    let digits: Vec<char> = hex.chars().filter(|digit| !digit.is_whitespace()).collect();
    digits
        .chunks(2)
        .map(|pair| {
            let pair: String = pair.iter().collect();
            u8::from_str_radix(&pair, 16).expect("the test holds hex digits")
        })
        .collect()
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

/// Reads the whole input with `read`.
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

/// A stack of one stone with one patch entry, and the bytes of that entry: the type id, then
/// the wire form of the component as the tables of the specification state it.
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
            stone().set(Enchantments::new([level(0, 5), level(1, 255)]).expect("two types")),
            "0d",
            "02 00 05 01 ff01",
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
        ("01 01 01 00 0d 02 00 01 00 03", read_wire),
        ("01 01 01 00 0d 05 02 00 01 00 03", read_wire_with_length),
    ];
    let expected = stack(STONE, 1).set(Enchantments::new([level(0, 3)]).expect("one type"));
    for (bytes, read) in cases {
        assert_eq!(
            read_all(&unhex(bytes), read),
            Ok(Some(expected.clone())),
            "{bytes}"
        );
    }
}

#[test]
fn the_count_is_8_bits_and_the_length_prefixed_form_holds_at_most_99() {
    let empty = Err(WireError::EmptyStack);
    let cases: [(
        &str,
        Result<Option<u8>, WireError>,
        Result<Option<u8>, WireError>,
    ); 7] = [
        ("00", Ok(None), Ok(None)),
        ("ffffffff0f", Ok(None), Ok(None)),
        ("01", Ok(Some(1)), Ok(Some(1))),
        ("63", Ok(Some(99)), Ok(Some(99))),
        ("64", Ok(Some(100)), Err(WireError::CountOutOfRange)),
        ("ff01", Ok(Some(255)), Err(WireError::CountOutOfRange)),
        (
            "8002",
            Err(WireError::CountTooLarge),
            Err(WireError::CountTooLarge),
        ),
    ];
    for (count, bare, prefixed) in cases {
        let bytes = [unhex(count), unhex("01 00 00")].concat();
        let count_of = |stack: Option<ItemStack>| stack.map(|stack| stack.count.get());
        let mut input = bytes.as_slice();
        assert_eq!(read_wire(&mut input, &Names).map(count_of), bare, "{count}");
        let mut input = bytes.as_slice();
        let read = read_wire_with_length(&mut input, &Names).map(count_of);
        assert_eq!(read, prefixed, "{count}");
        let mut input = bytes.as_slice();
        let not_empty =
            read_wire_not_empty(&mut input, &Names).map(|stack| Some(stack.count.get()));
        let expected = bare.and_then(|count| count.map(Some).ok_or(WireError::EmptyStack));
        assert_eq!(not_empty, expected, "{count}");
    }

    let mut output = vec![0xaa];
    let over = stack(STONE, 100);
    assert_eq!(
        write_wire(Some(&over), &Names, &mut output),
        Err(WireError::CountOutOfRange)
    );
    assert_eq!(output, [0xaa], "a failed write leaves the output as it was");
    assert_eq!(
        write_wire_not_empty(&stack(AIR, 1), &Names, &mut output),
        empty
    );
}

fn saved(snbt: &str) -> NbtCompound {
    parse_compound(snbt).expect("the test input is valid SNBT")
}

#[test]
fn each_error_names_the_rule_that_the_input_breaks() {
    type Read = fn(&mut &[u8], &Names) -> Result<Option<ItemStack>, WireError>;
    let bare: Read = read_wire;
    let prefixed: Read = read_wire_with_length;
    let reads: [(&str, Read, WireError); 11] = [
        ("01", bare, WireError::UnexpectedEnd),
        ("01 01 ffffffff0f 00", bare, WireError::Malformed),
        ("01 e807 00 00", bare, WireError::UnknownItem),
        ("01 01 01 00 7a", bare, WireError::UnknownComponentType(122)),
        (
            "01 01 01 00 00 0a00",
            bare,
            WireError::UnsupportedComponentType(ComponentType(0)),
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
            "01 01 01 00 0d 01 00 00",
            bare,
            WireError::OutOfRange(BuiltinKey::Enchantments),
        ),
        (
            "01 01 01 00 0d 03 01 09 01",
            prefixed,
            WireError::UnknownEntry {
                component: BuiltinKey::Enchantments,
                table: NameTable::Enchantment,
            },
        ),
    ];
    for (bytes, read, expected) in reads {
        let bytes = unhex(bytes);
        assert_eq!(
            read(&mut bytes.as_slice(), &Names),
            Err(expected),
            "{expected:?}"
        );
    }

    let with_custom_data = saved(r#"{id:"stone",components:{"minecraft:custom_data":{x:1}}}"#);
    let writes: [(ItemStack, WireError); 2] = [
        (stack(999, 1), WireError::UnnamedId(NameTable::Item)),
        (
            read_saved(&with_custom_data, &Names).expect("a saved stack with custom data"),
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

    let wire_only = read_all(&unhex("01 01 01 00 00 02 0a00"), read_wire_with_length)
        .expect("custom data with a length prefix")
        .expect("a stone");
    assert_eq!(
        write_saved(&wire_only, &Names),
        Err(SavedError::WireValueOnly(ComponentType(0)))
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

/// The plain types of the golden `item_plain` that this crate decodes.
const DECODED_PLAIN: [&str; 6] = [
    "max_stack_size",
    "max_damage",
    "damage",
    "unbreakable",
    "repair_cost",
    "rarity",
];

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
        let length = u8::try_from(component.len()).expect("the golden components are short");
        let prefixed = [
            unhex("01 01 01 00"),
            vec![type_id, length],
            component.clone(),
        ]
        .concat();
        let read = read_all(&prefixed, read_wire_with_length).expect("the game wrote it");
        assert_eq!(
            hex(&wire_with_length(read.as_ref())),
            hex(&prefixed),
            "{}",
            value.kind
        );
        if !DECODED_PLAIN.contains(&value.kind.as_str()) {
            kept += 1;
            continue;
        }

        let bare = [unhex("01 01 01 00"), vec![type_id], component].concat();
        let tag = NbtTag::deserialize(&mut NbtReadHelper::new(Cursor::new(unhex(nbt))))
            .expect("the golden holds one NBT value");
        let mut patch = NbtCompound::new();
        patch.put(&format!("minecraft:{}", value.kind), tag);
        let mut saved = saved(r#"{id:"minecraft:stone"}"#);
        saved.put("components", NbtTag::Compound(patch));
        let from_saved = read_saved(&saved, &Names).expect("the game saved a valid component");

        assert_eq!(
            read_all(&bare, read_wire),
            Ok(Some(from_saved.clone())),
            "{}",
            value.kind
        );
        assert_eq!(read, Some(from_saved), "{}", value.kind);
        assert_eq!(hex(&wire(read.as_ref())), hex(&bare), "{}", value.kind);
        decoded += 1;
    }
    assert_eq!((decoded, kept), (17, 8));
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
    // The packet, the length of the fields before the first stack, the stacks, and the form.
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
            // In these stacks the game chose an order of the entries that is not the order of the
            // type ids.
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
