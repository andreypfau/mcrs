use bevy_ecs::component::Component;
use bevy_ecs::entity::Entity;
use bevy_ecs::world::{EntityRef, EntityWorldMut, World};
use mcrs_minecraft_core::ResourceKey;
use mcrs_minecraft_core::codec::Bounded;
use mcrs_minecraft_protocol::item::for_each_data_component;
use mcrs_minecraft_protocol::item::*;
use mcrs_minecraft_registry::ItemId;

use crate::definition::{ItemEntry, Items};
use crate::held::SlotTable;
use crate::mutate;
use crate::stack::{ItemStack, StackRevision};

pub const CHILD_KINDS: [ItemComponentKind; 3] = [
    ItemComponentKind::Container,
    ItemComponentKind::BundleContents,
    ItemComponentKind::ChargedProjectiles,
];

pub const fn is_child_kind(kind: ItemComponentKind) -> bool {
    let mut i = 0;
    while i < CHILD_KINDS.len() {
        if CHILD_KINDS[i] as u16 == kind as u16 {
            return true;
        }
        i += 1;
    }
    false
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
    #[error("{item} cannot carry {kind}")]
    UnsupportedChildKind {
        item: String,
        kind: ItemComponentKind,
    },
    #[error("{item} has {slots} container slots, the value needs {needed}")]
    ContainerOverflow {
        item: String,
        slots: usize,
        needed: usize,
    },
    #[error("{item} has count {count}, outside 1..={}", u8::MAX)]
    BadCount { item: String, count: i32 },
}

pub(crate) struct KindOps {
    pub read: fn(EntityRef) -> Option<ItemComponentValue>,
    pub insert: fn(&mut EntityWorldMut, &ItemComponentValue),
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
    entity.get::<K>().cloned().map(K::into_value)
}

fn insert_plain<K: ItemDataComponent + Component>(
    entity: &mut EntityWorldMut,
    value: &ItemComponentValue,
) {
    let value = K::from_value(value)
        .unwrap_or_else(|| panic!("{} written as {:?}", K::KIND, value.kind()))
        .clone();
    entity.insert(value);
}

fn remove_plain<K: ItemDataComponent + Component>(entity: &mut EntityWorldMut) {
    if entity.contains::<K>() {
        entity.remove::<K>();
    }
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
    ($($id:literal $name:literal : $ty:ident [$($flag:ident),*]),* $(,)?) => {
        static OPS: [KindOps; ItemComponentKind::COUNT] = [$(KindOps::plain::<$ty>()),*];
    };
}

for_each_data_component!(kind_ops);

pub(crate) fn ops(kind: ItemComponentKind) -> &'static KindOps {
    &OPS[kind as usize]
}

fn entry<'a>(world: &World, stack: Entity, items: &'a Items) -> Result<&'a ItemEntry, StackError> {
    let item = world
        .get::<ItemStack>(stack)
        .ok_or(StackError::NotAStack(stack))?
        .item;
    items
        .get(item)
        .ok_or_else(|| StackError::UnknownItem(format!("#{}", item.0)))
}

fn named_entry<'a>(items: &'a Items, value: &ItemStackValue) -> Result<&'a ItemEntry, StackError> {
    items
        .id_of(value.item.as_str())
        .and_then(|id| items.get(id))
        .ok_or_else(|| StackError::UnknownItem(value.item.as_str().to_owned()))
}

pub(crate) fn child_kind(entry: &ItemEntry) -> Option<ItemComponentKind> {
    CHILD_KINDS
        .into_iter()
        .find(|kind| entry.prototype.get_value(*kind).is_some())
}

// ponytail: the campfire's four slots are not in the block corpus, so its
// stacks accept the codec bound; upgrade by dumping block-entity slot counts.
fn container_slots(entry: &ItemEntry) -> usize {
    entry
        .container_slots
        .map_or(MAX_CONTAINER_SLOTS, usize::from)
}

