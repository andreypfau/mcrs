use mcrs_minecraft_fixtures::fixture;
use mcrs_minecraft_item_stack::ComponentType;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::CustomName;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::HashError;
use mcrs_minecraft_item_stack::HashScratch;
use mcrs_minecraft_item_stack::HashedComponent;
use mcrs_minecraft_item_stack::HashedStack;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_stack::RepairCost;
use mcrs_minecraft_item_stack::Unbreakable;
use mcrs_minecraft_item_stack::WireError;
use mcrs_minecraft_item_stack::hash_stack;
use mcrs_minecraft_item_stack::matches_hashed;
use mcrs_minecraft_item_stack::read_hashed;
use mcrs_minecraft_item_stack::read_wire_with_length;
use mcrs_minecraft_item_stack::write_hashed;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_text::Text;
use serde::Deserialize;

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

const AIR: u16 = 0;
const STONE: u16 = 1;
const APPLE: u16 = 1030;
const DIAMOND_SWORD: u16 = 1073;
const DAMAGE_TYPE: ComponentType = ComponentType(3);
const UNBREAKABLE_TYPE: ComponentType = ComponentType(4);
const CUSTOM_NAME_TYPE: ComponentType = ComponentType(6);
const REPAIR_COST_TYPE: ComponentType = ComponentType(19);

/// The ids of the goldens. Each name is in the namespace `minecraft`.
struct Names;

