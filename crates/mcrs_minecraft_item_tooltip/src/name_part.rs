use alloc::vec;

use mcrs_minecraft_item_stack::CustomName;
use mcrs_minecraft_item_stack::Enchantments;
use mcrs_minecraft_item_stack::ItemName;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_stack::Profile;
use mcrs_minecraft_item_stack::Rarity;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_profile::ProfileIdentity;
use mcrs_minecraft_text::Color;
use mcrs_minecraft_text::IntoText;
use mcrs_minecraft_text::Text;

pub(crate) fn name_part(stack: &ItemStack, items: &ItemTable) -> Text {
    let custom_name = items.effective::<CustomName>(stack);
    let name = match custom_name {
        Some(CustomName(name)) => name.clone(),
        None => head_name(stack, items)
            .or_else(|| {
                items
                    .effective::<ItemName>(stack)
                    .map(|ItemName(name)| name.clone())
            })
            .unwrap_or_default(),
    };
    let part = Text::default()
        .color(rarity_color(stack, items))
        .add_child(name);
    match custom_name {
        Some(_) => part.italic(),
        None => part,
    }
}

fn head_name(stack: &ItemStack, items: &ItemTable) -> Option<Text> {
    let item = items.name(NameTable::Item, stack.kind.0)?;
    if item.namespace() != "minecraft" || item.path() != "player_head" {
        return None;
    }
    let player = match &items.effective::<Profile>(stack)?.profile {
        ProfileIdentity::Full(profile) => &profile.name,
        ProfileIdentity::Partial { name, .. } => name.as_ref()?,
    };
    Some(Text::translate(
        "block.minecraft.player_head.named",
        vec![Text::text(player.0.clone())],
    ))
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
