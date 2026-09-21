use bevy_ecs::entity::Entity;
use bevy_ecs::world::EntityRef;
use mcrs_minecraft_protocol::item::{
    Bees, Damage, EnchantmentGlintOverride, Enchantments, ItemComponentKind, ItemComponentValue,
    LodestoneTracker, MaxDamage, MaxStackSize, Unbreakable,
};

use crate::definition::Items;
use crate::held::SlotTable;
use crate::stack::ItemStack;
use crate::value::{child_kind, ops};

fn own_child_kind(entity: EntityRef, items: &Items) -> Option<ItemComponentKind> {
    let entry = items.get(entity.get::<ItemStack>()?.item)?;
    child_kind(entry)
}

pub fn max_stack_size(entity: EntityRef) -> u8 {
    entity.get::<MaxStackSize>().map_or(1, |max| max.0.0 as u8)
}

pub fn is_damageable(entity: EntityRef) -> bool {
    entity.contains::<MaxDamage>()
        && entity.contains::<Damage>()
        && !entity.contains::<Unbreakable>()
}

pub fn is_stackable(entity: EntityRef) -> bool {
    max_stack_size(entity) > 1
        && (!is_damageable(entity) || entity.get::<Damage>().is_none_or(|damage| damage.0.0 == 0))
}

pub fn component_value(entity: EntityRef, kind: ItemComponentKind) -> Option<ItemComponentValue> {
    (ops(kind).read)(entity)
}

pub fn has_component(entity: EntityRef, items: &Items, kind: ItemComponentKind) -> bool {
    if Some(kind) == own_child_kind(entity, items) {
        return entity.contains::<SlotTable>();
    }
    (ops(kind).contains)(entity)
}

/// Vanilla's `hasNonDefault`: the stack's value for `kind` is not the
/// prototype's, a tombstone included.
pub fn has_non_default(entity: EntityRef, items: &Items, kind: ItemComponentKind) -> bool {
    let Some(entry) = entity
        .get::<ItemStack>()
        .and_then(|stack| items.get(stack.item))
    else {
        return false;
    };
    if Some(kind) == child_kind(entry) {
        return !entity.contains::<SlotTable>();
    }
    (ops(kind).differs)(entity, entry.prototype.get_value(kind))
}

pub fn max_damage(entity: EntityRef) -> i32 {
    entity.get::<MaxDamage>().map_or(0, |max| max.0.0)
}

pub fn damage_value(entity: EntityRef) -> i32 {
    entity
        .get::<Damage>()
        .map_or(0, |damage| damage.0.0)
        .clamp(0, max_damage(entity))
}

pub fn is_damaged(entity: EntityRef) -> bool {
    is_damageable(entity) && damage_value(entity) > 0
}

pub fn next_damage_will_break(entity: EntityRef) -> bool {
    is_damageable(entity) && damage_value(entity) >= max_damage(entity) - 1
}

pub fn is_enchanted(entity: EntityRef) -> bool {
    entity
        .get::<Enchantments>()
        .is_some_and(|enchantments| !enchantments.0.is_empty())
}

pub fn has_foil(entity: EntityRef, items: &Items) -> bool {
    if let Some(EnchantmentGlintOverride(foil)) = entity.get::<EnchantmentGlintOverride>() {
        return *foil;
    }
    // ponytail: the only vanilla override is the compass with a lodestone
    // tracker; a `foil_when_has` field in the dumped corpus is the upgrade.
    let compass = entity
        .get::<ItemStack>()
        .and_then(|stack| items.get(stack.item))
        .is_some_and(|entry| entry.identifier.as_str() == "minecraft:compass");
    if compass && entity.contains::<LodestoneTracker>() {
        return true;
    }
    is_enchanted(entity)
}

/// The child stacks of a container, bundle or crossbow, in cell order.
pub fn children<'a>(
    entity: EntityRef<'a>,
    lookup: &impl Fn(Entity) -> Option<EntityRef<'a>>,
) -> Vec<EntityRef<'a>> {
    entity
        .get::<SlotTable>()
        .map(|table| {
            table
                .iter()
                .filter_map(|(_, child)| lookup(child))
                .collect()
        })
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
            let weight = if has_component(child, items, ItemComponentKind::BundleContents) {
                bundle_weight(child, items, lookup) + 1.0 / 16.0
            } else if child.get::<Bees>().is_some_and(|bees| !bees.0.is_empty()) {
                1.0
            } else {
                1.0 / max_stack_size(child) as f32
            };
            count * weight
        })
        .sum()
}
