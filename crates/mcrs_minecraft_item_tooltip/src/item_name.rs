use alloc::vec;

use mcrs_minecraft_item_stack::CustomName;
use mcrs_minecraft_item_stack::ItemName;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_stack::Profile;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_profile::ProfileIdentity;
use mcrs_minecraft_text::Text;

#[must_use]
pub fn item_name(stack: &ItemStack, items: &ItemTable) -> Option<Text> {
    if let Some(CustomName(name)) = items.effective::<CustomName>(stack) {
        return Some(name.clone());
    }
    head_name(stack, items).or_else(|| {
        items
            .effective::<ItemName>(stack)
            .map(|ItemName(name)| name.clone())
    })
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
