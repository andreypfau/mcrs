use bevy_ecs::change_detection::DetectChangesMut;
use bevy_ecs::entity::Entity;
use bevy_ecs::query::With;
use bevy_ecs::system::Local;
use bevy_ecs::system::ParamSet;
use bevy_ecs::system::Query;
use bevy_ecs::system::SystemParam;
use bevy_ecs::world::World;
use mcrs_minecraft_container_click::Click;
use mcrs_minecraft_engine_inventory_menu::MenuButtons;
use mcrs_minecraft_engine_inventory_menu::MenuParts;
use mcrs_minecraft_engine_inventory_menu::Menus;
use mcrs_minecraft_engine_item::Request;
use mcrs_minecraft_engine_item::RequestId;
use mcrs_minecraft_engine_item::SlotPolicy;
use mcrs_minecraft_engine_item::SlotRef;
use mcrs_minecraft_engine_item::Slots;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_inventory_click::Operations;
use mcrs_minecraft_inventory_menu::MenuLayout;
use mcrs_minecraft_inventory_transaction::Op;

use crate::MenuClickError;
use crate::MenuClicker;
use crate::MenuDrag;
use crate::click_menu::ClickMenu;
use crate::click_slots::ClickSlots;
use crate::click_slots::StackHandle;

#[derive(SystemParam)]
pub struct MenuClicks<'w, 's> {
    world_and_drags: ParamSet<'w, 's, (&'w World, Query<'w, 's, &'static mut MenuDrag>)>,
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
    ops: Local<'s, Operations<SlotRef, StackHandle>>,
}

impl MenuClicks<'_, '_> {
    pub fn plan(
        &mut self,
        viewer: Entity,
        click: Click,
        clicker: MenuClicker,
    ) -> Result<Request, MenuClickError> {
        let menu = (self.viewers.get(viewer).ok())
            .and_then(Menus::open)
            .ok_or(MenuClickError::NoOpenMenu)?;
        let (layout, parts, buttons) = self
            .menus
            .get(menu)
            .map_err(|_| MenuClickError::NoOpenMenu)?;
        let MenuDrag(mut drag) = *self
            .world_and_drags
            .p1()
            .get(menu)
            .map_err(|_| MenuClickError::NoMenuDrag)?;
        let slots = ClickSlots::read(
            self.world_and_drags.p0(),
            viewer,
            layout,
            parts,
            clicker.cursor,
            &self.holders,
            &mut self.stacks,
        );
        let click_menu = ClickMenu {
            layout,
            parts,
            buttons: *buttons,
            slots: &slots,
        };
        let planned = mcrs_minecraft_inventory_click::request(
            click,
            clicker.player,
            &click_menu,
            &slots,
            &mut drag,
            &mut self.ops,
        );
        if let Ok(mut current) = self.world_and_drags.p1().get_mut(menu) {
            current.set_if_neq(MenuDrag(drag));
        }
        planned?;
        Ok(request(clicker.request, &self.ops))
    }
}

fn request(id: RequestId, ops: &Operations<SlotRef, StackHandle>) -> Request {
    let mut request = Request::new(id);
    for op in ops.iter() {
        request = match op {
            Op::Split { from, to, count } => request.split(from, to, count),
            Op::Merge { from, to } => request.merge(from, to),
            Op::Swap { left, right } => request.swap(left, right),
            Op::Set { slot, stack, count } => {
                request.set(slot, stack.map(|stack| stack.stack.entity), count)
            }
            Op::Drop { from, count } => request.drop(from, count),
        };
    }
    request
}
