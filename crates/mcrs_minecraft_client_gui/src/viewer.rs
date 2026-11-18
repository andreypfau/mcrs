use bevy::prelude::*;
use mcrs_minecraft_container_click::{ClickInput, Clicked, Gesture, StackPresence};

use crate::menu_stacks::MenuStacks;
use crate::modifiers::Modifiers;

pub(crate) struct Viewer<'a> {
    pub(crate) entity: Entity,
    pub(crate) menu: Entity,
    pub(crate) creative: bool,
    pub(crate) stacks: &'a MenuStacks,
}

fn presence(stack: Option<Entity>) -> StackPresence {
    match stack {
        Some(_) => StackPresence::Stack,
        None => StackPresence::Empty,
    }
}

impl Viewer<'_> {
    pub(crate) fn input(
        &self,
        gesture: Gesture,
        clicked: Clicked,
        modifiers: Modifiers,
    ) -> ClickInput {
        ClickInput {
            gesture,
            shift: modifiers.shift,
            control: modifiers.control,
            creative: self.creative,
            cursor: presence(self.stacks.carried()),
            target: match clicked {
                Clicked::Entry(menu) => presence(self.stacks.get(menu)),
                Clicked::Outside | Clicked::Nowhere => StackPresence::Empty,
            },
        }
    }
}
