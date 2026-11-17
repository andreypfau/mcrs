use mcrs_minecraft_item_stack::Count;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Kind;
use mcrs_minecraft_nbt::compound::NbtCompound;
use mcrs_minecraft_nbt::tag::NbtTag;
use mcrs_minecraft_wire::Encode;
use mcrs_minecraft_wire::VarInt;

pub(crate) const COMPONENT_TYPES: [&str; 73] = [
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
    "weapon",
    "attack_range",
    "enchantable",
    "equippable",
    "repairable",
    "glider",
    "tooltip_style",
    "death_protection",
    "blocks_attacks",
    "piercing_weapon",
    "kinetic_weapon",
    "attack_animation",
    "interact_animation",
    "additional_trade_cost",
    "block_transformer",
    "villager_food",
    "stored_enchantments",
    "dye",
    "dyed_color",
    "map_id",
    "map_decorations",
    "map_post_processing",
    "charged_projectiles",
    "bundle_contents",
    "potion_contents",
    "potion_duration_scale",
    "suspicious_stew_effects",
    "writable_book_content",
    "written_book_content",
    "trim",
    "debug_stick_state",
    "entity_data",
    "bucket_entity_data",
    "block_entity_data",
    "instrument",
    "provides_trim_material",
    "ominous_bottle_amplifier",
    "jukebox_playable",
    "provides_banner_patterns",
    "recipes",
    "lodestone_tracker",
    "firework_explosion",
    "fireworks",
    "profile",
];

pub(crate) fn stack(kind: u16, count: u8) -> ItemStack {
    let count = Count::new(count).expect("the test counts are not zero");
    ItemStack::new(Kind(kind), count)
}

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

pub(crate) fn var_int(value: usize) -> Vec<u8> {
    let mut bytes = Vec::new();
    VarInt(i32::try_from(value).expect("the test lengths fit in 32 bits"))
        .encode(&mut bytes)
        .expect("a Vec takes every byte");
    bytes
}

pub(crate) fn same(left: &NbtTag, right: &NbtTag) -> bool {
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

pub(crate) fn same_map(left: &NbtCompound, right: &NbtCompound) -> bool {
    left.child_tags.len() == right.child_tags.len()
        && left
            .child_tags
            .iter()
            .all(|(key, value)| right.get(key).is_some_and(|other| same(value, other)))
}
