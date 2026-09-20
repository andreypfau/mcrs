use bevy_ecs::query::QueryData;
use bevy_ecs::world::EntityRef;
use mcrs_minecraft_protocol::item::{Damage, ItemDataComponent, MaxDamage, MaxStackSize, Unbreakable};

use crate::definition::Items;
use crate::patch::Patch;
use crate::stack::ItemStack;

#[derive(QueryData)]
pub struct StackComponent<K: ItemDataComponent> {
    pub stack: &'static ItemStack,
    patch: Option<&'static Patch<K>>,
}

impl<K: ItemDataComponent> StackComponentItem<'_, '_, K> {
    pub fn get<'a>(&'a self, items: &'a Items) -> Option<&'a K> {
        resolve(self.stack, self.patch, items)
    }
}

fn resolve<'a, K: ItemDataComponent>(
    stack: &'a ItemStack,
    patch: Option<&'a Patch<K>>,
    items: &'a Items,
) -> Option<&'a K> {
    match patch {
        Some(patch) => patch.0.as_ref(),
        None => items.get(stack.item)?.prototype.get::<K>(),
    }
}

pub fn effective<'a, K: ItemDataComponent>(entity: EntityRef<'a>, items: &'a Items) -> Option<&'a K> {
    resolve(entity.get::<ItemStack>()?, entity.get::<Patch<K>>(), items)
}

pub fn max_stack_size(entity: EntityRef, items: &Items) -> u8 {
    effective::<MaxStackSize>(entity, items).map_or(1, |max| max.0.0 as u8)
}

pub fn is_damageable(entity: EntityRef, items: &Items) -> bool {
    effective::<MaxDamage>(entity, items).is_some()
        && effective::<Damage>(entity, items).is_some()
        && effective::<Unbreakable>(entity, items).is_none()
}

pub fn is_stackable(entity: EntityRef, items: &Items) -> bool {
    max_stack_size(entity, items) > 1
        && (!is_damageable(entity, items)
            || effective::<Damage>(entity, items).is_none_or(|damage| damage.0.0 == 0))
}
