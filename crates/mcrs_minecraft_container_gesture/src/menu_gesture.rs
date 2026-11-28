use alloc::vec::Vec;
use core::time::Duration;

use mcrs_minecraft_container_click::{
    ButtonBinding, Click, ClickKind, Clicked, Gesture, StackPresence,
};
use mcrs_minecraft_inventory_click::{DragKind, DragStage};

use crate::drag_preview::DragPreview;
use crate::gesture_button::GestureButton;
use crate::gesture_click::GestureClick;
use crate::gesture_hit::GestureHit;
use crate::last_press::LastPress;
use crate::menu_view::MenuView;
use crate::modifiers::Modifiers;
use crate::open_drag::OpenDrag;
use crate::pointer_motion::PointerMotion;
use crate::press_kind::PressKind;
use crate::release_rule::ReleaseRule;
use crate::viewer::Viewer;

const DOUBLE_CLICK: Duration = Duration::from_millis(250);

#[derive(Debug)]
pub struct MenuGesture<S> {
    release: ReleaseRule,
    press: PressKind,
    last_press: Option<LastPress>,
    remembered: Option<S>,
}

impl<S> Default for MenuGesture<S> {
    fn default() -> Self {
        Self {
            release: ReleaseRule::default(),
            press: PressKind::default(),
            last_press: None,
            remembered: None,
        }
    }
}

impl<S> MenuGesture<S> {
    #[must_use]
    pub fn is_dragging(&self) -> bool {
        self.open_drag().is_some()
    }

    pub fn skip_release(&mut self) {
        self.release = ReleaseRule::NoClick;
    }

    fn open_drag(&self) -> Option<OpenDrag> {
        match self.release {
            ReleaseRule::Drag(open) => Some(open),
            ReleaseRule::NoClick | ReleaseRule::Click => None,
        }
    }
}

impl<S: Clone> MenuGesture<S> {
    pub(crate) fn click<M>(
        &mut self,
        viewer: &mut Viewer<M>,
        sent: &mut Vec<GestureClick>,
        hit: GestureHit,
        gesture: Gesture,
        modifiers: Modifiers,
    ) -> bool
    where
        M: MenuView<Stack = S>,
    {
        let Some(clicked) = hit.clicked else {
            return false;
        };
        let input = viewer.input(gesture, hit, modifiers);
        let Some(click) = Click::from_input(clicked, input) else {
            return false;
        };
        if hit.entry.is_some() {
            self.remember_quick_move(click, &mut viewer.menu);
        }
        sent.push(GestureClick::Hit(click));
        true
    }

    pub(crate) fn follow_pointer<M>(
        &mut self,
        menu: &M,
        hit: GestureHit,
        motion: PointerMotion,
        preview: &mut DragPreview,
    ) where
        M: MenuView<Stack = S>,
    {
        preview.clear();
        if !menu.knows_drag_rules() {
            return;
        }
        let ReleaseRule::Drag(open) = &mut self.release else {
            return;
        };
        let prunes = open.len() >= 2;
        for index in open.entries() {
            match menu.stays_in_drag(index) {
                Some(entry) => preview.join(index, entry),
                None if prunes => open.remove(index),
                None => {}
            }
        }
        if motion == PointerMotion::Moved
            && open.opened_by() == PressKind::Single
            && let Some(index) = hit.entry
            && !open.contains(index)
            && let Some(split) = menu.drag_split(open.kind(), open.len())
            && (open.kind() == DragKind::Full || u32::from(split.count) > open.len())
            && let Some(entry) = menu.joins_drag(index)
        {
            open.insert(index);
            if open.contains(index) {
                preview.join(index, entry);
            }
        }
    }

    pub(crate) fn project_preview<M>(&self, menu: &M, preview: &mut DragPreview)
    where
        M: MenuView<Stack = S>,
    {
        let Some(open) = self.open_drag() else {
            preview.clear();
            return;
        };
        preview.retain(|index| open.contains(index));
        let split = menu.drag_split(open.kind(), preview.entry_count());
        match split {
            Some(split) => preview.split(split),
            None => preview.clear(),
        }
    }

