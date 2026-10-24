use crate::component::*;
use crate::keys::DataComponentType;
use crate::keys::Item;
use crate::{
    ComponentPatch, ItemComponentValue, ItemDataComponent, ItemStackValue, ProtoStack, Template,
    for_each_data_component,
};
use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::world::{EntityRef, EntityWorldMut, World};
use mcrs_minecraft_core::Id;
use mcrs_minecraft_core::codec::Bounded;

use crate::definition::{ItemEntry, Items};
use crate::held::SlotTable;
use crate::item_stack::ItemStack;

pub const CHILD_KINDS: [DataComponentType; 3] = [
    DataComponentType::Container,
    DataComponentType::BundleContents,
    DataComponentType::ChargedProjectiles,
];

pub fn is_child_kind(kind: DataComponentType) -> bool {
    CHILD_KINDS.contains(&kind)
}

#[derive(Debug, thiserror::Error)]
pub enum StackError {
    #[error("unknown item {0}")]
    UnknownItem(String),
    #[error("{0:?} is not a stack")]
    NotAStack(Entity),
    #[error("{stack:?} holds {held} and cannot become {given}")]
    ItemMismatch {
        stack: Entity,
        held: String,
        given: String,
    },
    #[error("{item} cannot carry {}", .kind.location())]
    UnsupportedChildKind {
        item: String,
        kind: DataComponentType,
    },
    #[error("{item} has {slots} container slots, the value needs {needed}")]
    ContainerOverflow {
        item: String,
        slots: usize,
        needed: usize,
    },
    #[error("{item} has count {count}, outside 1..={}", u8::MAX)]
    BadCount { item: String, count: i32 },
    #[error("{item} charges {count} projectiles, over {MAX_CHARGED_PROJECTILES}")]
    ProjectileOverflow { item: String, count: usize },
    #[error("{item} holds child stacks without a child kind")]
    OrphanChildren { item: String },
    #[error("{} written as {}", .expected.location(), .given.location())]
    KindMismatch {
        expected: DataComponentType,
        given: DataComponentType,
    },
}

#[doc(hidden)]
pub struct KindOps {
    pub read: fn(EntityRef) -> Option<ItemComponentValue>,
    pub insert: fn(&mut EntityWorldMut, &ItemComponentValue) -> Result<(), StackError>,
    pub remove: fn(&mut EntityWorldMut),
    pub contains: fn(EntityRef) -> bool,
    pub differs: fn(EntityRef, Option<&ItemComponentValue>) -> bool,
}

impl KindOps {
    const fn plain<K: ItemDataComponent + Component>() -> Self {
        KindOps {
            read: read_plain::<K>,
            insert: insert_plain::<K>,
            remove: remove_plain::<K>,
            contains: contains_plain::<K>,
            differs: differs_plain::<K>,
        }
    }
}

fn read_plain<K: ItemDataComponent + Component>(entity: EntityRef) -> Option<ItemComponentValue> {
    entity.get::<K>().cloned().map(K::into)
}

fn insert_plain<K: ItemDataComponent + Component>(
    entity: &mut EntityWorldMut,
    value: &ItemComponentValue,
) -> Result<(), StackError> {
    let given = K::from_value(value).ok_or(StackError::KindMismatch {
        expected: K::KIND,
        given: value.kind(),
    })?;
    entity.insert(given.clone());
    Ok(())
}

fn remove_plain<K: ItemDataComponent + Component>(entity: &mut EntityWorldMut) {
    entity.remove::<K>();
}

fn contains_plain<K: ItemDataComponent + Component>(entity: EntityRef) -> bool {
    entity.contains::<K>()
}

fn differs_plain<K: ItemDataComponent + Component>(
    entity: EntityRef,
    prototype: Option<&ItemComponentValue>,
) -> bool {
    entity.get::<K>() != prototype.and_then(K::from_value)
}

macro_rules! kind_ops {
    ($($kind:ident : $ty:ident [$($flag:ident),*]),* $(,)?) => {
        #[doc(hidden)]
        pub fn ops(kind: DataComponentType) -> &'static KindOps {
            match kind {
                $(DataComponentType::$kind => {
                    const OPS: KindOps = KindOps::plain::<$ty>();
                    &OPS
                })*
            }
        }
    };
}

for_each_data_component!(kind_ops);

pub fn entry<'a>(
    world: &World,
    stack: Entity,
    items: &'a Items,
) -> Result<&'a ItemEntry, StackError> {
    let item = world
        .get::<ItemStack>(stack)
        .ok_or(StackError::NotAStack(stack))?
        .item;
    items
        .get(item)
        .ok_or_else(|| StackError::UnknownItem(format!("#{}", item.number())))
}

pub fn named_entry<'a>(
    items: &'a Items,
    value: &ItemStackValue,
) -> Result<&'a ItemEntry, StackError> {
    items
        .get(value.item)
        .ok_or_else(|| StackError::UnknownItem(format!("#{}", value.item.number())))
}

pub fn child_kind(entry: &ItemEntry) -> Option<DataComponentType> {
    CHILD_KINDS
        .into_iter()
        .find(|kind| entry.prototype.get_value(*kind).is_some())
}

