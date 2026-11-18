use bevy::prelude::*;
use mcrs_minecraft_engine_inventory_menu::MenuTitle;
use mcrs_minecraft_item::ItemHoverMap;

use crate::font::Span;
use crate::language::ClientLanguage;
use crate::text::text_spans;

pub(crate) const LABEL_COLOR: u32 = 0xFF40_4040;
const INVENTORY_LABEL: &str = "container.inventory";

#[derive(Clone, Debug, Default, PartialEq)]
pub struct MenuLabelText {
    menu: Option<Entity>,
    titled: bool,
    title: Vec<Span>,
    inventory: Vec<Span>,
}

impl MenuLabelText {
    pub fn follow(
        &mut self,
        menu: Entity,
        title: Option<Ref<MenuTitle>>,
        language: Ref<ClientLanguage>,
    ) {
        let stale = self.menu != Some(menu)
            || language.is_changed()
            || self.titled != title.is_some()
            || title.as_ref().is_some_and(DetectChanges::is_changed);
        if !stale {
            return;
        }
        self.menu = Some(menu);
        self.titled = title.is_some();
        self.write(title.as_deref(), &language);
    }

    pub(crate) fn write(&mut self, title: Option<&MenuTitle>, language: &ClientLanguage) {
        self.title = title.map_or_else(Vec::new, |title| match title.0.map(&ItemHoverMap) {
            Ok(text) => text_spans(&text, language, LABEL_COLOR),
            Err(_) => vec![Span::new(language.plain_text(&title.0), LABEL_COLOR)],
        });
        self.inventory = language.translate(INVENTORY_LABEL, LABEL_COLOR, &[]);
    }

    #[must_use]
    pub fn title(&self) -> &[Span] {
        &self.title
    }

    #[must_use]
    pub fn inventory(&self) -> &[Span] {
        &self.inventory
    }
}