impl Names {
    fn entries(table: NameTable) -> Vec<(u16, &'static str)> {
        let indexed = |paths: &[&'static str]| -> Vec<(u16, &'static str)> {
            (0..).zip(paths.iter().copied()).collect()
        };
        match table {
            NameTable::Item => vec![
                (AIR, "air"),
                (STONE, "stone"),
                (APPLE, "apple"),
                (DIAMOND_SWORD, "diamond_sword"),
            ],
            NameTable::Enchantment => vec![(33, "sharpness"), (40, "unbreaking")],
            NameTable::Attribute => vec![(1, "armor"), (3, "attack_damage"), (30, "scale")],
            NameTable::Block => vec![(1, "stone"), (9, "dirt")],
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

fn unhex(hex: &str) -> Vec<u8> {
    (0..hex.len())
        .step_by(2)
        .map(|at| u8::from_str_radix(&hex[at..at + 2], 16).expect("the golden holds hex digits"))
        .collect()
}

fn stack(kind: u16, count: u8) -> ItemStack {
    let count = Count::new(count).expect("the test counts are not zero");
    ItemStack::new(Kind(kind), count)
}

fn hashed(stack: Option<&ItemStack>) -> Option<HashedStack> {
    hash_stack(stack, &Names, &mut HashScratch::default()).expect("the test stack has a hash")
}

/// One component that the game saved and hashed.
struct Recorded {
    case: String,
    kind: String,
    wire: String,
    hash: i32,
}

#[derive(Deserialize)]
struct PlainGolden {
    values: Vec<PlainValue>,
}

#[derive(Deserialize)]
struct PlainValue {
    kind: String,
    input: Option<String>,
    wire: Option<String>,
    hash: Option<i32>,
}

const PLAIN_KINDS: [&str; 6] = [
    "max_stack_size",
    "max_damage",
    "damage",
    "unbreakable",
    "rarity",
    "repair_cost",
];

fn plain_cases() -> Vec<Recorded> {
    let golden: PlainGolden =
        serde_json::from_str(&fixture("item_plain").text()).expect("the golden is JSON");
    golden
        .values
        .into_iter()
        .filter(|value| PLAIN_KINDS.contains(&value.kind.as_str()))
        .filter_map(|value| {
            Some(Recorded {
                case: value.input?,
                kind: value.kind,
                wire: value.wire?,
                hash: value.hash?,
            })
        })
        .collect()
}

/// Lines `<case> wire = <hex>` and `<case> hash = <number>`.
fn record_cases() -> Vec<Recorded> {
    const CASES: [(&str, &str); 5] = [
        ("tooltip_default", "tooltip_display"),
        ("tooltip_full", "tooltip_display"),
        ("food_min", "food"),
        ("food_full", "food"),
        ("item_model", "item_model"),
    ];
    let golden = fixture("item_records").text();
    let field = |case: &str, name: &str| -> String {
        let prefix = format!("{case} {name} = ");
        golden
            .lines()
            .find_map(|line| line.strip_prefix(&prefix))
            .expect("the golden has the field")
            .to_owned()
    };
    CASES
        .iter()
        .map(|(case, kind)| Recorded {
            case: (*case).to_owned(),
            kind: (*kind).to_owned(),
            wire: field(case, "wire"),
            hash: field(case, "hash").parse().expect("the hash is a number"),
        })
        .collect()
}

/// Blocks `sample minecraft:<kind> <input>` with indented lines `wire <hex>` and `hash <number>`.
fn registry_ref_cases() -> Vec<Recorded> {
    const KINDS: [&str; 3] = ["enchantments", "attribute_modifiers", "tool"];
    let golden = fixture("item_registry_refs").text();
    let mut cases = Vec::new();
    let mut sample: Option<(String, String)> = None;
    let mut wire: Option<String> = None;
    for line in golden.lines() {
        if let Some(rest) = line.strip_prefix("sample minecraft:") {
            let (kind, input) = rest.split_once(' ').expect("a sample has an input");
            sample = KINDS
                .contains(&kind)
                .then(|| (kind.to_owned(), input.to_owned()));
            wire = None;
        } else if let Some(hex) = line.strip_prefix("  wire ") {
            wire = Some(hex.to_owned());
        } else if let Some(hash) = line.strip_prefix("  hash ")
            && let Some((kind, input)) = &sample
        {
            cases.push(Recorded {
                case: input.clone(),
                kind: kind.clone(),
                wire: wire.take().expect("the sample has its wire form"),
                hash: hash.parse().expect("the hash is a number"),
            });
        }
    }
    cases
}

fn component_type(kind: &str) -> ComponentType {
    let id = COMPONENT_TYPES
        .iter()
        .position(|name| *name == kind)
        .expect("the kind has a type id");
    ComponentType(u16::try_from(id).expect("the type id fits"))
}

/// A stack of stone with the one component, read from the wire form with length prefix.
fn stack_with(component: ComponentType, wire: &str) -> ItemStack {
    let value = unhex(wire);
    let length = u8::try_from(value.len()).expect("the golden values are short");
    let type_id = u8::try_from(component.0).expect("the type ids of the test are small");
    let mut frame = vec![0x01, 0x01, 0x01, 0x00, type_id, length];
    frame.extend(value);
    read_wire_with_length(&mut frame.as_slice(), &Names)
        .expect("the game wrote a valid component")
        .expect("the stack is not empty")
}

#[test]
fn a_component_has_the_hash_that_the_game_recorded() {
    let mut scratch = HashScratch::default();
    let mut compared: Vec<String> = Vec::new();
    let cases = plain_cases()
        .into_iter()
        .chain(record_cases())
        .chain(registry_ref_cases());
    for case in cases {
        let component = component_type(&case.kind);
        let stack = stack_with(component, &case.wire);
        let label = format!("{} {}", case.kind, case.case);
        let hashed = hash_stack(Some(&stack), &Names, &mut scratch)
            .expect("a decoded component has a hash")
            .expect("the stack is not empty");
        assert_eq!(
            hashed.values,
            [HashedComponent {
                component,
                hash: case.hash,
            }],
            "{label}"
        );
        compared.push(case.kind);
    }
    for kind in [
        "max_stack_size",
        "max_damage",
        "damage",
        "unbreakable",
        "item_model",
        "rarity",
        "enchantments",
        "attribute_modifiers",
        "tooltip_display",
        "repair_cost",
        "food",
        "tool",
    ] {
        assert!(
            compared.iter().any(|compared| compared == kind),
            "the goldens lost each case of {kind}"
        );
    }
    assert!(compared.len() >= 35, "the goldens lost cases: {compared:?}");
}

/// The fields of `container_click` before each hashed stack: the window, the state, the slot,
/// the button, the mode and the number of changed slots, then the slot of each changed slot.
#[test]
fn the_click_that_the_game_wrote_holds_the_hashes_of_the_stack() {
    let golden = fixture("inventory_packets").text();
    let bytes = unhex(
        golden
            .lines()
            .find_map(|line| line.strip_prefix("container_click "))
            .expect("the golden has the packet"),
    );
    let mut input: &[u8] = &bytes;
    let skip = |input: &mut &[u8], count: usize| {
        *input = input.get(count..).expect("the packet has the field");
    };
    skip(&mut input, 7);
    let mut stacks: Vec<(&[u8], Option<HashedStack>)> = Vec::new();
    for slot_field in [2, 2, 0] {
        skip(&mut input, slot_field);
        let before = input;
        let stack = read_hashed(&mut input, &Names).expect("the game wrote a valid hashed stack");
        stacks.push((&before[..before.len() - input.len()], stack));
    }
    assert!(input.is_empty(), "{} bytes are left", input.len());
    for (recorded, stack) in &stacks {
        let mut written = Vec::new();
        write_hashed(stack.as_ref(), &mut written).expect("a read stack writes");
        assert_eq!(written, *recorded);
    }

    let [(_, empty), (_, sword), (_, cursor)] = stacks.as_slice() else {
        panic!("the packet has three hashed stacks");
    };
    let mut scratch = HashScratch::default();
    assert_eq!(
        matches_hashed(empty.as_ref(), None, &Names, &mut scratch),
        Ok(true)
    );
    assert_eq!(
        matches_hashed(
            cursor.as_ref(),
            Some(&stack(APPLE, 1)),
            &Names,
            &mut scratch
        ),
        Ok(true)
    );

    let sword = sword.as_ref().expect("the slot holds the sword");
    let without_name = stack(DIAMOND_SWORD, 1).set(Damage(7)).set(Unbreakable);
    let computed = hashed(Some(&without_name)).expect("the stack is not empty");
    for value in &computed.values {
        assert!(
            sword.values.contains(value),
            "{value:?} is not in the click"
        );
    }
    let types: Vec<ComponentType> = computed
        .values
        .iter()
        .map(|value| value.component)
        .collect();
    assert_eq!(types, [DAMAGE_TYPE, UNBREAKABLE_TYPE]);
    let named = without_name.set(CustomName(Text::text("named")));
    assert_eq!(
        matches_hashed(Some(sword), Some(&named), &Names, &mut scratch),
        Ok(true)
    );
}

#[test]
fn a_hashed_stack_matches_a_stack_by_the_rule_of_the_specification() {
    let held = stack(DIAMOND_SWORD, 1)
        .set(Damage(7))
        .set(Unbreakable)
        .unset::<RepairCost>();
    let claimed = hashed(Some(&held)).expect("the stack is not empty");
    let changed = |change: fn(&mut HashedStack)| {
        let mut claimed = claimed.clone();
        change(&mut claimed);
        Some(claimed)
    };
    let named = held.clone().set(CustomName(Text::text("named")));
    let named_claim = hashed(Some(&named));
    let air_claim = |count: i32| {
        Some(HashedStack {
            kind: Kind(AIR),
            count,
            values: Vec::new(),
            removals: Vec::new(),
        })
    };
    let cases: [(
        &str,
        Option<HashedStack>,
        Option<ItemStack>,
        Result<bool, HashError>,
    ); 15] = [
        ("both empty", None, None, Ok(true)),
        ("air claim of count 0", air_claim(0), None, Ok(true)),
        ("air claim of count 1", air_claim(1), None, Ok(false)),
        ("empty and air", None, Some(stack(AIR, 1)), Ok(true)),
        ("empty claim", None, Some(held.clone()), Ok(false)),
        ("empty stack", Some(claimed.clone()), None, Ok(false)),
        ("same", Some(claimed.clone()), Some(held.clone()), Ok(true)),
        (
            "other order",
            changed(|claimed| claimed.values.reverse()),
            Some(held.clone()),
            Ok(true),
        ),
        (
            "other item",
            changed(|claimed| claimed.kind = Kind(STONE)),
            Some(held.clone()),
            Ok(false),
        ),
        (
            "other count",
            changed(|claimed| claimed.count = 2),
            Some(held.clone()),
            Ok(false),
        ),
        (
            "other hash",
            changed(|claimed| claimed.values[0].hash ^= 1),
            Some(held.clone()),
            Ok(false),
        ),
        (
            "no removal",
            changed(|claimed| claimed.removals.clear()),
            Some(held.clone()),
            Ok(false),
        ),
        (
            "extra removal",
            changed(|claimed| claimed.removals.push(CUSTOM_NAME_TYPE)),
            Some(held.clone()),
            Ok(false),
        ),
        (
            "extra value",
            named_claim.clone(),
            Some(held.clone()),
            Ok(false),
        ),
        (
            "text and all else equal",
            named_claim,
            Some(named),
            Ok(true),
        ),
    ];
    let mut scratch = HashScratch::default();
    for (name, claimed, held, expected) in cases {
        assert_eq!(
            matches_hashed(claimed.as_ref(), held.as_ref(), &Names, &mut scratch),
            expected,
            "{name}"
        );
    }
    assert_eq!(claimed.removals, [REPAIR_COST_TYPE]);
}

#[test]
fn the_reader_refuses_what_the_wire_form_does_not_allow() {
    let too_many = {
        let mut bytes = vec![0x01, 0x01, 0x01, 0x81, 0x02];
        bytes.extend((0..257).flat_map(|_| [0x03, 0, 0, 0, 0]));
        bytes
    };
    let cases: [(&str, Vec<u8>, WireError); 4] = [
        ("presence byte", vec![0x02], WireError::Malformed),
        (
            "cut hash",
            vec![0x01, 0x01, 0x01, 0x01, 0x03, 0, 0],
            WireError::UnexpectedEnd,
        ),
        ("257 values", too_many, WireError::TooManyEntries),
        (
            "unknown type",
            vec![0x01, 0x01, 0x01, 0x00, 0x01, 0x7a],
            WireError::UnknownComponentType(122),
        ),
    ];
    for (name, bytes, expected) in cases {
        let read = read_hashed(&mut bytes.as_slice(), &Names);
        assert_eq!(read, Err(expected), "{name}");
    }

    let accepted: [(&str, Vec<u8>, HashedStack); 2] = [
        (
            "one value type two times",
            vec![
                0x01, 0x01, 0x01, 0x02, 0x03, 0, 0, 0, 0, 0x03, 0, 0, 0, 1, 0x00,
            ],
            HashedStack {
                kind: Kind(STONE),
                count: 1,
                values: vec![HashedComponent {
                    component: DAMAGE_TYPE,
                    hash: 1,
                }],
                removals: Vec::new(),
            },
        ),
        (
            "one removal type two times",
            vec![0x01, 0x01, 0x01, 0x00, 0x02, 0x13, 0x13],
            HashedStack {
                kind: Kind(STONE),
                count: 1,
                values: Vec::new(),
                removals: vec![REPAIR_COST_TYPE],
            },
        ),
    ];
    for (name, bytes, expected) in accepted {
        let read = read_hashed(&mut bytes.as_slice(), &Names);
        assert_eq!(read, Ok(Some(expected)), "{name}");
    }

    let mut output = vec![0xAA];
    let repeated = HashedStack {
        kind: Kind(STONE),
        count: 1,
        values: Vec::new(),
        removals: vec![REPAIR_COST_TYPE, REPAIR_COST_TYPE],
    };
    assert_eq!(
        write_hashed(Some(&repeated), &mut output),
        Err(WireError::RepeatedComponentType(REPAIR_COST_TYPE))
    );
    assert_eq!(output, [0xAA], "a failed write leaves the output as it was");
}
