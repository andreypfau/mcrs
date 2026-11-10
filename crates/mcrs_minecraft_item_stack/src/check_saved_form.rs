use core::ops::RangeInclusive;

use crate::BlockSet;
use crate::Builtin;
use crate::BuiltinKey;
use crate::ItemStack;
use crate::NameLookup;
use crate::NameTable;
use crate::PatchEntry;
use crate::WireError;

pub(crate) const MAX_COUNT: u8 = 99;
pub(crate) const NOT_NEGATIVE: RangeInclusive<i32> = 0..=i32::MAX;
pub(crate) const POSITIVE: RangeInclusive<i32> = 1..=i32::MAX;
pub(crate) const STACK_SIZE: RangeInclusive<i32> = 1..=MAX_COUNT as i32;

/// Checks that each decoded part of the stack has a saved form: the count is at most 99, each
/// number is in the range of its saved form, and each numeric id has a name. A component that this
/// crate does not decode, or that holds a text, is not checked.
pub(crate) fn check_saved_form<L: NameLookup>(
    stack: &ItemStack,
    names: &L,
) -> Result<(), WireError> {
    if stack.count.get() > MAX_COUNT {
        return Err(WireError::CountOutOfRange);
    }
    for entry in stack.patch() {
        if let PatchEntry::Value(value) = entry {
            check_value(value, names)?;
        }
    }
    Ok(())
}

fn check_value<L: NameLookup>(value: &Builtin, names: &L) -> Result<(), WireError> {
    let check = Check {
        component: value.key(),
        names,
    };
    match value {
        Builtin::MaxStackSize(size) => check.range(size.0, STACK_SIZE),
        Builtin::MaxDamage(damage) => check.range(damage.0, POSITIVE),
        Builtin::Damage(damage) => check.range(damage.0, NOT_NEGATIVE),
        Builtin::RepairCost(cost) => check.range(cost.0, NOT_NEGATIVE),
        Builtin::Food(food) => check.range(food.nutrition, NOT_NEGATIVE),
        Builtin::Enchantments(enchantments) => {
            for entry in enchantments.entries() {
                check.named(NameTable::Enchantment, entry.enchantment.0)?;
            }
            Ok(())
        }
        Builtin::AttributeModifiers(modifiers) => {
            for modifier in modifiers.modifiers.iter() {
                check.named(NameTable::Attribute, modifier.attribute.0)?;
            }
            Ok(())
        }
        Builtin::TooltipDisplay(display) => {
            for component_type in display.hidden_components() {
                check.named(NameTable::ComponentType, component_type.0)?;
            }
            Ok(())
        }
        Builtin::Tool(tool) => {
            check.range(tool.damage_per_block, NOT_NEGATIVE)?;
            for rule in tool.rules.iter() {
                match &rule.blocks {
                    BlockSet::Tag(block_tag) => check.named(NameTable::BlockTag, block_tag.0)?,
                    BlockSet::Blocks(blocks) => {
                        for block in blocks.iter() {
                            check.named(NameTable::Block, block.0)?;
                        }
                    }
                }
            }
            Ok(())
        }
        Builtin::Unbreakable(_)
        | Builtin::ItemModel(_)
        | Builtin::Rarity(_)
        | Builtin::CustomName(_)
        | Builtin::ItemName(_)
        | Builtin::Lore(_) => Ok(()),
    }
}

struct Check<'names, L> {
    component: BuiltinKey,
    names: &'names L,
}

impl<L: NameLookup> Check<'_, L> {
    fn range(&self, number: i32, range: RangeInclusive<i32>) -> Result<(), WireError> {
        if range.contains(&number) {
            Ok(())
        } else {
            Err(WireError::OutOfRange(self.component))
        }
    }

    fn named(&self, table: NameTable, id: u16) -> Result<(), WireError> {
        match self.names.name(table, id) {
            Some(_) => Ok(()),
            None => Err(WireError::UnknownEntry {
                component: self.component,
                table,
            }),
        }
    }
}
