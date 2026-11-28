use alloc::vec::Vec;
use core::time::Duration;

use mcrs_minecraft_container_click::Gesture;

use crate::drag_preview::DragPreview;
use crate::gesture_button::GestureButton;
use crate::gesture_click::GestureClick;
use crate::gesture_hit::GestureHit;
use crate::menu_gesture::MenuGesture;
use crate::menu_view::MenuView;
use crate::modifiers::Modifiers;
use crate::pointer_motion::PointerMotion;
use crate::viewer::Viewer;

pub struct GestureFrame<'a, M: MenuView> {
    pub viewer: Viewer<M>,
    pub gesture: &'a mut MenuGesture<M::Stack>,
    pub sent: &'a mut Vec<GestureClick>,
}

impl<M: MenuView> GestureFrame<'_, M> {
    pub fn click(&mut self, hit: GestureHit, gesture: Gesture, modifiers: Modifiers) -> bool {
        (self.gesture).click(&mut self.viewer, self.sent, hit, gesture, modifiers)
    }

    pub fn follow_pointer(
        &mut self,
        hit: GestureHit,
        motion: PointerMotion,
        preview: &mut DragPreview,
    ) {
        (self.gesture).follow_pointer(&self.viewer.menu, hit, motion, preview);
    }

    pub fn project_preview(&self, preview: &mut DragPreview) {
        self.gesture.project_preview(&self.viewer.menu, preview);
    }

    pub fn press(
        &mut self,
        button: GestureButton,
        at: Duration,
        hit: GestureHit,
        modifiers: Modifiers,
    ) {
        (self.gesture).press(&mut self.viewer, self.sent, button, at, hit, modifiers);
    }

    pub fn release(&mut self, button: GestureButton, hit: GestureHit, modifiers: Modifiers) {
        (self.gesture).release(&mut self.viewer, self.sent, button, hit, modifiers);
    }
}
