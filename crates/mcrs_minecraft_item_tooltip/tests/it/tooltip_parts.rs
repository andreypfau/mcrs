use std::num::NonZeroU8;

use mcrs_minecraft_entity_attribute::ModifierOperation;
use mcrs_minecraft_item_stack::AttributeId;
use mcrs_minecraft_item_stack::AttributeModifier;
use mcrs_minecraft_item_stack::AttributeModifiers;
use mcrs_minecraft_item_stack::ComponentType;
use mcrs_minecraft_item_stack::CustomName;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::EnchantmentId;
use mcrs_minecraft_item_stack::EnchantmentLevel;
use mcrs_minecraft_item_stack::Enchantments;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Lore;
use mcrs_minecraft_item_stack::MapId;
use mcrs_minecraft_item_stack::ModifierDisplay;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_stack::Profile;
use mcrs_minecraft_item_stack::TooltipDisplay;
use mcrs_minecraft_item_stack::Unbreakable;
use mcrs_minecraft_item_tooltip::AttackBases;
use mcrs_minecraft_item_tooltip::EnchantmentClass;
use mcrs_minecraft_item_tooltip::EnchantmentEntry;
use mcrs_minecraft_item_tooltip::ItemAvailability;
use mcrs_minecraft_item_tooltip::MapSummary;
use mcrs_minecraft_item_tooltip::TooltipDetail;
use mcrs_minecraft_item_tooltip::TooltipInputs;
use mcrs_minecraft_item_tooltip::TooltipView;
use mcrs_minecraft_item_tooltip::append_tooltip_parts;
use mcrs_minecraft_resource::ResourceLocation;
use mcrs_minecraft_resource::ResourceLocationBuf;
use mcrs_minecraft_text::Color;
use mcrs_minecraft_text::IntoText;
use mcrs_minecraft_text::ItemHover;
use mcrs_minecraft_text::Text;
use mcrs_minecraft_text::TranslateArg;

use crate::fixture::CORPUS;

const PLAYER: AttackBases = AttackBases {
    attack_damage: 1.0,
    attack_speed: 4.0,
};
const SHARPNESS: EnchantmentId = EnchantmentId(0);
const MENDING: EnchantmentId = EnchantmentId(1);
const VANISHING: EnchantmentId = EnchantmentId(2);
const MAP: MapSummary = MapSummary {
    id: MapId(3),
    locked: true,
    scale: 2,
};

struct Case {
    name: &'static str,
    item: &'static str,
    stack: ItemStack,
    detail: TooltipDetail,
    view: TooltipView,
    availability: ItemAvailability,
    expected: Vec<Text>,
}

fn case(name: &'static str, item: &'static str, expected: Vec<Text>) -> Case {
    Case {
        name,
        item,
        stack: CORPUS.stack(item),
        detail: TooltipDetail::Basic,
        view: TooltipView::Standard,
        availability: ItemAvailability::Enabled,
        expected,
    }
}

fn key(key: &'static str) -> Text {
    Text::translate(key, Vec::new())
}

fn args(key: &'static str, with: Vec<TranslateArg<ItemHover>>) -> Text {
    Text::translate_args(key, with)
}

fn named(color: Color, name: Text) -> Text {
    Text::default().color(color).add_child(name)
}

fn level(at: u8) -> NonZeroU8 {
    NonZeroU8::new(at).expect("a level above 0")
}

fn attribute(key: &'static str, number: &str, attribute: &str) -> Text {
    Text::translate_args(
        key,
        vec![
            TranslateArg::Text(Text::text(number.to_owned())),
            TranslateArg::Text(Text::translate(
                format!("attribute.name.{attribute}"),
                Vec::new(),
            )),
        ],
    )
}

fn sword_lines(damage: &str) -> Vec<Text> {
    let base = |number: &str, name: &str| {
        Text::text(" ")
            .color(Color::DARK_GREEN)
            .add_child(attribute("attribute.modifier.equals.0", number, name))
    };
    vec![
        Text::default(),
        key("item.modifiers.mainhand").color(Color::GRAY),
        base(damage, "attack_damage"),
        base("1.6", "attack_speed"),
    ]
}

