use mcrs_minecraft_item_stack::CustomName;
use mcrs_minecraft_item_stack::Enchantments;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Rarity;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_text::Color;
use mcrs_minecraft_text::IntoText;
use mcrs_minecraft_text::Text;

use crate::item_name::item_name;

pub(crate) fn name_part(stack: &ItemStack, items: &ItemTable) -> Text {
    let part = Text::default()
        .color(rarity_color(stack, items))
        .add_child(item_name(stack, items).unwrap_or_default());
    match items.effective::<CustomName>(stack) {
        Some(_) => part.italic(),
        None => part,
    }
}

fn rarity_color(stack: &ItemStack, items: &ItemTable) -> Color {
    let rarity = items
        .effective::<Rarity>(stack)
        .copied()
        .unwrap_or(Rarity::Common);
    let enchanted = items
        .effective::<Enchantments>(stack)
        .is_some_and(|enchantments| !enchantments.entries().is_empty());
    match (rarity, enchanted) {
        (Rarity::Common, false) => Color::WHITE,
        (Rarity::Uncommon, false) => Color::YELLOW,
        (Rarity::Rare, false) | (Rarity::Common | Rarity::Uncommon, true) => Color::AQUA,
        (Rarity::Epic, _) | (Rarity::Rare, true) => Color::LIGHT_PURPLE,
    }
}
