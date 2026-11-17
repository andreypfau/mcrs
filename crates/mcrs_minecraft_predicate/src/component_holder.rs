use mcrs_minecraft_item_stack::AttributeModifiers;
use mcrs_minecraft_item_stack::Builtin;
use mcrs_minecraft_item_stack::CustomName;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::Enchantments;
use mcrs_minecraft_item_stack::Food;
use mcrs_minecraft_item_stack::ItemComponent;
use mcrs_minecraft_item_stack::ItemModel;
use mcrs_minecraft_item_stack::ItemName;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::Lore;
use mcrs_minecraft_item_stack::MapDecorations;
use mcrs_minecraft_item_stack::MapId;
use mcrs_minecraft_item_stack::MaxDamage;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_stack::Rarity;
use mcrs_minecraft_item_stack::RepairCost;
use mcrs_minecraft_item_stack::Tool;
use mcrs_minecraft_item_stack::TooltipDisplay;
use mcrs_minecraft_item_stack::Unbreakable;
use mcrs_minecraft_item_table::ItemTable;

use mcrs_minecraft_item::data_component_type_keys::DataComponentType;

#[derive(Clone, Copy, Debug)]
pub(crate) enum ComponentHolder<'a> {
    Empty,
    Stack {
        stack: &'a ItemStack,
        table: &'a ItemTable,
    },
}

impl<'a> ComponentHolder<'a> {
    pub(crate) fn effective<T: ItemComponent + 'static>(self) -> Option<&'a T> {
        match self {
            Self::Empty => None,
            Self::Stack { stack, table } => table.effective(stack),
        }
    }

    pub(crate) fn has(self, kind: DataComponentType) -> bool {
        match kind {
            DataComponentType::MaxStackSize => self.effective::<MaxStackSize>().is_some(),
            DataComponentType::MaxDamage => self.effective::<MaxDamage>().is_some(),
            DataComponentType::Damage => self.effective::<Damage>().is_some(),
            DataComponentType::Unbreakable => self.effective::<Unbreakable>().is_some(),
            DataComponentType::CustomName => self.effective::<CustomName>().is_some(),
            DataComponentType::ItemName => self.effective::<ItemName>().is_some(),
            DataComponentType::ItemModel => self.effective::<ItemModel>().is_some(),
            DataComponentType::Lore => self.effective::<Lore>().is_some(),
            DataComponentType::Rarity => self.effective::<Rarity>().is_some(),
            DataComponentType::Enchantments => self.effective::<Enchantments>().is_some(),
            DataComponentType::AttributeModifiers => {
                self.effective::<AttributeModifiers>().is_some()
            }
            DataComponentType::TooltipDisplay => self.effective::<TooltipDisplay>().is_some(),
            DataComponentType::RepairCost => self.effective::<RepairCost>().is_some(),
            DataComponentType::Food => self.effective::<Food>().is_some(),
            DataComponentType::Tool => self.effective::<Tool>().is_some(),
            DataComponentType::MapId => self.effective::<MapId>().is_some(),
            DataComponentType::MapDecorations => self.effective::<MapDecorations>().is_some(),
            _ => false,
        }
    }

    pub(crate) fn has_equal(self, expected: &Builtin) -> bool {
        match expected {
            Builtin::MaxStackSize(value) => self.effective() == Some(value),
            Builtin::MaxDamage(value) => self.effective() == Some(value),
            Builtin::Damage(value) => self.effective() == Some(value),
            Builtin::Unbreakable(value) => self.effective() == Some(value),
            Builtin::CustomName(value) => self.effective() == Some(value),
            Builtin::ItemName(value) => self.effective() == Some(value),
            Builtin::ItemModel(value) => self.effective() == Some(value),
            Builtin::Lore(value) => self.effective() == Some(value),
            Builtin::Rarity(value) => self.effective() == Some(value),
            Builtin::Enchantments(value) => self.effective() == Some(value),
            Builtin::AttributeModifiers(value) => self.effective() == Some(value),
            Builtin::TooltipDisplay(value) => self.effective() == Some(value),
            Builtin::RepairCost(value) => self.effective() == Some(value),
            Builtin::Food(value) => self.effective() == Some(value),
            Builtin::Tool(value) => self.effective() == Some(value),
            Builtin::MapId(value) => self.effective() == Some(value),
            Builtin::MapDecorations(value) => self.effective() == Some(value),
            _ => false,
        }
    }
}
