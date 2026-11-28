use alloc::format;
use alloc::vec;
use alloc::vec::Vec;

use mcrs_minecraft_entity_attribute::ModifierOperation;
use mcrs_minecraft_entity_attribute::attribute_keys::Attribute;
use mcrs_minecraft_item_stack::AttributeModifier;
use mcrs_minecraft_item_stack::ModifierDisplay;
use mcrs_minecraft_item_stack::SlotGroup;
use mcrs_minecraft_text::Color;
use mcrs_minecraft_text::IntoText;
use mcrs_minecraft_text::Text;
use mcrs_minecraft_text::TranslateArg;

use crate::AttackBases;
use crate::decimal::decimal;

#[derive(Clone, Copy)]
enum Change {
    Increase,
    Decrease,
}

pub(crate) fn attribute_parts(
    modifiers: &[AttributeModifier],
    attack: AttackBases,
    parts: &mut Vec<Text>,
) {
    for group in SlotGroup::ALL {
        let mut shown = modifiers
            .iter()
            .filter(|modifier| modifier.slot == group)
            .filter(|modifier| modifier.display != ModifierDisplay::Hidden)
            .peekable();
        if shown.peek().is_some() {
            parts.push(Text::default());
            let header = format!("item.modifiers.{}", group.name());
            parts.push(Text::translate(header, Vec::new()).color(Color::GRAY));
        }
        for modifier in shown {
            if let ModifierDisplay::Override(text) = &modifier.display {
                parts.push(text.clone());
            } else {
                parts.extend(default_part(modifier, attack));
            }
        }
    }
}

fn default_part(modifier: &AttributeModifier, attack: AttackBases) -> Option<Text> {
    let attribute = Attribute::from_protocol_id(modifier.attribute.0)?;
    let base = base_value(modifier, attack);
    let value = modifier.amount + base.unwrap_or(0.0);
    let shown = shown_number(value, modifier.operation, attribute);
    let operation = u8::from(modifier.operation);
    let name = Text::translate(
        format!("attribute.name.{}", attribute.location().path()),
        Vec::new(),
    );
    if base.is_some() {
        let equals = line("equals", operation, shown, name);
        return Some(Text::text(" ").color(Color::DARK_GREEN).add_child(equals));
    }
    let (kind, number, change) = if value > 0.0 {
        ("plus", shown, Change::Increase)
    } else if value < 0.0 {
        ("take", -shown, Change::Decrease)
    } else {
        return None;
    };
    Some(line(kind, operation, number, name).color(change_color(attribute, change)))
}

fn line(kind: &str, operation: u8, number: f64, name: Text) -> Text {
    Text::translate_args(
        format!("attribute.modifier.{kind}.{operation}"),
        vec![
            TranslateArg::Text(Text::text(decimal(number))),
            TranslateArg::Text(name),
        ],
    )
}

fn base_value(modifier: &AttributeModifier, attack: AttackBases) -> Option<f64> {
    let id = modifier.id();
    if id.namespace() != "minecraft" {
        return None;
    }
    match id.path() {
        "base_attack_damage" => Some(attack.attack_damage),
        "base_attack_speed" => Some(attack.attack_speed),
        _ => None,
    }
}

fn shown_number(value: f64, operation: ModifierOperation, attribute: Attribute) -> f64 {
    match operation {
        ModifierOperation::AddMultipliedBase | ModifierOperation::AddMultipliedTotal => {
            value * 100.0
        }
        ModifierOperation::AddValue if attribute == Attribute::KnockbackResistance => value * 10.0,
        ModifierOperation::AddValue => value,
    }
}

fn change_color(attribute: Attribute, change: Change) -> Color {
    match (attribute, change) {
        (Attribute::BurningTime | Attribute::FallDamageMultiplier, Change::Increase) => Color::RED,
        (Attribute::BurningTime | Attribute::FallDamageMultiplier, Change::Decrease) => Color::BLUE,
        (
            Attribute::Gravity
            | Attribute::Scale
            | Attribute::WaypointTransmitRange
            | Attribute::WaypointReceiveRange,
            _,
        ) => Color::GRAY,
        (_, Change::Increase) => Color::BLUE,
        (_, Change::Decrease) => Color::RED,
    }
}
