use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::system::Query;
use bevy_ecs::system::SystemParam;
use bevy_ecs::world::World;
use mcrs_minecraft_engine_inventory_menu::MenuButtons;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::SlotPolicy;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_inventory_menu::MenuLayout;

use crate::MenuTargetView;
use crate::click_slots::ClickSlots;

#[derive(SystemParam)]
pub struct MenuTargets<'w, 's> {
    world: &'w World,
    viewers: Query<'w, 's, &'static Menus, With<Slots>>,
    menus: Query<
        'w,
        's,
        (
            &'static MenuLayout,
            &'static MenuParts,
            &'static MenuButtons,
        ),
    >,
    holders: Query<'w, 's, (&'static Slots, Option<&'static SlotPolicy>)>,
    stacks: Stacks<'w, 's>,
}

impl MenuTargets<'_, '_> {
    pub fn read(&mut self, viewer: Entity, cursor: SlotRef) -> Option<MenuTargetView<'_>> {
        let menu = self.viewers.get(viewer).ok().and_then(Menus::open)?;
        let (layout, parts, buttons) = self.menus.get(menu).ok()?;
        let slots = ClickSlots::read(
            self.world,
            viewer,
            layout,
            parts,
            cursor,
            &self.holders,
            &mut self.stacks,
        );
        Some(MenuTargetView {
            layout,
            parts,
            buttons: *buttons,
            slots,
        })
    }
}