fn component_type(path: &str) -> ComponentType {
    let name = ResourceLocation::new("minecraft", path).expect("a component name");
    ComponentType(
        CORPUS
            .names
            .id(NameTable::ComponentType, name)
            .expect("the component type is registered"),
    )
}

fn modifier(
    attribute: &str,
    amount: f64,
    operation: ModifierOperation,
    display: ModifierDisplay,
) -> AttributeModifier {
    let name = ResourceLocation::new("minecraft", attribute).expect("an attribute name");
    let id = CORPUS
        .names
        .id(NameTable::Attribute, name)
        .expect("the attribute is registered");
    let key = ResourceLocationBuf::new("example", "modifier").expect("a modifier id");
    let mut modifier = AttributeModifier::new(AttributeId(id), key, amount, operation);
    modifier.display = display;
    modifier
}

fn modified_stone(name: &'static str, modifier: AttributeModifier, parts: Vec<Text>) -> Case {
    let header = if parts.is_empty() {
        Vec::new()
    } else {
        vec![
            Text::default(),
            key("item.modifiers.any").color(Color::GRAY),
        ]
    };
    let mut row = case(
        name,
        "minecraft:stone",
        [vec![stone_name()], header, parts].concat(),
    );
    row.stack = row.stack.set(AttributeModifiers {
        modifiers: vec![modifier].into(),
    });
    row
}

fn modifier_cases() -> Vec<Case> {
    vec![
        modified_stone(
            "negative multiplied modifier",
            modifier(
                "movement_speed",
                -0.15,
                ModifierOperation::AddMultipliedBase,
                ModifierDisplay::Default,
            ),
            vec![attribute("attribute.modifier.take.1", "15", "movement_speed").color(Color::RED)],
        ),
        modified_stone(
            "knockback resistance added",
            modifier(
                "knockback_resistance",
                0.1,
                ModifierOperation::AddValue,
                ModifierDisplay::Default,
            ),
            vec![
                attribute("attribute.modifier.plus.0", "1", "knockback_resistance")
                    .color(Color::BLUE),
            ],
        ),
        modified_stone(
            "fall damage increase",
            modifier(
                "fall_damage_multiplier",
                0.25,
                ModifierOperation::AddMultipliedTotal,
                ModifierDisplay::Default,
            ),
            vec![
                attribute("attribute.modifier.plus.2", "25", "fall_damage_multiplier")
                    .color(Color::RED),
            ],
        ),
        modified_stone(
            "hidden modifier",
            modifier(
                "armor",
                2.0,
                ModifierOperation::AddValue,
                ModifierDisplay::Hidden,
            ),
            Vec::new(),
        ),
        modified_stone(
            "override modifier",
            modifier(
                "armor",
                2.0,
                ModifierOperation::AddValue,
                ModifierDisplay::Override(Text::text("Sturdy")),
            ),
            vec![Text::text("Sturdy")],
        ),
    ]
}

fn stone_name() -> Text {
    named(Color::WHITE, key("block.minecraft.stone"))
}

fn iron_sword_name() -> Text {
    named(Color::WHITE, key("item.minecraft.iron_sword"))
}

fn lore() -> Lore {
    Lore::new(vec![
        Text::text("first"),
        Text::text("second").color(Color::GOLD),
    ])
    .expect("two lines are within the limit")
}

