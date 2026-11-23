use std::borrow::Cow;
use std::num::NonZeroU8;
use std::sync::LazyLock;

use mcrs_minecraft_entity_attribute::ModifierOperation;
use mcrs_minecraft_game_registries::registry_load::test_registries;
use mcrs_minecraft_item::component::predicate::ItemPredicate;
use mcrs_minecraft_item_stack::AttackAnimation;
use mcrs_minecraft_item_stack::AttributeId;
use mcrs_minecraft_item_stack::AttributeModifier;
use mcrs_minecraft_item_stack::AttributeModifiers;
use mcrs_minecraft_item_stack::Builtin;
use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::EnchantmentId;
use mcrs_minecraft_item_stack::EnchantmentLevel;
use mcrs_minecraft_item_stack::Enchantments;
use mcrs_minecraft_item_stack::InteractAnimation;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_item_stack::MaxDamage;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_stack::PiercingWeapon;
use mcrs_minecraft_item_stack::SlotGroup;
use mcrs_minecraft_item_stack::SwingAnimation;
use mcrs_minecraft_item_stack::SwingKind;
use mcrs_minecraft_item_stack::Unbreakable;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_predicate::BakedItemPredicate;
use mcrs_minecraft_predicate::ItemSubject;
use mcrs_minecraft_registries::Registries;
use mcrs_minecraft_registries::Resolve;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;

fn registries() -> &'static Registries {
    test_registries()
}

struct RegistryLookup;

impl NameLookup for RegistryLookup {
    fn id(&self, table: NameTable, name: ResourceLocation<'_>) -> Option<u16> {
        let registry = match table {
            NameTable::Item => "minecraft:item",
            NameTable::Enchantment => "minecraft:enchantment",
            NameTable::Attribute => "minecraft:attribute",
            NameTable::Block => "minecraft:block",
            NameTable::ComponentType => "minecraft:data_component_type",
            NameTable::SoundEvent => "minecraft:sound_event",
            _ => return None,
        };
        registries().id_of(ResourceLocation::read(registry).ok()?, name)
    }

    fn name(&self, _: NameTable, _: u16) -> Option<ResourceLocation<'_>> {
        None
    }
}

fn id(registry: &str, name: &str) -> u16 {
    registries()
        .id_of(
            ResourceLocation::read(registry).unwrap(),
            ResourceLocation::read(name).unwrap(),
        )
        .unwrap_or_else(|| panic!("{registry} has no {name}"))
}

fn item(name: &str) -> Kind {
    Kind(id("minecraft:item", name))
}

pub(crate) fn enchantment(name: &str, level: u8) -> EnchantmentLevel {
    EnchantmentLevel {
        enchantment: EnchantmentId(id("minecraft:enchantment", name)),
        level: NonZeroU8::new(level).expect("a level above zero"),
    }
}

fn modifier(attribute: &str, id_text: &str, amount: f64, slot: SlotGroup) -> AttributeModifier {
    let mut modifier = AttributeModifier::new(
        AttributeId(id("minecraft:attribute", attribute)),
        ResourceLocationBuf::read(id_text).unwrap(),
        amount,
        ModifierOperation::AddValue,
    );
    modifier.slot = slot;
    modifier
}

fn sword_modifiers() -> AttributeModifiers {
    AttributeModifiers {
        modifiers: Cow::Owned(vec![
            modifier(
                "minecraft:attack_damage",
                "minecraft:base_attack_damage",
                7.0,
                SlotGroup::Mainhand,
            ),
            modifier(
                "minecraft:attack_speed",
                "minecraft:base_attack_speed",
                -2.4,
                SlotGroup::Mainhand,
            ),
        ]),
    }
}

fn no_enchantments() -> Enchantments {
    Enchantments::new([]).expect("no enchantment occurs two times")
}

pub(crate) static TABLE: LazyLock<ItemTable> = LazyLock::new(|| {
    let sword = item("minecraft:diamond_sword");
    let stick = item("minecraft:stick");
    let prototype = |kind: Kind| -> Vec<Builtin> {
        if kind == sword {
            vec![
                Builtin::MaxStackSize(MaxStackSize(1)),
                Builtin::MaxDamage(MaxDamage(1561)),
                Builtin::Damage(Damage(0)),
                Builtin::Enchantments(no_enchantments()),
                Builtin::AttributeModifiers(sword_modifiers()),
            ]
        } else if kind == stick {
            vec![
                Builtin::MaxStackSize(MaxStackSize(64)),
                Builtin::Enchantments(no_enchantments()),
                Builtin::AttributeModifiers(AttributeModifiers::default()),
            ]
        } else {
            Vec::new()
        }
    };
    ItemTable::leak((0..=sword.0.max(stick.0)).map(|id| prototype(Kind(id))))
});