    pub(crate) fn press<M>(
        &mut self,
        viewer: &mut Viewer<M>,
        sent: &mut Vec<GestureClick>,
        button: GestureButton,
        at: Duration,
        hit: GestureHit,
        modifiers: Modifiers,
    ) where
        M: MenuView<Stack = S>,
    {
        self.take_press(LastPress {
            button,
            target: hit.entry,
            at,
        });
        if button == GestureButton::LEFT && hit.takes_left_press {
            if self.open_drag().is_none() {
                self.skip_release();
            }
            return;
        }
        let gesture = Gesture::Press {
            button: button.number,
            binding: button.binding,
        };
        let input = viewer.input(gesture, hit, modifiers);
        let kind = if input.cursor == StackPresence::Empty
            || hit
                .clicked
                .is_none_or(|clicked| clicked == Clicked::Nowhere)
            || !hit.permits_drag
        {
            None
        } else if button == GestureButton::LEFT {
            Some(DragKind::Split)
        } else if button == GestureButton::RIGHT {
            Some(DragKind::Single)
        } else if button.binding == ButtonBinding::PickItem && input.creative {
            Some(DragKind::Full)
        } else {
            None
        };
        let clicked_now = self.click(viewer, sent, hit, gesture, modifiers);
        if self.is_dragging() {
            return;
        }
        self.release = match kind {
            _ if clicked_now => ReleaseRule::NoClick,
            Some(kind) => ReleaseRule::Drag(OpenDrag::new(kind, self.press, button)),
            None => ReleaseRule::Click,
        };
    }

    pub(crate) fn release<M>(
        &mut self,
        viewer: &mut Viewer<M>,
        sent: &mut Vec<GestureClick>,
        button: GestureButton,
        hit: GestureHit,
        modifiers: Modifiers,
    ) where
        M: MenuView<Stack = S>,
    {
        let rule = self.release;
        if button == GestureButton::LEFT
            && self.press == PressKind::Double
            && self.double_release(&mut viewer.menu, sent, hit, modifiers)
        {
            self.release = ReleaseRule::Click;
            return;
        }
        self.release = ReleaseRule::Click;
        match rule {
            ReleaseRule::Drag(open) if open.button() != button => self.skip_release(),
            ReleaseRule::Drag(open) if open.len() > 0 => {
                self.end_drag(&mut viewer.menu, sent, open)
            }
            ReleaseRule::Drag(_) | ReleaseRule::Click => {
                let gesture = Gesture::Release {
                    button: button.number,
                    binding: button.binding,
                };
                self.click(viewer, sent, hit, gesture, modifiers);
            }
            ReleaseRule::NoClick => {}
        }
    }

    fn send<M>(&mut self, menu: &mut M, sent: &mut Vec<GestureClick>, click: Click)
    where
        M: MenuView<Stack = S>,
    {
        self.remember_quick_move(click, menu);
        sent.push(GestureClick::Menu(click));
    }

    fn end_drag<M>(&mut self, menu: &mut M, sent: &mut Vec<GestureClick>, open: OpenDrag)
    where
        M: MenuView<Stack = S>,
    {
        let drag = |stage, clicked| Click {
            kind: ClickKind::Drag,
            button: open.kind().button(stage),
            clicked,
        };
        self.send(menu, sent, drag(DragStage::Start, Clicked::Outside));
        for index in open.entries() {
            self.send(menu, sent, drag(DragStage::Entry, Clicked::Entry(index)));
        }
        self.send(menu, sent, drag(DragStage::End, Clicked::Outside));
    }

    fn double_release<M>(
        &mut self,
        menu: &mut M,
        sent: &mut Vec<GestureClick>,
        hit: GestureHit,
        modifiers: Modifiers,
    ) -> bool
    where
        M: MenuView<Stack = S>,
    {
        let Some(target) = hit.entry else {
            return false;
        };
        if !menu.permits_pick_up_all(target) {
            return false;
        }
        if !modifiers.shift {
            let pick_up_all = Click {
                kind: ClickKind::PickUpAll,
                button: 0,
                clicked: Clicked::Entry(target),
            };
            self.send(menu, sent, pick_up_all);
            return true;
        }
        self.quick_move_alike(target, menu, sent);
        true
    }

    fn remember_quick_move<M>(&mut self, click: Click, menu: &mut M)
    where
        M: MenuView<Stack = S>,
    {
        if click.kind == ClickKind::QuickMove
            && let Clicked::Entry(index) = click.clicked
        {
            self.remembered = menu.stack(index);
        }
    }

    fn quick_move_alike<M>(&mut self, target: u16, menu: &mut M, sent: &mut Vec<GestureClick>)
    where
        M: MenuView<Stack = S>,
    {
        let Some(mut probe) = self.remembered.clone() else {
            return;
        };
        for index in 0..menu.entry_count() {
            if menu.shift_double_click_moves(target, index, &mut probe) {
                let click = Click {
                    kind: ClickKind::QuickMove,
                    button: 0,
                    clicked: Clicked::Entry(index),
                };
                self.remember_quick_move(click, menu);
                sent.push(GestureClick::Menu(click));
            }
        }
    }

    fn take_press(&mut self, press: LastPress) {
        let double = self.last_press.is_some_and(|last| {
            press.at.saturating_sub(last.at) < DOUBLE_CLICK
                && last.button == press.button
                && last.target == press.target
        });
        self.press = if double {
            PressKind::Double
        } else {
            PressKind::Single
        };
        self.last_press = Some(press);
    }
}
