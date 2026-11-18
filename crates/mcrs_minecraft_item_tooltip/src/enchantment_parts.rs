use alloc::format;
use alloc::vec::Vec;

use mcrs_minecraft_item_stack::EnchantmentLevel;
use mcrs_minecraft_text::Color;
use mcrs_minecraft_text::Text;

use crate::EnchantmentClass;
use crate::TooltipInputs;

pub(crate) fn enchantment_parts(
    levels: &[EnchantmentLevel],
    inputs: &TooltipInputs<'_>,
    parts: &mut Vec<Text>,
) {
    for enchantment in inputs.tooltip_order {
        let ordered = levels
            .iter()
            .find(|level| level.enchantment == *enchantment);
        parts.extend(ordered.and_then(|level| enchantment_part(*level, inputs)));
    }
    let others = levels
        .iter()
        .filter(|level| !inputs.tooltip_order.contains(&level.enchantment));
    parts.extend(others.filter_map(|level| enchantment_part(*level, inputs)));
}

fn enchantment_part(level: EnchantmentLevel, inputs: &TooltipInputs<'_>) -> Option<Text> {
    let entry = inputs.enchantments.get(usize::from(level.enchantment.0))?;
    let mut part = entry.description.clone();
    if part.style.color.is_none() {
        part.style.color = Some(match entry.class {
            EnchantmentClass::Curse => Color::RED,
            EnchantmentClass::Other => Color::GRAY,
        });
    }
    if level.level.get() != 1 || entry.max_level != 1 {
        part.extra.push(Text::text(" "));
        part.extra.push(Text::translate(
            format!("enchantment.level.{}", level.level),
            Vec::new(),
        ));
    }
    Some(part)
}