fn stack(name: &str, count: u8) -> ItemStack {
    ItemStack::new(item(name), Count::new(count).expect("a count above zero"))
}

pub(crate) fn sword() -> ItemStack {
    stack("minecraft:diamond_sword", 1)
}

pub(crate) fn bake(json: &str) -> BakedItemPredicate {
    let named = serde_json::from_str(json).unwrap_or_else(|error| panic!("{json}: {error}"));
    let resolved = ItemPredicate::resolve(named, registries())
        .unwrap_or_else(|error| panic!("{json}: {error}"));
    BakedItemPredicate::bake(&resolved, registries(), &RegistryLookup)
        .unwrap_or_else(|error| panic!("{json}: {error}"))
}

#[derive(Clone, Debug)]
enum Held {
    Empty,
    Stack(ItemStack),
}

impl Held {
    fn subject(&self) -> ItemSubject<'_> {
        match self {
            Held::Empty => ItemSubject::Empty,
            Held::Stack(stack) => ItemSubject::Stack(stack),
        }
    }
}

fn check(cases: &[(&str, &str, Held, bool)]) {
    let failures: Vec<String> = cases
        .iter()
        .filter(|(_, json, held, expected)| bake(json).matches(held.subject(), &TABLE) != *expected)
        .map(|(rule, json, stack, expected)| {
            format!("{rule}: {json} on {stack:?} should give {expected}")
        })
        .collect();
    assert!(failures.is_empty(), "{failures:#?}");
}

