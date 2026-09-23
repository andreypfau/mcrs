use bevy_ecs::entity::Entity;
use bevy_ecs::world::World;
use mcrs_minecraft_assets::tag::registry::DynTagRegistry;
use mcrs_minecraft_core::HolderSet;
use mcrs_minecraft_item::enchantment::EnchantmentData;
use mcrs_minecraft_item::{
    Item, ItemStack, Items, SelectedHotbarSlot, SlotTable, is_stackable, max_stack_size, slots,
    stack_to_value, tags,
};
use mcrs_minecraft_protocol::entity::EquipmentSlot;
use mcrs_minecraft_protocol::item::{ComponentPatch, Enchantments, Equippable};
use mcrs_minecraft_registry::{ItemId, StaticRegistry};
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
    pub binding_curse: bool,
    pub fits_inside_container_items: bool,
}

impl StackView {
    pub fn of(world: &World, stack: Entity, items: &Items) -> Option<Self> {
        let entity = world.get_entity(stack).ok()?;
        let item = entity.get::<ItemStack>()?;
        let equippable = entity.get::<Equippable>();
        Some(StackView {
            key: StackKey {
                item: item.item,
                components: stack_to_value(world, stack, items).components,
            },
            count: item.count,
            max: max_stack_size(entity),
            stackable: is_stackable(entity),
            armour: match equippable
                .filter(|equippable| admits_player(equippable))
                .map(|equippable| equippable.slot)
            {
                Some(EquipmentSlot::Head) => Some(slots::ARMOR_HEAD),
                Some(EquipmentSlot::Chest) => Some(slots::ARMOR_CHEST),
                Some(EquipmentSlot::Legs) => Some(slots::ARMOR_LEGS),
                Some(EquipmentSlot::Feet) => Some(slots::ARMOR_FEET),
                _ => None,
            },
            offhand: equippable.is_some_and(|equippable| equippable.slot == EquipmentSlot::OffHand),
            binding_curse: prevents_armor_change(world, entity.get::<Enchantments>()),
            fits_inside_container_items: !world
                .get_resource::<DynTagRegistry<Item>>()
                .is_some_and(|tags| tags.contains(&tags::SHULKER_BOXES, u32::from(item.item.0))),
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

fn prevents_armor_change(world: &World, enchantments: Option<&Enchantments>) -> bool {
    let (Some(enchantments), Some(registry)) = (
        enchantments,
        world.get_resource::<StaticRegistry<EnchantmentData>>(),
    ) else {
        return false;
    };
    enchantments.0.iter().any(|(id, _)| {
        registry
            .get_by_loc(id.as_str())
            .and_then(|data| data.effects.as_ref())
            .is_some_and(|effects| effects.prevent_armor_change.is_some())
    })
}

fn admits_player(equippable: &Equippable) -> bool {
    match &equippable.allowed_entities {
        None => true,
        // ponytail: the entity-type tags live outside this crate, so a tag admits
        // nobody; resolve it against the tag registry once a player-wearable item sets one.
        Some(HolderSet::Tag(_)) => false,
        Some(set) => set
            .entries()
            .iter()
            .any(|entity| entity.as_str() == "minecraft:player"),
    }
}

/// A stack the planners move that sits in no slot: a dropped item.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Source {
    Slot(Slot),
    Item(Entity),
}

/// The planners' overlay: every player slot plus the open menu's slots, with
/// the planned moves applied as they are recorded.
#[derive(Debug)]
pub struct MenuSnapshot {
    pub player: Entity,
    pub selected: u8,
    pub layout: Vec<Slot>,
    pub shulker_box_slots: bool,
    stacks: FxHashMap<Source, StackView>,
}

impl MenuSnapshot {
    pub fn new(world: &World, items: &Items, player: Entity, layout: Vec<Slot>) -> Self {
        let mut stacks = FxHashMap::default();
        let player_slots = (0..slots::COUNT as u16).map(|index| Slot::new(player, index));
        for slot in player_slots.chain(layout.iter().copied()) {
            if let Some(view) =
                stack_in(world, slot).and_then(|stack| StackView::of(world, stack, items))
            {
                stacks.insert(Source::Slot(slot), view);
            }
        }
        MenuSnapshot {
            player,
            selected: world
                .get::<SelectedHotbarSlot>(player)
                .map_or(0, |selected| selected.0),
            layout,
            shulker_box_slots: false,
            stacks,
        }
    }

    /// A snapshot with nothing in it, for planning against hand-made slots.
    pub fn empty(player: Entity, selected: u8, layout: Vec<Slot>) -> Self {
        MenuSnapshot {
            player,
            selected,
            layout,
            shulker_box_slots: false,
            stacks: FxHashMap::default(),
        }
    }

    pub fn add_item(&mut self, item: Entity, view: StackView) {
        self.stacks.insert(Source::Item(item), view);
    }

    pub fn carried(&self) -> Slot {
        Slot::new(self.player, slots::CARRIED)
    }

    pub fn get(&self, source: impl Into<Source>) -> Option<&StackView> {
        self.stacks.get(&source.into())
    }

    pub fn count(&self, source: impl Into<Source>) -> u8 {
        self.get(source).map_or(0, |view| view.count)
    }

    pub fn set(&mut self, source: impl Into<Source>, view: Option<StackView>) {
        match view {
            Some(view) => self.stacks.insert(source.into(), view),
            None => self.stacks.remove(&source.into()),
        };
    }

    pub fn take(&mut self, source: impl Into<Source>) -> Option<StackView> {
        self.stacks.remove(&source.into())
    }

    /// How many of the stack the slot may hold, `None` when it may not hold
    /// it at all.
    /// The armour slots hold one stack the player may wear there.
    /// ponytail: a container with its own limit (a chest's 64) caps here when it exists.
    pub fn slot_max(&self, slot: Slot, view: &StackView) -> Option<u8> {
        if slot.holder != self.player {
            if self.shulker_box_slots && !view.fits_inside_container_items {
                return None;
            }
            return Some(view.max);
        }
        match slot.index {
            slots::RESULT => None,
            slots::ARMOR_HEAD..=slots::ARMOR_FEET => (view.armour == Some(slot.index)).then_some(1),
            _ => Some(view.max),
        }
    }

    pub fn may_place(&self, slot: Slot, view: &StackView) -> bool {
        self.slot_max(slot, view).is_some()
    }

    pub fn can_quick_replace(&self, slot: Slot, view: &StackView) -> bool {
        match self.get(slot) {
            None => true,
            Some(occupant) => occupant.same(view) && occupant.count <= view.max,
        }
    }

    pub fn may_pickup(&self, slot: Slot, creative: bool) -> bool {
        creative
            || slot.holder != self.player
            || !(slots::ARMOR_HEAD..=slots::ARMOR_FEET).contains(&slot.index)
            || !self.get(slot).is_some_and(|view| view.binding_curse)
    }

    pub fn can_take_for_pick_all(&self, slot: Slot) -> bool {
        !(slot.holder == self.player && slot.index == slots::RESULT)
    }
}

impl From<Slot> for Source {
    fn from(slot: Slot) -> Self {
        Source::Slot(slot)
    }
}
