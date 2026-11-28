use std::mem;

use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::Entity;
use bevy_ecs::system::Commands;
use bevy_ecs::system::Query;
use bevy_ecs::system::Res;
use bevy_ecs::system::Single;
use bevy_ecs::system::SystemParam;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::ItemCommandsExt;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::StackCommandsExt;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_engine_registries::RegistryNames;
use mcrs_minecraft_engine_registries::read_packet_stack;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_item_stack::ItemStack;
use mcrs_minecraft_item_stack::WireError;
use mcrs_minecraft_item_table::ItemTable;
use mcrs_minecraft_player_holders::PlayerHolders;
use mcrs_minecraft_protocol_game::ProtoStack;
use mcrs_minecraft_registries::CurrentRegistries;
use mcrs_minecraft_registries::Wire;

use crate::RemoteMenu;

#[derive(SystemParam)]
pub(crate) struct LocalInventory<'w, 's> {
    player: Single<'w, 's, (Entity, &'static PlayerHolders, &'static Menus)>,
    menus: Query<
        'w,
        's,
        (
            &'static MenuLayout,
            &'static MenuParts,
            Option<&'static mut RemoteMenu>,
        ),
    >,
    slots: Query<'w, 's, &'static Slots>,
    stacks: Option<Stacks<'w, 's>>,
    commands: Commands<'w, 's>,
}

#[derive(SystemParam)]
pub(crate) struct StackNames<'w> {
    table: Option<Res<'w, ItemTable>>,
    registries: Option<Res<'w, CurrentRegistries>>,
}

#[must_use]
pub fn shows_inventory_menu(menus: &Menus) -> bool {
    menus
        .open()
        .is_some_and(|open| menus.iter().next() == Some(open))
}

impl StackNames<'_> {
    pub(crate) fn names(&self) -> Option<RegistryNames<'_>> {
        Some(RegistryNames::new(
            self.registries.as_deref()?,
            self.table.as_deref()?,
        ))
    }
}

impl LocalInventory<'_, '_> {
    pub(crate) fn player(&self) -> Entity {
        self.player.0
    }

    pub(crate) fn inventory_menu(&self) -> Option<Entity> {
        self.player.2.iter().next()
    }

    pub(crate) fn menu_of(&self, container_id: i32) -> Option<Entity> {
        if container_id == RemoteMenu::INVENTORY {
            return self.inventory_menu();
        }
        let open = self.player.2.open()?;
        let (_, _, remote) = self.menus.get(open).ok()?;
        (remote?.container_id() == container_id).then_some(open)
    }

    pub(crate) fn is_open(&self, menu: Entity) -> bool {
        self.player.2.open() == Some(menu)
    }

    pub(crate) fn cursor(&self) -> SlotRef {
        SlotRef {
            holder: self.player.1.cursor,
            index: 0,
        }
    }

    pub(crate) fn entry(&self, menu: Entity, index: u16) -> Option<SlotRef> {
        let (layout, parts, _) = self.menus.get(menu).ok()?;
        parts.slot(layout.entry(index)?)
    }

    pub(crate) fn collect_entries(
        &self,
        menu: Entity,
        limit: usize,
        out: &mut Vec<Option<SlotRef>>,
    ) {
        out.clear();
        let Ok((layout, parts, _)) = self.menus.get(menu) else {
            return;
        };
        let count = layout.len().min(u16::try_from(limit).unwrap_or(u16::MAX));
        out.extend((0..count).map(|index| parts.slot(layout.entry(index)?)));
    }

    pub(crate) fn set_state_id(&mut self, menu: Entity, state_id: i32) {
        match self.menus.get_mut(menu) {
            Ok((_, _, Some(mut current))) => {
                let remote = current.with_state_id(state_id);
                current.set_if_neq(remote);
            }
            Ok((_, _, None)) => {
                self.commands
                    .entity(menu)
                    .insert(RemoteMenu::inventory(state_id));
            }
            Err(_) => {}
        }
    }

    pub(crate) fn set(
        &mut self,
        slot: SlotRef,
        wire: &ProtoStack<Wire>,
        names: &RegistryNames<'_>,
        bytes: &mut Vec<u8>,
    ) -> Result<(), WireError> {
        let stack = read_packet_stack(wire, names, bytes)?;
        self.replace(slot, stack);
        Ok(())
    }

    fn replace(&mut self, slot: SlotRef, mut stack: Option<ItemStack>) {
        let Ok(slots) = self.slots.get(slot.holder) else {
            return;
        };
        let index = usize::from(slot.index);
        if index >= slots.len() {
            return;
        }
        let occupant = slots.get(index);
        if let (Some(occupant), Some(value), Some(stacks)) =
            (occupant, stack.as_mut(), self.stacks.as_mut())
            && let Some(held) = stacks.count(occupant)
        {
            let wanted = mem::replace(&mut value.count, held);
            if stacks.equals_value(occupant, value) {
                if wanted != held {
                    self.commands.entity(occupant).insert(wanted).touch();
                }
                return;
            }
            value.count = wanted;
        }
        if let Some(occupant) = occupant {
            self.commands.entity(occupant).despawn();
        }
        if let Some(stack) = stack {
            self.commands
                .spawn_stack(stack)
                .put_at(slot.holder, slot.index);
        }
    }
}