fn children(world: &World, stack: Entity) -> Vec<Option<Entity>> {
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
    kind: ItemComponentKind,
    items: &Items,
) -> ItemComponentValue {
    let mut templates: Vec<Option<Template>> = children(world, stack)
        .into_iter()
        .map(|child| child.map(|child| Template(stack_to_value(world, child, items))))
        .collect();
    let dense = || templates.iter().flatten().cloned().collect();
    match kind {
        ItemComponentKind::Container => {
            let occupied = templates
                .iter()
                .rposition(Option::is_some)
                .map_or(0, |last| last + 1);
            templates.truncate(occupied);
            Container::new(templates)
                .expect("a slot table never outgrows the container bound")
                .into()
        }
        ItemComponentKind::BundleContents => BundleContents(dense()).into(),
        ItemComponentKind::ChargedProjectiles => ChargedProjectiles::new(dense())
            .expect("a slot table never outgrows the projectile bound")
            .into(),
        other => unreachable!("{other} holds no child stacks"),
    }
}

fn child_targets(value: &ItemComponentValue) -> Vec<Option<&ItemStackValue>> {
    match value {
        ItemComponentValue::Container(container) => container
            .slots()
            .iter()
            .map(|slot| slot.as_ref().map(|template| &template.0))
            .collect(),
        ItemComponentValue::BundleContents(list) => {
            list.0.iter().map(|template| Some(&template.0)).collect()
        }
        ItemComponentValue::ChargedProjectiles(list) => list
            .items()
            .iter()
            .map(|template| Some(&template.0))
            .collect(),
        other => unreachable!("{} holds no child stacks", other.kind()),
    }
}

