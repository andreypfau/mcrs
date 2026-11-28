use core::time::Duration;

use bevy::prelude::*;
use mcrs_minecraft_container_click::{ButtonBinding, Gesture, StackPresence};
use mcrs_minecraft_container_gesture::{
    DragPreview, GestureButton, GestureClick, GestureFrame, MenuView, Modifiers, PointerMotion,
};
use mcrs_minecraft_engine_inventory_click::MenuTargetView;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_inventory_click::{DragEntry, DragKind, DragSplit};
use mcrs_minecraft_inventory_client::{CreativeClick, MenuAction, MenuInput};
use mcrs_minecraft_item_stack::ItemStack;

use crate::creative_clicks;
use crate::creative_grid::CreativeGrid;
use crate::hover::Hover;
use crate::menu_stacks::MenuStacks;

pub(crate) fn gesture_button(button: MouseButton) -> Option<GestureButton> {
    let other = |number| GestureButton {
        number,
        binding: ButtonBinding::Other,
    };
    match button {
        MouseButton::Left => Some(GestureButton::LEFT),
        MouseButton::Right => Some(GestureButton::RIGHT),
        MouseButton::Middle => Some(GestureButton::MIDDLE),
        MouseButton::Back => Some(other(4)),
        MouseButton::Forward => Some(other(5)),
        MouseButton::Other(number) => u8::try_from(number).ok().map(other),
    }
}

fn presence(stack: Option<Entity>) -> StackPresence {
    match stack {
        Some(_) => StackPresence::Stack,
        None => StackPresence::Empty,
    }
}

pub(crate) struct MenuStacksView<'a, 'w, 's> {
    pub(crate) stacks: &'a MenuStacks,
    pub(crate) targets: Option<&'a MenuTargetView<'a>>,
    pub(crate) items: Option<&'a mut Stacks<'w, 's>>,
}

impl MenuView for MenuStacksView<'_, '_, '_> {
    type Stack = ItemStack;

    fn carried(&self) -> StackPresence {
        presence(self.stacks.carried())
    }

    fn entry(&self, index: u16) -> StackPresence {
        presence(self.stacks.get(index))
    }

    fn knows_drag_rules(&self) -> bool {
        self.targets.is_some()
    }

    fn joins_drag(&self, index: u16) -> Option<DragEntry> {
        self.targets?.joins_drag(index)
    }

    fn stays_in_drag(&self, index: u16) -> Option<DragEntry> {
        self.targets?.stays_in_drag(index)
    }

    fn drag_split(&self, kind: DragKind, size: u32) -> Option<DragSplit> {
        self.targets?.drag_split(kind, size)
    }

    fn permits_pick_up_all(&self, index: u16) -> bool {
        (self.targets).is_some_and(|targets| targets.permits_pick_up_all(index))
    }

    fn entry_count(&self) -> u16 {
        self.targets.map_or(0, MenuTargetView::entry_count)
    }

    fn stack(&mut self, index: u16) -> Option<ItemStack> {
        let stack = self.stacks.get(index)?;
        self.items.as_deref_mut()?.to_value(stack)
    }

    fn shift_double_click_moves(
        &mut self,
        target: u16,
        index: u16,
        remembered: &mut ItemStack,
    ) -> bool {
        let (Some(targets), Some(items)) = (self.targets, self.items.as_deref_mut()) else {
            return false;
        };
        targets.shift_double_click_moves(target, index, |stack| {
            let Some(count) = items.count(stack) else {
                return false;
            };
            remembered.count = count;
            items.equals_value(stack, remembered)
        })
    }
}

pub(crate) struct GestureInput<'a, 'w, 's> {
    pub(crate) frame: GestureFrame<'a, MenuStacksView<'a, 'w, 's>>,
    pub(crate) viewer: Entity,
    pub(crate) grid: Option<CreativeGrid<'a>>,
    pub(crate) sent: &'a mut Vec<MenuInput>,
}

impl GestureInput<'_, '_, '_> {
    fn wrap(&mut self, hover: Hover) {
        for click in self.frame.sent.drain(..) {
            let action = match (click, hover) {
                (GestureClick::Hit(click), Hover::Creative(hit)) => {
                    match creative_clicks::target_of(hit, self.grid) {
                        Some(target) => MenuAction::Creative(CreativeClick {
                            area: target.area,
                            click,
                        }),
                        None => creative_clicks::entry_click(click),
                    }
                }
                (GestureClick::Hit(click) | GestureClick::Menu(click), _) => match self.grid {
                    Some(_) => creative_clicks::entry_click(click),
                    None => MenuAction::Click(click),
                },
            };
            self.sent.push(MenuInput {
                viewer: self.viewer,
                action,
            });
        }
    }

    pub(crate) fn click(&mut self, hover: Hover, gesture: Gesture, modifiers: Modifiers) -> bool {
        let clicked = (self.frame).click(hover.hit(self.grid), gesture, modifiers);
        self.wrap(hover);
        clicked
    }

    pub(crate) fn follow_pointer(
        &mut self,
        hover: Hover,
        motion: PointerMotion,
        preview: &mut DragPreview,
    ) {
        (self.frame).follow_pointer(hover.hit(self.grid), motion, preview);
    }

    pub(crate) fn project_preview(&self, preview: &mut DragPreview) {
        self.frame.project_preview(preview);
    }

    pub(crate) fn skip_release(&mut self) {
        self.frame.gesture.skip_release();
    }

    pub(crate) fn press(
        &mut self,
        button: MouseButton,
        at: Duration,
        hover: Hover,
        modifiers: Modifiers,
    ) {
        let Some(button) = gesture_button(button) else {
            return;
        };
        (self.frame).press(button, at, hover.hit(self.grid), modifiers);
        self.wrap(hover);
    }

    pub(crate) fn release(&mut self, button: MouseButton, hover: Hover, modifiers: Modifiers) {
        let Some(button) = gesture_button(button) else {
            return;
        };
        (self.frame).release(button, hover.hit(self.grid), modifiers);
        self.wrap(hover);
    }
}
