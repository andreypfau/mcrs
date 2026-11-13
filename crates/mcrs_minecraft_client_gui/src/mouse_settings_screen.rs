use mcrs_minecraft_client_options::ClientOptions;
use mcrs_minecraft_client_widget::{
    BUTTON_HEIGHT, DONE_BUTTON_WIDTH, GuiExtent, GuiRect, Slider, Widget, WidgetId, WidgetKind,
};

use crate::font::Font;
use crate::language::ClientLanguage;
use crate::option_caption::option_caption;
use crate::option_sub_screen::{OptionList, option_sub_screen};

pub(crate) const SENSITIVITY: WidgetId = WidgetId(3);

/// The "Mouse Settings" screen on a screen of the size `screen` with the values of `options`
/// (CTRL-MOUSE-003).
pub(crate) fn mouse_settings_screen(
    screen: GuiExtent,
    language: &ClientLanguage,
    font: &Font,
    options: &ClientOptions,
) -> Option<[Widget; 4]> {
    let [title, done] = option_sub_screen(
        screen,
        language,
        font,
        "options.mouse_settings.title",
        GuiExtent::new(DONE_BUTTON_WIDTH, BUTTON_HEIGHT),
    )?;
    let mut list = OptionList::new(screen);
    let [sensitivity] = list.option_row([Widget::new(
        SENSITIVITY,
        GuiRect::default(),
        sensitivity_caption(language, options.sensitivity),
        WidgetKind::Slider(Slider::continuous(f64::from(options.sensitivity))),
    )])?;
    Some([title, list.into_widget(), sensitivity, done])
}

/// Stores the value of Sensitivity after its change: the slider position (CTRL-VAL-001).
pub(crate) fn write_option(options: &mut ClientOptions, widget: &Widget) {
    if widget.id() == SENSITIVITY
        && let WidgetKind::Slider(slider) = &widget.kind
    {
        options.sensitivity = slider.position() as f32;
    }
}

/// The caption of Sensitivity at `sensitivity` (CTRL-VAL-001, steps 3 to 5).
fn sensitivity_caption(language: &ClientLanguage, sensitivity: f32) -> String {
    let named = |key| {
        option_caption(
            language,
            "options.generic_value",
            "options.sensitivity",
            language.get(key),
        )
    };
    if sensitivity == 0.0 {
        return named("options.sensitivity.min");
    }
    if sensitivity == 1.0 {
        return named("options.sensitivity.max");
    }
    let percent = (f64::from(sensitivity) * 200.0).trunc();
    option_caption(
        language,
        "options.percent_value",
        "options.sensitivity",
        &percent.to_string(),
    )
}
