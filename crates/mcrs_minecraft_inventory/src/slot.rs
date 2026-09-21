use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_item::{
    Held, ItemStack, Items, SelectedHotbarSlot, SlotTable, is_stackable, max_stack_size, slots, stack_to_value,
};
use mcrs_minecraft_protocol::entity::EquipmentSlot;
use mcrs_minecraft_protocol::item::{ComponentPatch, Equippable};
use mcrs_minecraft_registry::ItemId;
use rustc_hash::FxHashMap;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct Slot {
    pub holder: Entity,
    pub index: u16,
}

impl Slot {
    pub const fn new(holder: Entity, index: u16) -> Self {
        Slot { holder, index }
    }
}

impl From<Held> for Slot {
    fn from(held: Held) -> Self {
        Slot::new(held.holder, held.index)
    }
}

pub fn stack_in(world: &World, slot: Slot) -> Option<Entity> {
    world.get::<SlotTable>(slot.holder)?.get(slot.index)
}

/// What makes two stacks mergeable: the item and its patch over the prototype.
#[derive(Clone, Debug, PartialEq)]
pub struct StackKey {
    pub item: ItemId,
    pub components: ComponentPatch,
}

/// A stack as the planners see it: everything a slot rule needs and nothing
/// that identifies the entity, so a split-off part is the same view with a
/// smaller count.
#[derive(Clone, Debug, PartialEq)]
pub struct StackView {
    pub key: StackKey,
    pub count: u8,
    pub max: u8,
    pub stackable: bool,
    pub armour: Option<u16>,
    pub offhand: bool,
}

impl StackView {
    pub fn of(world: &World, stack: Entity, items: &Items) -> Option<Self> {
        let entity = world.get_entity(stack).ok()?;
        let item = entity.get::<ItemStack>()?;
        let equippable = entity.get::<Equippable>().map(|equippable| equippable.slot);
        Some(StackView {
            key: StackKey {
                item: item.item,
                components: stack_to_value(world, stack, items).components,
            },
            count: item.count,
            max: max_stack_size(entity),
            stackable: is_stackable(entity),
            armour: match equippable {
                Some(EquipmentSlot::Head) => Some(slots::ARMOR_HEAD),
                Some(EquipmentSlot::Chest) => Some(slots::ARMOR_CHEST),
                Some(EquipmentSlot::Legs) => Some(slots::ARMOR_LEGS),
                Some(EquipmentSlot::Feet) => Some(slots::ARMOR_FEET),
                _ => None,
            },
            offhand: equippable == Some(EquipmentSlot::OffHand),
        })
    }

    pub fn same(&self, other: &StackView) -> bool {
        self.key == other.key
    }

    pub fn with_count(&self, count: u8) -> StackView {
        StackView {
            count,
            ..self.clone()
        }
    }
}

/// A stack the planners move that sits in no slot: a dropped item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    Slot(Slot),
    Item(Entity),
}

/// The planners' overlay: every player cell plus the open menu's cells, with
/// the planned moves applied as they are recorded.
#[derive(Debug)]
pub struct MenuSnapshot {
    pub player: Entity,
    pub selected: u8,
    pub layout: Vec<Slot>,
    cells: FxHashMap<Source, StackView>,
}

impl MenuSnapshot {
    pub fn new(world: &World, items: &Items, player: Entity, layout: Vec<Slot>) -> Self {
        let mut cells = FxHashMap::default();
        let player_cells = (0..slots::COUNT as u16).map(|index| Slot::new(player, index));
        for slot in player_cells.chain(layout.iter().copied()) {
            if let Some(view) = stack_in(world, slot).and_then(|stack| StackView::of(world, stack, items)) {
                cells.insert(Source::Slot(slot), view);
            }
        }
        MenuSnapshot {
            player,
            selected: world.get::<SelectedHotbarSlot>(player).map_or(0, |selected| selected.0),
            layout,
            cells,
        }
    }

    /// A snapshot with nothing in it, for planning against hand-made cells.
    pub fn empty(player: Entity, selected: u8, layout: Vec<Slot>) -> Self {
        MenuSnapshot {
            player,
            selected,
            layout,
            cells: FxHashMap::default(),
        }
    }

    pub fn add_item(&mut self, item: Entity, view: StackView) {
        self.cells.insert(Source::Item(item), view);
    }

    pub fn carried(&self) -> Slot {
        Slot::new(self.player, slots::CARRIED)
    }

    pub fn get(&self, source: impl Into<Source>) -> Option<&StackView> {
        self.cells.get(&source.into())
    }

    pub fn count(&self, source: impl Into<Source>) -> u8 {
        self.get(source).map_or(0, |view| view.count)
    }

    pub fn set(&mut self, source: impl Into<Source>, view: Option<StackView>) {
        match view {
            Some(view) => self.cells.insert(source.into(), view),
            None => self.cells.remove(&source.into()),
        };
    }

    pub fn take(&mut self, source: impl Into<Source>) -> Option<StackView> {
        self.cells.remove(&source.into())
    }

    /// How many of the stack the cell may hold, `None` when it may not hold
    /// it at all.
    /// ponytail: the only cell rules are the player's result and armour cells;
    /// a container with its own limit (a chest's 64) caps here when it exists.
    pub fn cell_max(&self, slot: Slot, view: &StackView) -> Option<u8> {
        if slot.holder != self.player {
            return Some(view.max);
        }
        match slot.index {
            slots::RESULT => None,
            slots::ARMOR_HEAD..=slots::ARMOR_FEET => (view.armour == Some(slot.index)).then_some(1),
            _ => Some(view.max),
        }
    }
}

impl From<Slot> for Source {
    fn from(slot: Slot) -> Self {
        Source::Slot(slot)
    }
}
