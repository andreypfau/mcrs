use core::time::Duration;

use bevy::prelude::*;
use mcrs_minecraft_container_click::{
    ButtonBinding, Click, ClickKind, Clicked, Gesture, StackPresence,
};
use mcrs_minecraft_engine_inventory_click::MenuTargetView;
use mcrs_minecraft_engine_item::Stacks;
use mcrs_minecraft_inventory_click::{DragKind, DragStage};
use mcrs_minecraft_inventory_client::{CreativeClick, MenuAction, MenuInput};

use crate::creative_clicks;
use crate::creative_grid::CreativeGrid;
use crate::drag_preview::DragPreview;
use crate::hover::Hover;
use crate::last_press::LastPress;
use crate::menu_gesture::MenuGesture;
use crate::modifiers::Modifiers;
use crate::open_drag::OpenDrag;
use crate::press_kind::PressKind;
use crate::release_rule::ReleaseRule;
use crate::viewer::Viewer;

pub(crate) fn button_number(button: MouseButton) -> Option<u8> {
    match button {
        MouseButton::Left => Some(0),
        MouseButton::Right => Some(1),
        MouseButton::Middle => Some(2),
        MouseButton::Back => Some(4),
        MouseButton::Forward => Some(5),
        MouseButton::Other(_) => None,
    }
}

fn button_binding(button: MouseButton) -> ButtonBinding {
    match button {
        MouseButton::Middle => ButtonBinding::PickItem,
        _ => ButtonBinding::Other,
    }
}

pub(crate) struct GestureFrame<'a, 'w, 's> {
    pub(crate) viewer: &'a Viewer<'a>,
    pub(crate) targets: Option<&'a MenuTargetView<'a>>,
    pub(crate) items: Option<&'a mut Stacks<'w, 's>>,
    pub(crate) grid: Option<CreativeGrid<'a>>,
    pub(crate) gesture: &'a mut MenuGesture,
    pub(crate) sent: &'a mut Vec<MenuInput>,
}

