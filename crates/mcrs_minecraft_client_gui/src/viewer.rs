use bevy::prelude::*;
use mcrs_minecraft_container_click::{ClickInput, Clicked, Gesture, StackPresence};

use crate::creative_clicks;
use crate::creative_grid::CreativeGrid;
use crate::hover::Hover;
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
        hover: Hover,
        modifiers: Modifiers,
        grid: Option<CreativeGrid>,
    ) -> ClickInput {
        let creative_target = match hover {
            Hover::Creative(hit) => creative_clicks::target(hit, grid),
            Hover::RecipeBookButton | Hover::Menu(_) => None,
        };
        ClickInput {
            gesture,
            shift: modifiers.shift,
            control: modifiers.control,
            creative: self.creative,
            cursor: presence(self.stacks.carried()),
            target: creative_target.unwrap_or(match hover.clicked() {
                Clicked::Entry(menu) => presence(self.stacks.get(menu)),
                Clicked::Outside | Clicked::Nowhere => StackPresence::Empty,
            }),
        }
    }
}