pub fn stack_to_value(world: &World, stack: Entity, items: &Items) -> ItemStackValue {
    let entry = entry(world, stack, items).expect("a stack entity names a corpus item");
    let count = world.get::<ItemStack>(stack).unwrap().count;
    let entity = world.entity(stack);
    let own_child_kind = child_kind(entry);
    let mut components = ComponentPatch::EMPTY;
    for kind in ItemComponentKind::ALL {
        let prototype = entry.prototype.get_value(kind);
        if Some(kind) == own_child_kind {
            if !entity.contains::<SlotTable>() {
                components.removed.push(kind);
                continue;
            }
            let derived = child_value(world, stack, kind, items);
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
        unreachable!(
            "{} holds child stacks without a child kind",
            entry.identifier
        );
    }
    ItemStackValue {
        item: ResourceKey::from_location(entry.identifier.clone()),
        count: Bounded(i32::from(count)),
        components,
    }
}

pub fn stack_to_slot(world: &World, stack: Entity, items: &Items) -> ProtoStack {
    let value = stack_to_value(world, stack, items);
    let item = world.get::<ItemStack>(stack).unwrap().item;
    ProtoStack::new(item, value.count.0, value.components)
}

pub fn spawn_stack(
    world: &mut World,
    value: &ItemStackValue,
    items: &Items,
) -> Result<Entity, StackError> {
    let entry = named_entry(items, value)?;
    check(entry, value, items)?;
    let stack = spawn_checked(world, entry, value, items);
    mutate::bump(world, stack);
    Ok(stack)
}

/// Prototype normalisation: a patch entry equal to the prototype is dropped,
/// a removal of a kind the prototype lacks is dropped, and child stacks are
/// reconciled in place. The value is checked in full before anything is
/// written, so an error leaves the world untouched.
pub fn apply_value(
    world: &mut World,
    stack: Entity,
    value: &ItemStackValue,
    items: &Items,
) -> Result<(), StackError> {
    let entry = entry(world, stack, items)?;
    if entry.identifier != *value.item.location() {
        return Err(StackError::ItemMismatch {
            stack,
            held: entry.identifier.as_str().to_owned(),
            given: value.item.as_str().to_owned(),
        });
    }
    check(entry, value, items)?;
    write_value(world, stack, entry, value, items);
    mutate::bump(world, stack);
    Ok(())
}

fn check(entry: &ItemEntry, value: &ItemStackValue, items: &Items) -> Result<(), StackError> {
    if u8::try_from(value.count.0).is_err() || value.count.0 == 0 {
        return Err(StackError::BadCount {
            item: entry.identifier.as_str().to_owned(),
            count: value.count.0,
        });
    }
    let own_child_kind = child_kind(entry);
    for kind in CHILD_KINDS {
        let Some(given) = value.components.get_value(kind) else {
            continue;
        };
        let targets = child_targets(given);
        // ponytail: vanilla lets any item carry a child-kind component; here
        // children are entities under the prototype's kind, so a foreign one
        // with contents is refused rather than kept as inert data.
        if Some(kind) != own_child_kind {
            if !targets.is_empty() {
                return Err(StackError::UnsupportedChildKind {
                    item: entry.identifier.as_str().to_owned(),
                    kind,
                });
            }
            continue;
        }
        // ponytail: vanilla keeps container slots past the block's size on the
        // item and drops them on placement; the fixed table refuses them here.
        if kind == ItemComponentKind::Container && targets.len() > container_slots(entry) {
            return Err(StackError::ContainerOverflow {
                item: entry.identifier.as_str().to_owned(),
                slots: container_slots(entry),
                needed: targets.len(),
            });
        }
        for target in targets.into_iter().flatten() {
            check(named_entry(items, target)?, target, items)?;
        }
    }
    Ok(())
}

fn spawn_checked(
    world: &mut World,
    entry: &ItemEntry,
    value: &ItemStackValue,
    items: &Items,
) -> Entity {
    let stack = world
        .spawn((
            ItemStack {
                item: entry.id,
                count: value.count.0 as u8,
            },
            StackRevision::default(),
        ))
        .id();
    write_value(world, stack, entry, value, items);
    stack
}

fn write_value(
    world: &mut World,
    stack: Entity,
    entry: &ItemEntry,
    value: &ItemStackValue,
    items: &Items,
) {
    let own_child_kind = child_kind(entry);
    let effective = entry.prototype.apply(&value.components);
    let mut entity = world.entity_mut(stack);
    for kind in ItemComponentKind::ALL {
        if Some(kind) == own_child_kind {
            continue;
        }
        match effective.get_value(kind) {
            Some(given) => (ops(kind).insert)(&mut entity, given),
            None => (ops(kind).remove)(&mut entity),
        }
    }
    if let Some(kind) = own_child_kind {
        let removed = value.components.is_removed(kind);
        let targets = match value.components.get_value(kind) {
            Some(given) if !removed => child_targets(given),
            _ => Vec::new(),
        };
        reconcile_children(world, stack, kind, entry, &targets, items);
        // A bundle or box without its contents component refuses insertion in
        // vanilla, so the tombstone leaves no cells to move into.
        if removed {
            world.entity_mut(stack).remove::<SlotTable>();
        }
    }
    world.get_mut::<ItemStack>(stack).unwrap().count = value.count.0 as u8;
}

fn reconcile_children(
    world: &mut World,
    stack: Entity,
    kind: ItemComponentKind,
    entry: &ItemEntry,
    targets: &[Option<&ItemStackValue>],
    items: &Items,
) {
    if !world.entity(stack).contains::<SlotTable>() {
        let table = match kind {
            ItemComponentKind::Container => SlotTable::fixed(container_slots(entry)),
            ItemComponentKind::ChargedProjectiles => SlotTable::fixed(MAX_CHARGED_PROJECTILES),
            _ => SlotTable::list(),
        };
        world.entity_mut(stack).insert(table);
    }
    let existing = children(world, stack);
    for (index, (child, target)) in existing
        .iter()
        .copied()
        .chain(std::iter::repeat(None))
        .zip(targets.iter().copied().chain(std::iter::repeat(None)))
        .take(existing.len().max(targets.len()))
        .enumerate()
    {
        let target_entry =
            target.map(|target| named_entry(items, target).expect("checked before the write"));
        let kept = match (child, target_entry) {
            (Some(child), Some(target_entry)) => item_of(world, child) == Some(target_entry.id),
            (None, None) => true,
            _ => false,
        };
        if kept {
            if let (Some(child), Some(target), Some(target_entry)) = (child, target, target_entry) {
                write_value(world, child, target_entry, target, items);
                mutate::touch(world, child);
            }
            continue;
        }
        if let Some(child) = child {
            world.despawn(child);
        }
        if let (Some(target), Some(target_entry)) = (target, target_entry) {
            let child = spawn_checked(world, target_entry, target, items);
            mutate::attach(world, child, stack, index as u16);
        }
    }
}

pub fn same_item_same_components(world: &World, a: Entity, b: Entity, items: &Items) -> bool {
    let item = |stack: Entity| world.get::<ItemStack>(stack).map(|stack| stack.item);
    item(a).is_some()
        && item(a) == item(b)
        && stack_to_value(world, a, items).components == stack_to_value(world, b, items).components
}

pub(crate) fn item_of(world: &World, stack: Entity) -> Option<ItemId> {
    world.get::<ItemStack>(stack).map(|stack| stack.item)
}
