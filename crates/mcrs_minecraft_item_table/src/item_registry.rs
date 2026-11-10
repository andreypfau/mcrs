use alloc::vec;
use alloc::vec::Vec;

use mcrs_minecraft_item_stack::BuiltinKey;
use mcrs_minecraft_item_stack::CustomComponent;
use mcrs_minecraft_item_stack::ItemComponent;

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
        macro_rules! rows {
            ($($name:ident => $path:literal),* $(,)?) => {
                vec![$(row::<mcrs_minecraft_item_stack::$name>(),)*]
            };
        }
        let mut builtin: Vec<BuiltinRow> = mcrs_minecraft_item_stack::for_each_builtin!(rows)
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

fn row<T: ItemComponent + 'static>() -> Option<BuiltinRow> {
    Some(BuiltinRow {
        key: T::builtin_key()?,
        row: ComponentRow::builtin::<T>(),
    })
}
