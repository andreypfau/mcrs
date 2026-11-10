use std::hash::DefaultHasher;
use std::hash::Hash;
use std::hash::Hasher;
use std::num::NonZeroU8;

use mcrs_minecraft_item_stack::AttributeId;
use mcrs_minecraft_item_stack::AttributeModifier;
use mcrs_minecraft_item_stack::BlockSet;
use mcrs_minecraft_item_stack::BlockTagId;
use mcrs_minecraft_item_stack::ComponentType;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::EnchantmentId;
use mcrs_minecraft_item_stack::EnchantmentLevel;
use mcrs_minecraft_item_stack::Enchantments;
use mcrs_minecraft_item_stack::Food;
use mcrs_minecraft_item_stack::ItemModel;
use mcrs_minecraft_item_stack::Lore;
use mcrs_minecraft_item_stack::MaxDamage;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_stack::ModifierOperation;
use mcrs_minecraft_item_stack::RepairCost;
use mcrs_minecraft_item_stack::Tool;
use mcrs_minecraft_item_stack::ToolRule;
use mcrs_minecraft_item_stack::TooltipDisplay;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_text::Text;

const QUIET_NAN: u32 = 0x7fc0_0000;
const NEGATIVE_NAN_WITH_PAYLOAD: u32 = 0xffc0_1234;

fn hash_of(value: &impl Hash) -> u64 {
    let mut hasher = DefaultHasher::new();
    value.hash(&mut hasher);
    hasher.finish()
}

fn assert_relation<T: PartialEq + Hash>(name: &str, left: &T, right: &T, equal: bool) {
    assert_eq!(left == right, equal, "{name}");
    if equal {
        assert_eq!(hash_of(left), hash_of(right), "the hashes of {name}");
    }
}

fn food(saturation: f32) -> Food {
    Food::new(4, saturation).expect("the nutrition is not negative")
}

fn modifier(amount: f64) -> AttributeModifier {
    let id = ResourceLocationBuf::read("test:bonus").expect("a valid identifier");
    AttributeModifier::new(AttributeId(3), id, amount, ModifierOperation::AddValue)
}

fn tool(default_mining_speed: f32) -> Tool {
    let mut tool = Tool::new(vec![ToolRule::new(BlockSet::Tag(BlockTagId(2)))]);
    tool.default_mining_speed = default_mining_speed;
    tool
}

#[test]
fn floats_are_equal_by_bit_pattern_with_one_nan() {
    let cases: [(&str, u32, u32, bool); 5] = [
        ("one number", 1.5_f32.to_bits(), 1.5_f32.to_bits(), true),
        ("two numbers", 1.5_f32.to_bits(), 1.25_f32.to_bits(), false),
        (
            "two NaN payloads",
            QUIET_NAN,
            NEGATIVE_NAN_WITH_PAYLOAD,
            true,
        ),
        ("a NaN and a number", QUIET_NAN, 1.5_f32.to_bits(), false),
        (
            "the two zeros",
            0.0_f32.to_bits(),
            (-0.0_f32).to_bits(),
            false,
        ),
    ];
    for (name, left, right, equal) in cases {
        let left = f32::from_bits(left);
        let right = f32::from_bits(right);
        assert_relation(&format!("food: {name}"), &food(left), &food(right), equal);
        assert_relation(&format!("tool: {name}"), &tool(left), &tool(right), equal);
        assert_relation(
            &format!("modifier: {name}"),
            &modifier(f64::from(left)),
            &modifier(f64::from(right)),
            equal,
        );
    }
}

#[test]
fn a_nan_keeps_its_stored_bits() {
    let saturation = food(f32::from_bits(NEGATIVE_NAN_WITH_PAYLOAD)).saturation;
    assert_eq!(saturation.to_bits(), NEGATIVE_NAN_WITH_PAYLOAD);
}

fn level(enchantment: u16, level: u8) -> EnchantmentLevel {
    EnchantmentLevel {
        enchantment: EnchantmentId(enchantment),
        level: NonZeroU8::new(level).expect("the test levels are not zero"),
    }
}

#[test]
fn enchantments_ignore_the_order_and_reject_a_repeated_enchantment() {
    let ascending = Enchantments::new([level(1, 2), level(5, 1), level(9, 3)]);
    let mixed = Enchantments::new([level(9, 3), level(1, 2), level(5, 1)]);
    let borrowed =
        Enchantments::from_static(&const { [level_of(1, 2), level_of(5, 1), level_of(9, 3)] });
    assert!(
        ascending.is_some(),
        "three different enchantments are a valid map"
    );
    assert_eq!(ascending, mixed);
    assert_eq!(ascending, borrowed);
    assert_eq!(hash_of(&ascending), hash_of(&mixed));
    assert_ne!(
        ascending,
        Enchantments::new([level(1, 2), level(5, 1), level(9, 4)])
    );

    assert_eq!(
        Enchantments::new([level(1, 2), level(5, 1), level(1, 3)]),
        None
    );
    assert_eq!(
        Enchantments::from_static(&const { [level_of(5, 1), level_of(1, 2)] }),
        None
    );
    assert_eq!(
        Enchantments::from_static(&const { [level_of(1, 1), level_of(1, 2)] }),
        None
    );
}

