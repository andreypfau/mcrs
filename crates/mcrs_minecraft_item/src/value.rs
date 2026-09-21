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
use crate::stack::ItemStack;

pub const CHILD_KINDS: [ItemComponentKind; 3] = [
    ItemComponentKind::Container,
    ItemComponentKind::BundleContents,
    ItemComponentKind::ChargedProjectiles,
];

pub fn is_child_kind(kind: ItemComponentKind) -> bool {
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

#[doc(hidden)]
pub struct KindOps {
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
    entity.get::<K>().cloned().map(K::into)
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

#[doc(hidden)]
pub fn ops(kind: ItemComponentKind) -> &'static KindOps {
    &OPS[kind as usize]
}

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
        .ok_or_else(|| StackError::UnknownItem(format!("#{}", item.0)))
}

pub fn named_entry<'a>(
    items: &'a Items,
    value: &ItemStackValue,
) -> Result<&'a ItemEntry, StackError> {
    items
        .id_of(value.item.as_str())
        .and_then(|id| items.get(id))
        .ok_or_else(|| StackError::UnknownItem(value.item.as_str().to_owned()))
}

pub fn child_kind(entry: &ItemEntry) -> Option<ItemComponentKind> {
    CHILD_KINDS
        .into_iter()
        .find(|kind| entry.prototype.get_value(*kind).is_some())
}

// ponytail: the campfire's four slots are not in the block corpus, so its
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

pub fn child_targets(value: &ItemComponentValue) -> Vec<Option<&ItemStackValue>> {
    match value {
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

pub fn same_item_same_components(world: &World, a: Entity, b: Entity, items: &Items) -> bool {
    let item = |stack: Entity| world.get::<ItemStack>(stack).map(|stack| stack.item);
    item(a).is_some()
        && item(a) == item(b)
        && stack_to_value(world, a, items).components == stack_to_value(world, b, items).components
}

pub fn item_of(world: &World, stack: Entity) -> Option<ItemId> {
    world.get::<ItemStack>(stack).map(|stack| stack.item)
}