#[test]
fn item_predicate_fields_test_item_and_count_pred_itm_002_003_gen_005() {
    let stick = |count| Held::Stack(stack("minecraft:stick", count));
    check(&[
        ("PRED-GEN-005", "{}", Held::Stack(sword()), true),
        (
            "PRED-ITM-002",
            r#"{"items": "minecraft:stick"}"#,
            stick(1),
            true,
        ),
        (
            "PRED-ITM-002",
            r#"{"items": "minecraft:stick"}"#,
            Held::Stack(sword()),
            false,
        ),
        (
            "PRED-ITM-002",
            r#"{"items": ["minecraft:apple", "minecraft:diamond_sword"]}"#,
            Held::Stack(sword()),
            true,
        ),
        (
            "PRED-ITM-002",
            r##"{"items": "#minecraft:swords"}"##,
            Held::Stack(sword()),
            true,
        ),
        (
            "PRED-ITM-002",
            r##"{"items": "#minecraft:swords"}"##,
            stick(1),
            false,
        ),
        (
            "PRED-NUM-001",
            r#"{"count": {"min": 2, "max": 5}}"#,
            stick(5),
            true,
        ),
        (
            "PRED-NUM-001",
            r#"{"count": {"min": 2, "max": 5}}"#,
            stick(6),
            false,
        ),
        ("PRED-NUM-001", r#"{"count": 3}"#, stick(3), true),
        ("PRED-NUM-001", r#"{"count": 3}"#, stick(2), false),
        ("PRED-ITM-003", "{}", Held::Empty, true),
        (
            "PRED-ITM-003",
            r#"{"count": {"min": 1}}"#,
            Held::Empty,
            false,
        ),
        ("PRED-ITM-003", r#"{"count": 0}"#, Held::Empty, true),
        (
            "PRED-ITM-003",
            r#"{"items": "minecraft:air"}"#,
            Held::Empty,
            true,
        ),
        (
            "PRED-ITM-003",
            r#"{"items": "minecraft:stick"}"#,
            Held::Empty,
            false,
        ),
        (
            "PRED-ITM-003",
            r#"{"components": {"minecraft:max_stack_size": 64}}"#,
            Held::Empty,
            false,
        ),
        (
            "PRED-ITM-003",
            r#"{"predicates": {"minecraft:max_stack_size": {}}}"#,
            Held::Empty,
            false,
        ),
    ]);
}

#[test]
fn the_matchers_read_the_effective_components_pred_itm_004_to_007() {
    let damaged = sword().set(Damage(5));
    let unbroken = sword().unset::<MaxDamage>().set(Unbreakable);
    check(&[
        (
            "PRED-ITM-004",
            r#"{"components": {}}"#,
            Held::Stack(sword()),
            true,
        ),
        (
            "PRED-ITM-004",
            r#"{"components": {"minecraft:damage": 5}}"#,
            Held::Stack(damaged.clone()),
            true,
        ),
        (
            "PRED-ITM-004",
            r#"{"components": {"minecraft:damage": 5}}"#,
            Held::Stack(sword()),
            false,
        ),
        (
            "PRED-ITM-004",
            r#"{"components": {"minecraft:damage": 5}}"#,
            Held::Stack(stack("minecraft:stick", 1)),
            false,
        ),
        (
            "PRED-ITM-004",
            r#"{"components": {"minecraft:max_damage": 1561, "minecraft:damage": 5}}"#,
            Held::Stack(damaged),
            true,
        ),
        (
            "PRED-ITM-004",
            r#"{"components": {"minecraft:max_damage": 1561}}"#,
            Held::Stack(unbroken.clone()),
            false,
        ),
        (
            "PRED-ITM-005",
            r#"{"predicates": {}}"#,
            Held::Stack(sword()),
            true,
        ),
        (
            "PRED-ITM-006",
            r#"{"predicates": {"minecraft:max_damage": {}}}"#,
            Held::Stack(sword()),
            true,
        ),
        (
            "PRED-ITM-006",
            r#"{"predicates": {"minecraft:max_damage": {}}}"#,
            Held::Stack(unbroken.clone()),
            false,
        ),
        (
            "PRED-ITM-006",
            r#"{"predicates": {"minecraft:unbreakable": {}}}"#,
            Held::Stack(unbroken),
            true,
        ),
        (
            "PRED-ITM-006",
            r#"{"predicates": {"minecraft:unbreakable": {}}}"#,
            Held::Stack(sword()),
            false,
        ),
        (
            "PRED-ITM-007",
            r#"{"predicates": {"minecraft:damage": {}}}"#,
            Held::Stack(stack("minecraft:stick", 1)),
            false,
        ),
    ]);
}

#[test]
#[expect(clippy::too_many_lines, reason = "one table of cases")]
fn the_data_component_predicates_test_their_component_pred_itm_010_to_014() {
    let worn = Held::Stack(sword().set(Damage(100)));
    let overused = Held::Stack(sword().set(MaxDamage(1561)).set(Damage(2000)));
    let enchanted =
        Held::Stack(sword().set(
            Enchantments::new([enchantment("minecraft:sharpness", 2)]).expect("one sharpness"),
        ));
    let stick = Held::Stack(stack("minecraft:stick", 1));
    check(&[
        (
            "PRED-ITM-010",
            r#"{"predicates": {"minecraft:damage": {"durability": {"min": 1461}}}}"#,
            worn.clone(),
            true,
        ),
        (
            "PRED-ITM-010",
            r#"{"predicates": {"minecraft:damage": {"durability": {"min": 1462}}}}"#,
            worn.clone(),
            false,
        ),
        (
            "PRED-ITM-010",
            r#"{"predicates": {"minecraft:damage": {"damage": 100}}}"#,
            worn.clone(),
            true,
        ),
        (
            "PRED-ITM-010",
            r#"{"predicates": {"minecraft:damage": {"damage": 99}}}"#,
            worn,
            false,
        ),
        (
            "PRED-ITM-010",
            r#"{"predicates": {"minecraft:damage": {"durability": {"max": -439}, "damage": {"min": 2000}}}}"#,
            overused.clone(),
            true,
        ),
        (
            "PRED-ITM-010",
            r#"{"predicates": {"minecraft:damage": {"durability": {"min": 0}}}}"#,
            overused,
            false,
        ),
        (
            "PRED-ITM-010",
            r#"{"predicates": {"minecraft:damage": {"durability": {"min": 0}}}}"#,
            Held::Stack(sword().set(MaxDamage(i32::MAX)).set(Damage(i32::MIN))),
            true,
        ),
        (
            "PRED-ITM-012",
            r#"{"predicates": {"minecraft:enchantments": [{"enchantments": "minecraft:sharpness", "levels": {"min": 2}}]}}"#,
            enchanted.clone(),
            true,
        ),
        (
            "PRED-ITM-012",
            r#"{"predicates": {"minecraft:enchantments": [{"enchantments": "minecraft:sharpness", "levels": {"min": 3}}]}}"#,
            enchanted.clone(),
            false,
        ),
        (
            "PRED-ITM-012",
            r#"{"predicates": {"minecraft:enchantments": [{"enchantments": "minecraft:efficiency"}]}}"#,
            enchanted.clone(),
            false,
        ),
        (
            "PRED-ITM-012",
            r##"{"predicates": {"minecraft:enchantments": [{"enchantments": "#minecraft:exclusive_set/damage"}]}}"##,
            enchanted.clone(),
            true,
        ),
        (
            "PRED-ITM-012",
            r#"{"predicates": {"minecraft:enchantments": [{"levels": 2}]}}"#,
            enchanted.clone(),
            true,
        ),
        (
            "PRED-ITM-012",
            r#"{"predicates": {"minecraft:enchantments": [{"levels": 1}]}}"#,
            enchanted.clone(),
            false,
        ),
        (
            "PRED-ITM-012",
            r#"{"predicates": {"minecraft:enchantments": [{}]}}"#,
            enchanted.clone(),
            true,
        ),
        (
            "PRED-ITM-012",
            r#"{"predicates": {"minecraft:enchantments": [{}]}}"#,
            stick.clone(),
            false,
        ),
        (
            "PRED-ITM-011",
            r#"{"predicates": {"minecraft:enchantments": [{"levels": 2}, {"enchantments": "minecraft:efficiency"}]}}"#,
            enchanted,
            false,
        ),
        (
            "PRED-ITM-011",
            r#"{"predicates": {"minecraft:enchantments": []}}"#,
            stick.clone(),
            true,
        ),
        (
            "PRED-ITM-011",
            r#"{"predicates": {"minecraft:enchantments": []}}"#,
            Held::Empty,
            false,
        ),
        (
            "PRED-ITM-014",
            r#"{"predicates": {"minecraft:custom_data": {}}}"#,
            stick.clone(),
            true,
        ),
        (
            "PRED-ITM-014",
            r#"{"predicates": {"minecraft:custom_data": {}}}"#,
            Held::Empty,
            true,
        ),
        (
            "PRED-ITM-014",
            r#"{"predicates": {"minecraft:custom_data": "{a: 1b}"}}"#,
            stick,
            false,
        ),
    ]);
}

#[test]
#[expect(clippy::too_many_lines, reason = "one table of cases")]
fn attribute_modifiers_test_a_collection_pred_itm_020_col_001_to_004() {
    let modifiers = |collection: &str| {
        format!(
            r#"{{"predicates": {{"minecraft:attribute_modifiers": {{"modifiers": {collection}}}}}}}"#
        )
    };
    let stick = Held::Stack(stack("minecraft:stick", 1));
    let cases: Vec<(&str, String, Held, bool)> = vec![
        ("PRED-COL-001", modifiers("{}"), Held::Stack(sword()), true),
        ("PRED-COL-001", modifiers("{}"), stick.clone(), true),
        (
            "PRED-ITM-020",
            modifiers(
                r#"{"contains": [{"attribute": "minecraft:attack_damage", "amount": {"min": 7.0}, "operation": "add_value", "slot": "mainhand", "id": "minecraft:base_attack_damage"}]}"#,
            ),
            Held::Stack(sword()),
            true,
        ),
        (
            "PRED-ITM-020",
            modifiers(
                r#"{"contains": [{"attribute": "minecraft:attack_damage", "slot": "offhand"}]}"#,
            ),
            Held::Stack(sword()),
            false,
        ),
        (
            "PRED-ITM-020",
            modifiers(r#"{"contains": [{"amount": {"min": 7.1}}]}"#),
            Held::Stack(sword()),
            false,
        ),
        (
            "PRED-ITM-020",
            modifiers(r#"{"contains": [{"id": "minecraft:other"}]}"#),
            Held::Stack(sword()),
            false,
        ),
        (
            "PRED-ITM-020",
            modifiers(r#"{"contains": [{"operation": "add_multiplied_base"}]}"#),
            Held::Stack(sword()),
            false,
        ),
        (
            "PRED-COL-002",
            modifiers(
                r#"{"contains": [{"attribute": "minecraft:attack_damage"}, {"id": "minecraft:base_attack_damage"}]}"#,
            ),
            Held::Stack(sword()),
            true,
        ),
        (
            "PRED-COL-002",
            modifiers(r#"{"contains": []}"#),
            stick.clone(),
            true,
        ),
        (
            "PRED-COL-002",
            modifiers(r#"{"contains": [{}]}"#),
            stick.clone(),
            false,
        ),
        (
            "PRED-COL-003",
            modifiers(r#"{"count": [{"test": {"operation": "add_value"}, "count": 2}]}"#),
            Held::Stack(sword()),
            true,
        ),
        (
            "PRED-COL-003",
            modifiers(
                r#"{"count": [{"test": {"attribute": "minecraft:attack_speed"}, "count": {"max": 0}}]}"#,
            ),
            Held::Stack(sword()),
            false,
        ),
        (
            "PRED-COL-003",
            modifiers(
                r#"{"count": [{"test": {"attribute": "minecraft:luck"}, "count": {"max": 0}}]}"#,
            ),
            Held::Stack(sword()),
            true,
        ),
        (
            "PRED-COL-004",
            modifiers(r#"{"size": 2}"#),
            Held::Stack(sword()),
            true,
        ),
        (
            "PRED-COL-004",
            modifiers(r#"{"size": {"min": 3}}"#),
            Held::Stack(sword()),
            false,
        ),
        ("PRED-COL-004", modifiers(r#"{"size": 0}"#), stick, true),
    ];
    let cases: Vec<(&str, &str, Held, bool)> = cases
        .iter()
        .map(|(rule, json, stack, expected)| (*rule, json.as_str(), stack.clone(), *expected))
        .collect();
    check(&cases);
}

#[test]
fn exact_and_presence_predicates_match_swing_and_piercing_components() {
    let spear = sword()
        .set(AttackAnimation(SwingAnimation {
            kind: SwingKind::Stab,
            duration: 13,
        }))
        .set(InteractAnimation(SwingAnimation {
            kind: SwingKind::None,
            duration: 4,
        }))
        .set(PiercingWeapon::new());

    check(&[
        (
            "REG-FLD-548",
            r#"{"components": {"minecraft:attack_animation": {"type": "stab", "duration": 13}}}"#,
            Held::Stack(spear.clone()),
            true,
        ),
        (
            "REG-FLD-548",
            r#"{"components": {"minecraft:attack_animation": {"type": "whack", "duration": 6}}}"#,
            Held::Stack(spear.clone()),
            false,
        ),
        (
            "REG-FLD-548",
            r#"{"components": {"minecraft:interact_animation": {"type": "none", "duration": 4}}}"#,
            Held::Stack(spear.clone()),
            true,
        ),
        (
            "REG-FLD-546",
            r#"{"components": {"minecraft:piercing_weapon": {}}}"#,
            Held::Stack(spear.clone()),
            true,
        ),
        (
            "REG-FLD-546",
            r#"{"components": {"minecraft:piercing_weapon": {"dismounts": true}}}"#,
            Held::Stack(spear.clone()),
            false,
        ),
        (
            "REG-FLD-546",
            r#"{"components": {"minecraft:piercing_weapon": {}}}"#,
            Held::Stack(sword()),
            false,
        ),
        (
            "REG-FLD-546",
            r#"{"predicates": {"minecraft:piercing_weapon": {}}}"#,
            Held::Stack(spear.clone()),
            true,
        ),
        (
            "REG-FLD-546",
            r#"{"predicates": {"minecraft:piercing_weapon": {}}}"#,
            Held::Stack(sword()),
            false,
        ),
        (
            "REG-FLD-548",
            r#"{"predicates": {"minecraft:attack_animation": {}}}"#,
            Held::Stack(spear.clone()),
            true,
        ),
        (
            "REG-FLD-548",
            r#"{"predicates": {"minecraft:attack_animation": {}}}"#,
            Held::Stack(sword()),
            false,
        ),
        (
            "REG-FLD-548",
            r#"{"predicates": {"minecraft:interact_animation": {}}}"#,
            Held::Stack(spear),
            true,
        ),
        (
            "REG-FLD-548",
            r#"{"predicates": {"minecraft:interact_animation": {}}}"#,
            Held::Stack(sword()),
            false,
        ),
    ]);
}
