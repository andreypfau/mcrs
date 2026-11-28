use mcrs_minecraft_container_click::{ClickInput, Clicked, Gesture, StackPresence};

use crate::gesture_hit::GestureHit;
use crate::menu_view::MenuView;
use crate::modifiers::Modifiers;

#[derive(Clone, Copy, Debug)]
pub struct Viewer<M> {
    pub menu: M,
    pub creative: bool,
}

impl<M: MenuView> Viewer<M> {
    #[must_use]
    pub(crate) fn input(
        &self,
        gesture: Gesture,
        hit: GestureHit,
        modifiers: Modifiers,
    ) -> ClickInput {
        let target = hit.target.unwrap_or_else(|| match hit.clicked {
            Some(Clicked::Entry(entry)) => self.menu.entry(entry),
            Some(Clicked::Outside | Clicked::Nowhere) | None => StackPresence::Empty,
        });
        ClickInput {
            gesture,
            shift: modifiers.shift,
            control: modifiers.control,
            creative: self.creative,
            cursor: self.menu.carried(),
            target,
        }
    }
}