fn plain_cases() -> Vec<Case> {
    let mut advanced_stone = case(
        "stone, advanced",
        "minecraft:stone",
        vec![
            stone_name(),
            Text::text("minecraft:stone").color(Color::DARK_GRAY),
            args("item.components", vec![TranslateArg::from(13_i64)]).color(Color::DARK_GRAY),
        ],
    );
    advanced_stone.detail = TooltipDetail::Advanced;
    let mut hidden = case("hide_tooltip", "minecraft:stone", Vec::new());
    let mut display = TooltipDisplay::hiding([]);
    display.hide_tooltip = true;
    hidden.stack = hidden.stack.set(display.clone());
    let mut creative = case(
        "hide_tooltip, creative",
        "minecraft:stone",
        vec![stone_name()],
    );
    creative.stack = creative.stack.set(display);
    creative.view = TooltipView::Creative;
    let mut disabled = case(
        "disabled item",
        "minecraft:stone",
        vec![stone_name(), key("item.disabled").color(Color::RED)],
    );
    disabled.availability = ItemAvailability::Disabled;
    let mut renamed = case(
        "custom name",
        "minecraft:stone",
        vec![named(Color::WHITE, Text::text("Rock")).italic()],
    );
    renamed.stack = renamed.stack.set(CustomName(Text::text("Rock")));
    let mut with_lore = case(
        "lore",
        "minecraft:stone",
        vec![
            stone_name(),
            Text::default()
                .color(Color::DARK_PURPLE)
                .italic()
                .add_child(Text::text("first")),
            Text::default()
                .color(Color::DARK_PURPLE)
                .italic()
                .add_child(Text::text("second").color(Color::GOLD)),
        ],
    );
    with_lore.stack = with_lore.stack.set(lore());
    let mut hidden_lore = case("hidden lore", "minecraft:stone", vec![stone_name()]);
    hidden_lore.stack = hidden_lore
        .stack
        .set(lore())
        .set(TooltipDisplay::hiding([component_type("lore")]));
    vec![
        case("stone", "minecraft:stone", vec![stone_name()]),
        advanced_stone,
        hidden,
        creative,
        disabled,
        renamed,
        with_lore,
        hidden_lore,
    ]
}

fn tool_cases() -> Vec<Case> {
    let mut enchanted = case(
        "enchanted sword",
        "minecraft:diamond_sword",
        [
            vec![
                named(Color::AQUA, key("item.minecraft.diamond_sword")),
                key("enchantment.minecraft.mending").color(Color::GRAY),
                key("enchantment.minecraft.sharpness")
                    .color(Color::GRAY)
                    .add_child(Text::text(" "))
                    .add_child(key("enchantment.level.5")),
                key("enchantment.minecraft.vanishing_curse").color(Color::RED),
            ],
            sword_lines("7"),
        ]
        .concat(),
    );
    let levels = [
        EnchantmentLevel {
            enchantment: SHARPNESS,
            level: level(5),
        },
        EnchantmentLevel {
            enchantment: MENDING,
            level: level(1),
        },
        EnchantmentLevel {
            enchantment: VANISHING,
            level: level(1),
        },
    ];
    enchanted.stack = enchanted
        .stack
        .set(Enchantments::new(levels).expect("distinct enchantments"));
    let mut damaged = case(
        "damaged sword, advanced",
        "minecraft:iron_sword",
        [
            vec![iron_sword_name()],
            sword_lines("6"),
            vec![
                args(
                    "item.durability",
                    vec![TranslateArg::from(200), TranslateArg::from(250)],
                ),
                Text::text("minecraft:iron_sword").color(Color::DARK_GRAY),
                args("item.components", vec![TranslateArg::from(19_i64)]).color(Color::DARK_GRAY),
            ],
        ]
        .concat(),
    );
    damaged.stack = damaged.stack.set(Damage(50));
    damaged.detail = TooltipDetail::Advanced;
    let mut unbreakable = case(
        "unbreakable sword",
        "minecraft:iron_sword",
        [
            vec![iron_sword_name()],
            sword_lines("6"),
            vec![key("item.unbreakable").color(Color::BLUE)],
        ]
        .concat(),
    );
    unbreakable.stack = unbreakable.stack.set(Unbreakable);
    vec![
        case(
            "iron sword",
            "minecraft:iron_sword",
            [vec![iron_sword_name()], sword_lines("6")].concat(),
        ),
        enchanted,
        damaged,
        unbreakable,
    ]
}

