use mcrs_minecraft_client_widget::{
    BUTTON_HEIGHT, DONE_BUTTON_WIDTH, GuiExtent, GuiRect, Widget, WidgetId, WidgetKind,
};

use crate::font::Font;
use crate::language::ClientLanguage;
use crate::option_sub_screen::{OptionList, option_sub_screen};

pub(crate) const MOUSE_SETTINGS: WidgetId = WidgetId(3);

/// The "Controls" screen on a screen of the size `screen` (CTRL-SCR-003).
pub(crate) fn controls_screen(
    screen: GuiExtent,
    language: &ClientLanguage,
    font: &Font,
) -> Option<[Widget; 4]> {
    let [title, done] = option_sub_screen(
        screen,
        language,
        font,
        "controls.title",
        GuiExtent::new(DONE_BUTTON_WIDTH, BUTTON_HEIGHT),
    )?;
    let mut list = OptionList::new(screen);
    let [mouse_settings] = list.option_row([Widget::new(
        MOUSE_SETTINGS,
        GuiRect::default(),
        language.get("options.mouse_settings").to_owned(),
        WidgetKind::Button,
    )])?;
    Some([title, list.into_widget(), mouse_settings, done])
}