const fn level_of(enchantment: u16, level: u8) -> EnchantmentLevel {
    EnchantmentLevel {
        enchantment: EnchantmentId(enchantment),
        level: NonZeroU8::new(level).expect("the test levels are not zero"),
    }
}

#[test]
fn tooltip_display_compares_its_hidden_components_as_a_set() {
    let hidden = |types: &[u16]| TooltipDisplay::hiding(types.iter().copied().map(ComponentType));
    assert_eq!(
        hidden(&[11, 13, 3, 13, 11]).hidden_components(),
        [ComponentType(11), ComponentType(13), ComponentType(3)]
    );
    assert_eq!(hidden(&[13, 3, 11]), hidden(&[11, 13, 3, 13]));
    assert_eq!(
        hash_of(&hidden(&[13, 3, 11])),
        hash_of(&hidden(&[11, 13, 3, 13]))
    );
    assert_ne!(hidden(&[13, 3]), hidden(&[13, 3, 11]));
    assert_eq!(hidden(&[]), TooltipDisplay::default());

    let mut hides_all = hidden(&[13]);
    hides_all.hide_tooltip = true;
    assert_ne!(hides_all, hidden(&[13]));
}

#[test]
fn constructors_accept_only_the_ranges_of_the_saved_form() {
    let rule = |speed: f32| {
        ToolRule::new(BlockSet::Tag(BlockTagId(0)))
            .with_speed(speed)
            .is_some()
    };
    let lore = |lines: usize| Lore::new(vec![Text::default(); lines]).is_some();
    let damage_per_block = |damage: i32| Tool::new(vec![]).with_damage_per_block(damage).is_some();
    let cases: [(&str, bool, bool); 29] = [
        ("max stack size 0", MaxStackSize::new(0).is_some(), false),
        ("max stack size 1", MaxStackSize::new(1).is_some(), true),
        ("max stack size 99", MaxStackSize::new(99).is_some(), true),
        (
            "max stack size 100",
            MaxStackSize::new(100).is_some(),
            false,
        ),
        ("max damage 0", MaxDamage::new(0).is_some(), false),
        ("max damage 1", MaxDamage::new(1).is_some(), true),
        ("max damage max", MaxDamage::new(i32::MAX).is_some(), true),
        ("damage -1", Damage::new(-1).is_some(), false),
        ("damage 0", Damage::new(0).is_some(), true),
        ("damage max", Damage::new(i32::MAX).is_some(), true),
        ("repair cost -1", RepairCost::new(-1).is_some(), false),
        ("repair cost 0", RepairCost::new(0).is_some(), true),
        ("repair cost max", RepairCost::new(i32::MAX).is_some(), true),
        ("nutrition -1", Food::new(-1, 0.0).is_some(), false),
        ("nutrition 0", Food::new(0, f32::NAN).is_some(), true),
        ("damage per block -1", damage_per_block(-1), false),
        ("damage per block 0", damage_per_block(0), true),
        ("lore of 0 lines", lore(0), true),
        ("lore of 256 lines", lore(256), true),
        ("lore of 257 lines", lore(257), false),
        ("speed 0", rule(0.0), false),
        ("speed -0", rule(-0.0), false),
        ("speed -1", rule(-1.0), false),
        ("speed NaN", rule(f32::NAN), false),
        ("speed infinity", rule(f32::INFINITY), false),
        ("the smallest speed", rule(f32::from_bits(1)), true),
        ("the largest speed", rule(f32::MAX), true),
        ("speed 1.5", rule(1.5), true),
        ("speed -infinity", rule(f32::NEG_INFINITY), false),
    ];
    for (name, accepted, expected) in cases {
        assert_eq!(accepted, expected, "{name}");
    }
}

#[test]
fn an_identifier_is_equal_in_its_borrowed_and_owned_forms() {
    const STICK: ResourceLocation<'static> = match ResourceLocation::new_static("minecraft:stick") {
        Ok(location) => location,
        Err(_) => panic!("a valid identifier"),
    };
    let owned =
        ItemModel::new(ResourceLocationBuf::read("minecraft:stick").expect("a valid identifier"));
    let borrowed = ItemModel::from_static(STICK);
    assert_relation("one model in two forms", &owned, &borrowed, true);
    let other =
        ItemModel::new(ResourceLocationBuf::read("minecraft:stone").expect("a valid identifier"));
    assert_relation("two models", &other, &borrowed, false);
}
