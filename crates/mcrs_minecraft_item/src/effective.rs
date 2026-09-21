use std::borrow::Cow;

use bevy_ecs::entity::Entity;
use bevy_ecs::query::QueryData;
use bevy_ecs::world::EntityRef;
use mcrs_minecraft_protocol::item::{
    Bees, Damage, EnchantmentGlintOverride, Enchantments, ItemComponentKind, ItemComponentValue,
    ItemDataComponent, MaxDamage, MaxStackSize, Unbreakable,
};

use crate::definition::Items;
use crate::held::SlotTable;
use crate::patch::Patch;
use crate::stack::ItemStack;
use crate::value::ops;

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

/// `Some(None)` is a tombstone: the prototype's value is removed on this stack.
pub fn patched_value(entity: EntityRef, kind: ItemComponentKind) -> Option<Option<ItemComponentValue>> {
    (ops(kind).read)(entity)
}

pub fn effective_value<'a>(
    entity: EntityRef<'a>,
    items: &'a Items,
    kind: ItemComponentKind,
) -> Option<Cow<'a, ItemComponentValue>> {
    match patched_value(entity, kind) {
        Some(patch) => patch.map(Cow::Owned),
        None => items
            .get(entity.get::<ItemStack>()?.item)?
            .prototype
            .get_value(kind)
            .map(Cow::Borrowed),
    }
}

pub fn max_damage(entity: EntityRef, items: &Items) -> i32 {
    effective::<MaxDamage>(entity, items).map_or(0, |max| max.0.0)
}

pub fn damage_value(entity: EntityRef, items: &Items) -> i32 {
    effective::<Damage>(entity, items)
        .map_or(0, |damage| damage.0.0)
        .clamp(0, max_damage(entity, items))
}

pub fn is_damaged(entity: EntityRef, items: &Items) -> bool {
    is_damageable(entity, items) && damage_value(entity, items) > 0
}

pub fn next_damage_will_break(entity: EntityRef, items: &Items) -> bool {
    is_damageable(entity, items) && damage_value(entity, items) >= max_damage(entity, items) - 1
}

pub fn is_enchanted(entity: EntityRef, items: &Items) -> bool {
    effective::<Enchantments>(entity, items).is_some_and(|enchantments| !enchantments.0.is_empty())
}

pub fn has_foil(entity: EntityRef, items: &Items) -> bool {
    if let Some(EnchantmentGlintOverride(foil)) = effective::<EnchantmentGlintOverride>(entity, items) {
        return *foil;
    }
    // ponytail: the only vanilla override is the compass with a lodestone
    // tracker; a `foil_when_has` field in the dumped corpus is the upgrade.
    let compass = entity
        .get::<ItemStack>()
        .and_then(|stack| items.get(stack.item))
        .is_some_and(|entry| entry.identifier.as_str() == "minecraft:compass");
    if compass && effective_value(entity, items, ItemComponentKind::LodestoneTracker).is_some() {
        return true;
    }
    is_enchanted(entity, items)
}

/// The child stacks of a container, bundle or crossbow, in cell order.
pub fn children<'a>(
    entity: EntityRef<'a>,
    lookup: &impl Fn(Entity) -> Option<EntityRef<'a>>,
) -> Vec<EntityRef<'a>> {
    entity
        .get::<SlotTable>()
        .map(|table| table.iter().filter_map(|(_, child)| lookup(child)).collect())
        .unwrap_or_default()
}

/// A bundle's fill fraction: each child weighs `count / max_stack_size`, a
/// nested bundle its own weight plus 1/16, and a hive with bees a full slot.
pub fn bundle_weight<'a>(
    entity: EntityRef<'a>,
    items: &Items,
    lookup: &impl Fn(Entity) -> Option<EntityRef<'a>>,
) -> f32 {
    children(entity, lookup)
        .into_iter()
        .map(|child| {
            let count = child.get::<ItemStack>().map_or(0, |stack| stack.count) as f32;
            let weight = if effective_value(child, items, ItemComponentKind::BundleContents).is_some() {
                bundle_weight(child, items, lookup) + 1.0 / 16.0
            } else if effective::<Bees>(child, items).is_some_and(|bees| !bees.0.is_empty()) {
                1.0
            } else {
                1.0 / max_stack_size(child, items) as f32
            };
            count * weight
        })
        .sum()
}