impl GestureFrame<'_, '_, '_> {
    fn remember_quick_move(&mut self, click: Click) {
        if click.kind == ClickKind::QuickMove
            && let Clicked::Entry(index) = click.clicked
        {
            self.gesture.remembered = (self.viewer.stacks.get(index))
                .zip(self.items.as_deref_mut())
                .and_then(|(stack, items)| items.to_value(stack));
        }
    }

    fn send(&mut self, click: Click) {
        self.remember_quick_move(click);
        let action = match self.grid {
            Some(_) => creative_clicks::entry_click(click),
            None => MenuAction::Click(click),
        };
        self.sent.push(MenuInput {
            viewer: self.viewer.entity,
            action,
        });
    }

    pub(crate) fn click(&mut self, hover: Hover, gesture: Gesture, modifiers: Modifiers) -> bool {
        let input = self.viewer.input(gesture, hover, modifiers, self.grid);
        let Some(click) = Click::from_input(hover.clicked(), input) else {
            return false;
        };
        let Hover::Creative(hit) = hover else {
            self.send(click);
            return true;
        };
        let Some(area) = creative_clicks::area(hit, self.grid) else {
            return false;
        };
        if hit.menu_index().is_some() {
            self.remember_quick_move(click);
        }
        self.sent.push(MenuInput {
            viewer: self.viewer.entity,
            action: MenuAction::Creative(CreativeClick { area, click }),
        });
        true
    }

    pub(crate) fn follow_pointer(&mut self, hover: Hover, moved: bool, preview: &mut DragPreview) {
        preview.clear();
        let (ReleaseRule::Drag(mut open), Some(targets)) = (self.gesture.release, self.targets)
        else {
            return;
        };
        let prunes = open.len() >= 2;
        for index in open.entries() {
            match targets.stays_in_drag(index) {
                Some(entry) => preview.join(index, entry),
                None if prunes => open.remove(index),
                None => {}
            }
        }
        if moved
            && open.opened_by == PressKind::Single
            && let Some(index) = hover.menu_entry()
            && !open.contains(index)
            && let Some(split) = targets.drag_split(open.kind, open.len())
            && (open.kind == DragKind::Full || u32::from(split.count) > open.len())
            && let Some(entry) = targets.joins_drag(index)
        {
            open.insert(index);
            if open.contains(index) {
                preview.join(index, entry);
            }
        }
        self.gesture.release = ReleaseRule::Drag(open);
    }

    pub(crate) fn project_preview(&self, preview: &mut DragPreview) {
        let split = (self.gesture.open_drag())
            .zip(self.targets)
            .and_then(|(open, targets)| targets.drag_split(open.kind, preview.entry_count()));
        match split {
            Some(split) => preview.split(split),
            None => preview.clear(),
        }
    }

    pub(crate) fn press(
        &mut self,
        button: MouseButton,
        at: Duration,
        hover: Hover,
        modifiers: Modifiers,
    ) {
        let clicked = hover.clicked();
        self.gesture.take_press(LastPress {
            button,
            target: hover.menu_entry(),
            menu: self.viewer.menu,
            at,
        });
        if button == MouseButton::Left && hover.takes_left_press() {
            if self.gesture.open_drag().is_none() {
                self.gesture.release = ReleaseRule::NoClick;
            }
            return;
        }
        let Some(number) = button_number(button) else {
            return;
        };
        let binding = button_binding(button);
        let gesture = Gesture::Press {
            button: number,
            binding,
        };
        let input = self.viewer.input(gesture, hover, modifiers, self.grid);
        let kind = match button {
            _ if input.cursor == StackPresence::Empty
                || clicked == Clicked::Nowhere
                || !hover.permits_drag() =>
            {
                None
            }
            MouseButton::Left => Some(DragKind::Split),
            MouseButton::Right => Some(DragKind::Single),
            _ if binding == ButtonBinding::PickItem && input.creative => Some(DragKind::Full),
            _ => None,
        };
        let clicked_now = self.click(hover, gesture, modifiers);
        self.gesture.release = match self.gesture.release {
            ReleaseRule::Drag(open) => ReleaseRule::Drag(open),
            ReleaseRule::NoClick | ReleaseRule::Click if clicked_now => ReleaseRule::NoClick,
            ReleaseRule::NoClick | ReleaseRule::Click => kind.map_or(ReleaseRule::Click, |kind| {
                ReleaseRule::Drag(OpenDrag::new(kind, self.gesture.press))
            }),
        };
    }

    pub(crate) fn release(&mut self, button: MouseButton, hover: Hover, modifiers: Modifiers) {
        let Some(number) = button_number(button) else {
            return;
        };
        let rule = self.gesture.release;
        if button == MouseButton::Left
            && self.gesture.press == PressKind::Double
            && self.double_release(hover, modifiers)
        {
            self.gesture.release = ReleaseRule::Click;
            return;
        }
        self.gesture.release = ReleaseRule::Click;
        match rule {
            ReleaseRule::Drag(open) if open.button() != button => {
                self.gesture.release = ReleaseRule::NoClick;
            }
            ReleaseRule::Drag(open) if open.len() > 0 => self.end_drag(open),
            ReleaseRule::Drag(_) | ReleaseRule::Click => {
                let gesture = Gesture::Release {
                    button: number,
                    binding: button_binding(button),
                };
                self.click(hover, gesture, modifiers);
            }
            ReleaseRule::NoClick => {}
        }
    }

    fn end_drag(&mut self, open: OpenDrag) {
        let drag = |stage, clicked| Click {
            kind: ClickKind::Drag,
            button: open.kind.button(stage),
            clicked,
        };
        self.send(drag(DragStage::Start, Clicked::Outside));
        for index in open.entries() {
            self.send(drag(DragStage::Entry, Clicked::Entry(index)));
        }
        self.send(drag(DragStage::End, Clicked::Outside));
    }

    fn double_release(&mut self, hover: Hover, modifiers: Modifiers) -> bool {
        let (Some(target), Some(targets)) = (hover.menu_entry(), self.targets) else {
            return false;
        };
        if !targets.permits_pick_up_all(target) {
            return false;
        }
        if !modifiers.shift {
            self.send(Click {
                kind: ClickKind::PickUpAll,
                button: 0,
                clicked: Clicked::Entry(target),
            });
            return true;
        }
        let Some(mut probe) = self.gesture.remembered.clone() else {
            return true;
        };
        for index in 0..targets.entry_count() {
            let Some(items) = self.items.as_deref_mut() else {
                return true;
            };
            let moves = targets.shift_double_click_moves(target, index, |stack| {
                let Some(count) = items.count(stack) else {
                    return false;
                };
                probe.count = count;
                items.equals_value(stack, &probe)
            });
            if moves {
                self.send(Click {
                    kind: ClickKind::QuickMove,
                    button: 0,
                    clicked: Clicked::Entry(index),
                });
            }
        }
        true
    }
}
