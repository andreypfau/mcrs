use alloc::vec::Vec;

use mcrs_minecraft_item_stack::AttributeModifiers;
use mcrs_minecraft_item_stack::Enchantments;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Lore;
use mcrs_minecraft_item_stack::MapId;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::Neutral;
use mcrs_minecraft_item_stack::Profile;
use mcrs_minecraft_item_stack::TooltipDisplay;
use mcrs_minecraft_item_stack::Unbreakable;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_text::Color;
use mcrs_minecraft_text::IntoText;
use mcrs_minecraft_text::Text;

use crate::ItemAvailability;
use crate::TooltipDetail;
use crate::TooltipInputs;
use crate::TooltipView;
use crate::advanced_parts::advanced_parts;
use crate::attribute_parts::attribute_parts;
use crate::enchantment_parts::enchantment_parts;
use crate::map_parts::map_parts;
use crate::name_part::name_part;
use crate::profile_part::profile_part;
use crate::random_painting_part::random_painting_part;
use crate::visible::Visible;

pub fn append_tooltip_parts<N: NameLookup>(
    stack: &ItemStack,
    items: &ItemTable,
    names: &N,
    inputs: &TooltipInputs<'_>,
    parts: &mut Vec<Text>,
) {
    let display = items
        .effective::<TooltipDisplay>(stack)
        .unwrap_or(<TooltipDisplay as Neutral>::NEUTRAL);
    if display.hide_tooltip && inputs.view != TooltipView::Creative {
        return;
    }
    let visible = Visible {
        stack,
        items,
        names,
        display,
    };
    parts.push(name_part(stack, items));
    if inputs.view == TooltipView::Creative {
        parts.extend(random_painting_part(&visible));
    }
    if let Some(map_id) = visible.component::<MapId>() {
        map_parts(&visible, *map_id, inputs, parts);
    }
    if let Some(enchantments) = visible.component::<Enchantments>() {
        enchantment_parts(enchantments.entries(), inputs, parts);
    }
    if let Some(profile) = visible.component::<Profile>() {
        parts.extend(profile_part(profile));
    }
    if let Some(lore) = visible.component::<Lore>() {
        let lines = lore.lines.iter().map(|line| {
            Text::default()
                .color(Color::DARK_PURPLE)
                .italic()
                .add_child(line.clone())
        });
        parts.extend(lines);
    }
    if let Some(modifiers) = visible.component::<AttributeModifiers>() {
        attribute_parts(&modifiers.modifiers, inputs.attack, parts);
    }
    if visible.component::<Unbreakable>().is_some() {
        parts.push(Text::translate("item.unbreakable", Vec::new()).color(Color::BLUE));
    }
    if inputs.detail == TooltipDetail::Advanced {
        advanced_parts(&visible, inputs.prototype_components, parts);
    }
    if inputs.availability == ItemAvailability::Disabled {
        parts.push(Text::translate("item.disabled", Vec::new()).color(Color::RED));
    }
}
