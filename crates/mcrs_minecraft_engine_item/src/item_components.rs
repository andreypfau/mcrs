use alloc::vec;
use alloc::vec::Vec;

use bevy_ecs::component::Component;
use bevy_ecs::component::StorageType;
use bevy_ecs::resource::Resource;
use bevy_ecs::world::FromWorld;
use bevy_ecs::world::World;
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
use mcrs_minecraft_item_stack::Rarity;
use mcrs_minecraft_item_stack::RepairCost;
use mcrs_minecraft_item_stack::Tool;
use mcrs_minecraft_item_stack::TooltipDisplay;
use mcrs_minecraft_item_stack::Unbreakable;

use crate::RegisterError;
use crate::column::Column;
use crate::row::Row;
use crate::row::RowKey;

const MAX_ROWS: usize = 128;

/// The rows of the item component types of a world: each built-in type at the index of its key,
/// then the entries without a type, then each registered custom type.
#[derive(Resource)]
pub(crate) struct ItemComponents {
    rows: Vec<Row>,
}

impl FromWorld for ItemComponents {
    fn from_world(world: &mut World) -> Self {
        let rows = vec![
            builtin::<MaxStackSize>(world, BuiltinKey::MaxStackSize),
            builtin::<MaxDamage>(world, BuiltinKey::MaxDamage),
            builtin::<Damage>(world, BuiltinKey::Damage),
            builtin::<Unbreakable>(world, BuiltinKey::Unbreakable),
            builtin::<CustomName>(world, BuiltinKey::CustomName),
            builtin::<ItemName>(world, BuiltinKey::ItemName),
            builtin::<ItemModel>(world, BuiltinKey::ItemModel),
            builtin::<Lore>(world, BuiltinKey::Lore),
            builtin::<Rarity>(world, BuiltinKey::Rarity),
            builtin::<Enchantments>(world, BuiltinKey::Enchantments),
            builtin::<AttributeModifiers>(world, BuiltinKey::AttributeModifiers),
            builtin::<TooltipDisplay>(world, BuiltinKey::TooltipDisplay),
            builtin::<RepairCost>(world, BuiltinKey::RepairCost),
            builtin::<Food>(world, BuiltinKey::Food),
            builtin::<Tool>(world, BuiltinKey::Tool),
            Row::undecoded(Column::register(world)),
        ];
        Self { rows }
    }
}

impl ItemComponents {
    pub(crate) fn rows(&self) -> &[Row] {
        &self.rows
    }

    pub(crate) fn builtin(&self, key: BuiltinKey) -> Option<&Row> {
        self.rows
            .get(key.index())
            .filter(|row| row.key() == RowKey::Builtin(key))
    }

    pub(crate) fn custom(&self, key: &str) -> Option<&Row> {
        self.rows
            .iter()
            .find(|row| matches!(row.key(), RowKey::Custom(custom) if custom == key))
    }

    pub(crate) fn register<C: CustomComponent + Component>(
        &mut self,
        world: &mut World,
    ) -> Result<(), RegisterError> {
        if C::STORAGE_TYPE != StorageType::Table {
            return Err(RegisterError::UnsupportedStorage);
        }
        if self.custom(C::KEY).is_some() {
            return Err(RegisterError::DuplicateKey);
        }
        if self.rows.len() >= MAX_ROWS {
            return Err(RegisterError::MaskFull);
        }
        self.rows.push(Row::custom(Column::<C>::register(world)));
        Ok(())
    }
}

fn builtin<T: ItemComponent + Component>(world: &mut World, key: BuiltinKey) -> Row {
    Row::builtin(Column::<T>::register(world), key)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn each_builtin_row_sits_at_its_key_and_fits_the_removed_bits() {
        let components = ItemComponents::from_world(&mut World::new());
        for (index, row) in components.rows().iter().enumerate() {
            if let RowKey::Builtin(key) = row.key() {
                assert_eq!(key.index(), index, "{key:?}");
                assert!(
                    !row.is_neutral() || key.index() < 32,
                    "{key:?} has a neutral value, so its removal needs a bit of `Removed`"
                );
            }
        }
    }
}
