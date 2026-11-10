use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;

/// The type ids of the specification, from 0 to 28.
pub(crate) const COMPONENT_TYPES: [&str; 29] = [
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

/// # Panics
///
/// Panics when `count` is zero.
pub(crate) fn stack(kind: u16, count: u8) -> ItemStack {
    let count = Count::new(count).expect("the test counts are not zero");
    ItemStack::new(Kind(kind), count)
}

/// The bytes of `hex`, which may hold whitespace between the digits.
///
/// # Panics
///
/// Panics when `hex` holds a character that is not a hex digit or whitespace.
pub(crate) fn unhex(hex: &str) -> Vec<u8> {
    let digits: Vec<char> = hex.chars().filter(|digit| !digit.is_whitespace()).collect();
    digits
        .chunks(2)
        .map(|pair| {
            let pair: String = pair.iter().collect();
            u8::from_str_radix(&pair, 16).expect("the test holds hex digits")
        })
        .collect()
}
