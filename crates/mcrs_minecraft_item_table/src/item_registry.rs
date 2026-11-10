use alloc::vec;
use alloc::vec::Vec;

use mcrs_minecraft_item_stack::AttributeModifiers;
use mcrs_minecraft_item_stack::BuiltinKey;
use mcrs_minecraft_item_stack::CustomComponent;
use mcrs_minecraft_item_stack::CustomName;
use mcrs_minecraft_item_stack::Damage;
use mcrs_minecraft_item_stack::Enchantments;
use mcrs_minecraft_item_stack::Food;
use mcrs_minecraft_item_stack::ItemComponent;
use mcrs_minecraft_item_stack::ItemModel;
use mcrs_minecraft_item_stack::ItemName;
use mcrs_minecraft_item_stack::Lore;
use mcrs_minecraft_item_stack::MaxDamage;
use mcrs_minecraft_item_stack::MaxStackSize;
use mcrs_minecraft_item_stack::Neutral;
use mcrs_minecraft_item_stack::Rarity;
use mcrs_minecraft_item_stack::RepairCost;
use mcrs_minecraft_item_stack::Tool;
use mcrs_minecraft_item_stack::TooltipDisplay;
use mcrs_minecraft_item_stack::Unbreakable;

use crate::ComponentRow;
use crate::DuplicateKey;
use crate::RowHashing;

/// One row of operations for each component type: each built-in type, then each registered
/// custom type.
#[derive(Clone, Debug)]
pub struct ItemRegistry {
    builtin: Vec<BuiltinRow>,
    custom: Vec<CustomRow>,
}

#[derive(Clone, Copy, Debug)]
struct BuiltinRow {
    key: BuiltinKey,
    row: ComponentRow,
}

#[derive(Clone, Copy, Debug)]
struct CustomRow {
    key: &'static str,
    row: ComponentRow,
}

impl ItemRegistry {
    /// A registry with the rows of the built-in types and no custom type.
    #[must_use]
    pub fn new() -> Self {
        let mut builtin: Vec<BuiltinRow> = vec![
            row::<MaxStackSize>(),
            row::<MaxDamage>(),
            row::<Damage>(),
            row::<Unbreakable>(),
            row::<CustomName>(),
            row::<ItemName>(),
            row::<ItemModel>(),
            neutral_row::<Lore>(),
            neutral_row::<Rarity>(),
            neutral_row::<Enchantments>(),
            neutral_row::<AttributeModifiers>(),
            neutral_row::<TooltipDisplay>(),
            neutral_row::<RepairCost>(),
            row::<Food>(),
            row::<Tool>(),
        ]
        .into_iter()
        .flatten()
        .collect();
        builtin.sort_by_key(|entry| entry.key);
        Self {
            builtin,
            custom: Vec::new(),
        }
    }

    /// Adds the row of the custom type `C`.
    pub fn register<C: CustomComponent>(
        &mut self,
        hashing: RowHashing,
    ) -> Result<(), DuplicateKey> {
        if self.custom(C::KEY).is_some() {
            return Err(DuplicateKey);
        }
        self.custom.push(CustomRow {
            key: C::KEY,
            row: ComponentRow::custom::<C>(hashing),
        });
        Ok(())
    }

    /// The row of the built-in type `key`.
    #[must_use]
    pub fn builtin(&self, key: BuiltinKey) -> Option<&ComponentRow> {
        let index = self
            .builtin
            .binary_search_by_key(&key, |entry| entry.key)
            .ok()?;
        self.builtin.get(index).map(|entry| &entry.row)
    }

    /// The row of the custom type with the key `key`.
    #[must_use]
    pub fn custom(&self, key: &str) -> Option<&ComponentRow> {
        self.custom
            .iter()
            .find(|entry| entry.key == key)
            .map(|entry| &entry.row)
    }
}

impl Default for ItemRegistry {
    fn default() -> Self {
        Self::new()
    }
}

fn row<T: ItemComponent>() -> Option<BuiltinRow> {
    Some(BuiltinRow {
        key: T::builtin_key()?,
        row: ComponentRow::builtin::<T>(),
    })
}

fn neutral_row<T: Neutral>() -> Option<BuiltinRow> {
    Some(BuiltinRow {
        key: T::builtin_key()?,
        row: ComponentRow::builtin_with_neutral::<T>(),
    })
}