// chisle: the campfire's four slots are not in the block corpus, so its
// stacks accept the codec bound; upgrade by dumping block-entity slot counts.
pub fn container_slots(entry: &ItemEntry) -> usize {
    entry
        .container_slots
        .map_or(MAX_CONTAINER_SLOTS, usize::from)
}

pub fn children(world: &World, stack: Entity) -> Vec<Option<Entity>> {
    world
        .get::<SlotTable>(stack)
        .map(|table| {
            (0..table.len() as u16)
                .map(|index| table.get(index))
                .collect()
        })
        .unwrap_or_default()
}

fn child_value(
    world: &World,
    stack: Entity,
    kind: DataComponentType,
    entry: &ItemEntry,
    items: &Items,
) -> Result<ItemComponentValue, StackError> {
    let mut templates = children(world, stack)
        .into_iter()
        .map(|child| {
            child
                .map(|child| stack_to_value(world, child, items).map(Template))
                .transpose()
        })
        .collect::<Result<Vec<Option<Template>>, _>>()?;
    let item = || entry.identifier.to_string();
    match kind {
        DataComponentType::Container => {
            let occupied = templates
                .iter()
                .rposition(Option::is_some)
                .map_or(0, |last| last + 1);
            templates.truncate(occupied);
            let needed = templates.len();
            Container::new(templates)
                .map(Into::into)
                .map_err(|_| StackError::ContainerOverflow {
                    item: item(),
                    slots: MAX_CONTAINER_SLOTS,
                    needed,
                })
        }
        DataComponentType::BundleContents => {
            Ok(BundleContents(templates.into_iter().flatten().collect()).into())
        }
        DataComponentType::ChargedProjectiles => {
            let dense: Vec<Template> = templates.into_iter().flatten().collect();
            let count = dense.len();
            ChargedProjectiles::new(dense).map(Into::into).map_err(|_| {
                StackError::ProjectileOverflow {
                    item: item(),
                    count,
                }
            })
        }
        other => Err(StackError::UnsupportedChildKind {
            item: item(),
            kind: other,
        }),
    }
}

/// The stacks a child kind's value holds, or `None` for a kind that holds none.
pub fn child_targets(value: &ItemComponentValue) -> Option<Vec<Option<&ItemStackValue>>> {
    Some(match value {
        ItemComponentValue::Container(container) => container
            .slots
            .0
            .iter()
            .map(|slot| slot.as_ref().map(|template| &template.0))
            .collect(),
        ItemComponentValue::BundleContents(list) => {
            list.0.iter().map(|template| Some(&template.0)).collect()
        }
        ItemComponentValue::ChargedProjectiles(list) => list
            .items
            .0
            .iter()
            .map(|template| Some(&template.0))
            .collect(),
        _ => return None,
    })
}

pub fn stack_to_value(
    world: &World,
    stack: Entity,
    items: &Items,
) -> Result<ItemStackValue, StackError> {
    let &ItemStack { item, count, .. } = world
        .get::<ItemStack>(stack)
        .ok_or(StackError::NotAStack(stack))?;
    let entry = items
        .get(item)
        .ok_or_else(|| StackError::UnknownItem(format!("#{}", item.number())))?;
    let entity = world.entity(stack);
    let own_child_kind = child_kind(entry);
    let mut components = ComponentPatch::EMPTY;
    for kind in DataComponentType::ALL.iter().copied() {
        let prototype = entry.prototype.get_value(kind);
        if Some(kind) == own_child_kind {
            if !entity.contains::<SlotTable>() {
                components.removed.push(kind);
                continue;
            }
            let derived = child_value(world, stack, kind, entry, items)?;
            if prototype != Some(&derived) {
                components.added.push(derived);
            }
            continue;
        }
        if !(ops(kind).differs)(entity, prototype) {
            continue;
        }
        match (ops(kind).read)(entity) {
            Some(value) => components.added.push(value),
            None => components.removed.push(kind),
        }
    }
    if own_child_kind.is_none() && !children(world, stack).is_empty() {
        return Err(StackError::OrphanChildren {
            item: entry.identifier.to_string(),
        });
    }
    Ok(ItemStackValue {
        item,
        count: Bounded(i32::from(count)),
        components,
    })
}

pub fn stack_to_slot(
    world: &World,
    stack: Entity,
    items: &Items,
) -> Result<ProtoStack, StackError> {
    let value = stack_to_value(world, stack, items)?;
    Ok(ProtoStack::new(value.item, value.count.0, value.components))
}

pub fn same_item_same_components(world: &World, a: Entity, b: Entity, items: &Items) -> bool {
    let item = |stack: Entity| world.get::<ItemStack>(stack).map(|stack| stack.item);
    item(a).is_some()
        && item(a) == item(b)
        && match (
            stack_to_value(world, a, items),
            stack_to_value(world, b, items),
        ) {
            (Ok(a), Ok(b)) => a.components == b.components,
            _ => false,
        }
}

pub fn item_of(world: &World, stack: Entity) -> Option<Id<Item>> {
    world.get::<ItemStack>(stack).map(|stack| stack.item)
}