fn other_cases() -> Vec<Case> {
    let helmet = case(
        "leather helmet",
        "minecraft:leather_helmet",
        vec![
            named(Color::WHITE, key("item.minecraft.leather_helmet")),
            Text::default(),
            key("item.modifiers.head").color(Color::GRAY),
            attribute("attribute.modifier.plus.0", "1", "armor").color(Color::BLUE),
        ],
    );
    let mut map = case(
        "known map",
        "minecraft:filled_map",
        vec![
            named(Color::WHITE, key("item.minecraft.filled_map")),
            args("filled_map.id", vec![TranslateArg::from(3)]).color(Color::GRAY),
            key("filled_map.locked").color(Color::GRAY),
        ],
    );
    map.stack = map.stack.set(MAP.id);
    let mut advanced_map = case(
        "known map, advanced",
        "minecraft:filled_map",
        vec![
            named(Color::WHITE, key("item.minecraft.filled_map")),
            args("filled_map.id", vec![TranslateArg::from(3)]).color(Color::GRAY),
            key("filled_map.locked").color(Color::GRAY),
            args("filled_map.scale", vec![TranslateArg::from(4)]).color(Color::GRAY),
            args(
                "filled_map.level",
                vec![TranslateArg::from(2), TranslateArg::from(4)],
            )
            .color(Color::GRAY),
            Text::text("minecraft:filled_map").color(Color::DARK_GRAY),
            args("item.components", vec![TranslateArg::from(15_i64)]).color(Color::DARK_GRAY),
        ],
    );
    advanced_map.stack = advanced_map.stack.set(MAP.id);
    advanced_map.detail = TooltipDetail::Advanced;
    let mut unknown_map = case(
        "unknown map",
        "minecraft:filled_map",
        vec![
            named(Color::WHITE, key("item.minecraft.filled_map")),
            key("filled_map.unknown").color(Color::GRAY),
        ],
    );
    unknown_map.stack = unknown_map.stack.set(MapId(4));
    let mut head = case(
        "named head",
        "minecraft:player_head",
        vec![
            named(
                Color::YELLOW,
                Text::translate(
                    "block.minecraft.player_head.named",
                    vec![Text::text("Steve")],
                ),
            ),
            key("component.profile.dynamic").color(Color::GRAY),
        ],
    );
    head.stack = head
        .stack
        .set(Profile::named("Steve").expect("a valid player name"));
    vec![helmet, map, advanced_map, unknown_map, head]
}

#[test]
fn the_parts_of_a_tooltip_follow_the_groups_of_the_stack() {
    let sharpness = key("enchantment.minecraft.sharpness");
    let mending = key("enchantment.minecraft.mending");
    let vanishing = key("enchantment.minecraft.vanishing_curse");
    let enchantments = [
        EnchantmentEntry {
            description: &sharpness,
            max_level: 5,
            class: EnchantmentClass::Other,
        },
        EnchantmentEntry {
            description: &mending,
            max_level: 1,
            class: EnchantmentClass::Other,
        },
        EnchantmentEntry {
            description: &vanishing,
            max_level: 1,
            class: EnchantmentClass::Curse,
        },
    ];
    let cases = plain_cases()
        .into_iter()
        .chain(tool_cases())
        .chain(other_cases())
        .chain(modifier_cases());
    for case in cases {
        let mut inputs = TooltipInputs::new(PLAYER, CORPUS.prototype_components(case.item));
        inputs.detail = case.detail;
        inputs.view = case.view;
        inputs.availability = case.availability;
        inputs.enchantments = &enchantments;
        inputs.tooltip_order = &[MENDING, SHARPNESS];
        inputs.maps = &[MAP];
        let mut parts = vec![Text::text("kept")];
        append_tooltip_parts(
            &case.stack,
            &CORPUS.items,
            &CORPUS.names,
            &inputs,
            &mut parts,
        );
        let expected = [vec![Text::text("kept")], case.expected].concat();
        assert_eq!(parts, expected, "{}", case.name);
    }
}
