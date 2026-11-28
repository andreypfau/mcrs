use alloc::string::ToString;
use alloc::vec;
use alloc::vec::Vec;

use mcrs_minecraft_item_stack::BuiltinKey;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::MaxDamage;
use mcrs_minecraft_item_stack::NameLookup;
use mcrs_minecraft_item_stack::NameTable;
use mcrs_minecraft_item_stack::PatchEntry;
use mcrs_minecraft_item_stack::Unbreakable;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_text::Color;
use mcrs_minecraft_text::IntoText;
use mcrs_minecraft_text::Text;
use mcrs_minecraft_text::TranslateArg;

use crate::visible::Visible;

pub(crate) fn advanced_parts<N: NameLookup>(
    visible: &Visible<'_, N>,
    prototype_components: usize,
    parts: &mut Vec<Text>,
) {
    if visible.shown(BuiltinKey::Damage) {
        parts.extend(durability_part(visible.stack, visible.items));
    }
    let stack = visible.stack;
    if let Some(item) = visible.items.name(NameTable::Item, stack.kind.0) {
        parts.push(Text::text(item.to_string()).color(Color::DARK_GRAY));
    }
    let components = effective_components(stack, visible.items, prototype_components);
    if components > 0 {
        let count = i64::try_from(components).unwrap_or(i64::MAX);
        let part = Text::translate_args("item.components", vec![TranslateArg::from(count)]);
        parts.push(part.color(Color::DARK_GRAY));
    }
}

fn durability_part(stack: &ItemStack, items: &ItemTable) -> Option<Text> {
    let MaxDamage(max_damage) = *items.effective::<MaxDamage>(stack)?;
    let Damage(damage) = *items.effective::<Damage>(stack)?;
    if items.effective::<Unbreakable>(stack).is_some() {
        return None;
    }
    let damage = damage.clamp(0, max_damage.max(0));
    if damage <= 0 {
        return None;
    }
    Some(Text::translate_args(
        "item.durability",
        vec![
            TranslateArg::from(max_damage.saturating_sub(damage)),
            TranslateArg::from(max_damage),
        ],
    ))
}

fn effective_components(
    stack: &ItemStack,
    items: &ItemTable,
    prototype_components: usize,
) -> usize {
    let prototype = items.prototype(stack.kind);
    let in_prototype = |key: BuiltinKey| {
        prototype.is_some_and(|prototype| {
            prototype
                .components()
                .iter()
                .any(|component| component.key() == key)
        })
    };
    stack
        .patch()
        .iter()
        .fold(prototype_components, |count, entry| {
            let typed_in_prototype = entry.builtin_key().map(in_prototype);
            match (entry, typed_in_prototype) {
                (PatchEntry::Removal(_) | PatchEntry::UndecodedRemoval(_), Some(false)) => count,
                (PatchEntry::Removal(_) | PatchEntry::UndecodedRemoval(_), _) => {
                    count.saturating_sub(1)
                }
                (_, Some(true)) => count,
                _ => count.saturating_add(1),
            }
        })
}
