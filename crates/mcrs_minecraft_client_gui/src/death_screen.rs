use bevy::prelude::Resource;
use mcrs_minecraft_client_widget::{
    BUTTON_HEIGHT, GuiExtent, GuiRect, Overflow, Widget, WidgetId, WidgetKind, text_line_extent,
};
use mcrs_minecraft_client_world::hardcore_mode::HardcoreMode;
use mcrs_minecraft_protocol_game::text::Text;

use crate::font::Font;
use crate::language::ClientLanguage;

pub(crate) const TITLE: WidgetId = WidgetId(0);
pub(crate) const MESSAGE: WidgetId = WidgetId(1);
pub(crate) const RESPAWN: WidgetId = WidgetId(2);
pub(crate) const TITLE_SCREEN: WidgetId = WidgetId(3);

const INACTIVE_TICKS: u32 = 20;
const TITLE_TOP: i32 = 30;
const MESSAGE_TOP: i32 = 85;
const BUTTON_WIDTH: i32 = 200;
const FIRST_BUTTON_BELOW_QUARTER: i32 = 72;
const BUTTON_STEP: i32 = 24;

#[derive(Resource, Clone, Debug)]
pub(crate) struct DeathScreen {
    message: Text,
    ticks: u32,
}

impl DeathScreen {
    pub(crate) fn new(message: Text) -> Self {
        Self { message, ticks: 0 }
    }

    pub(crate) fn tick(&mut self) {
        self.ticks = self.ticks.saturating_add(1);
    }

    pub(crate) fn buttons_active(&self) -> bool {
        self.ticks >= INACTIVE_TICKS
    }

    pub(crate) fn widgets(
        &self,
        screen: GuiExtent,
        language: &ClientLanguage,
        font: &Font,
        hardcore: HardcoreMode,
    ) -> [Widget; 4] {
        let (title_key, respawn_key) = match hardcore {
            HardcoreMode::Off => ("deathScreen.title", "deathScreen.respawn"),
            HardcoreMode::On => ("deathScreen.title.hardcore", "deathScreen.spectate"),
        };
        let centred_text = |id, top, caption: String| {
            let size = text_line_extent(font.str_width(&caption), None);
            let rect = GuiRect::new(
                screen.width / 2 - size.width / 2,
                top,
                size.width,
                size.height,
            );
            let kind = WidgetKind::Text {
                max_width: None,
                overflow: Overflow::Cut,
            };
            Widget::new(id, rect, caption, kind)
        };
        let button = |id, row, key| {
            let rect = GuiRect::new(
                screen.width / 2 - BUTTON_WIDTH / 2,
                screen.height / 4 + FIRST_BUTTON_BELOW_QUARTER + row * BUTTON_STEP,
                BUTTON_WIDTH,
                BUTTON_HEIGHT,
            );
            let mut widget =
                Widget::new(id, rect, language.get(key).to_owned(), WidgetKind::Button);
            widget.active = self.buttons_active();
            widget
        };
        [
            centred_text(TITLE, TITLE_TOP, language.get(title_key).to_owned()),
            centred_text(MESSAGE, MESSAGE_TOP, language.plain_text(&self.message)),
            button(RESPAWN, 0, respawn_key),
            button(TITLE_SCREEN, 1, "deathScreen.titleScreen"),
        ]
    }
}
