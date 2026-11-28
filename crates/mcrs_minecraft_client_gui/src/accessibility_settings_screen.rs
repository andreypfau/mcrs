use mcrs_minecraft_client_options::ClientOptions;
use mcrs_minecraft_client_widget::{
    BUTTON_HEIGHT, BUTTON_WIDTH, GuiExtent, GuiRect, Slider, TooltipDelay, Widget, WidgetId,
    WidgetKind, WidgetValue,
};

use crate::font::{Font, Span};
use crate::item_decorations::WHITE;
use crate::language::ClientLanguage;
use crate::menu::WidgetTooltip;
use crate::option_caption::option_caption;
use crate::option_sub_screen::{OptionList, option_sub_screen};

pub(crate) const FOV_EFFECTS: WidgetId = WidgetId(3);
pub(crate) const CONTROLS: WidgetId = WidgetId(4);

pub(crate) fn accessibility_settings_screen(
    screen: GuiExtent,
    language: &ClientLanguage,
    font: &Font,
    options: &ClientOptions,
) -> Option<[Widget; 5]> {
    let [title, done] = option_sub_screen(
        screen,
        language,
        font,
        "options.accessibility.title",
        GuiExtent::new(BUTTON_WIDTH, BUTTON_HEIGHT),
    )?;
    let scale = options.fov_effect_scale;
    let mut fov_effects = Widget::new(
        FOV_EFFECTS,
        GuiRect::default(),
        fov_effects_caption(language, scale),
        WidgetKind::Slider(Slider::continuous(f64::from(scale).max(0.0).sqrt())),
    );
    fov_effects.tooltip = Some(TooltipDelay::default());
    let mut list = OptionList::new(screen);
    let [controls] = list.option_row([Widget::new(
        CONTROLS,
        GuiRect::default(),
        language.get("options.controls").to_owned(),
        WidgetKind::Button,
    )])?;
    let [fov_effects] = list.option_row([fov_effects])?;
    Some([title, list.into_widget(), controls, fov_effects, done])
}

pub(crate) fn fov_effects_tooltip(language: &ClientLanguage) -> WidgetTooltip {
    WidgetTooltip {
        widget: FOV_EFFECTS,
        text: vec![Span::new(
            language.get("options.fovEffectScale.tooltip"),
            WHITE,
        )],
    }
}

pub(crate) fn write_option(options: &mut ClientOptions, widget: &Widget) {
    if widget.id() == FOV_EFFECTS
        && let Some(WidgetValue::Number(position)) = widget.value()
    {
        options.fov_effect_scale = (position * position) as f32;
    }
}

fn fov_effects_caption(language: &ClientLanguage, scale: f32) -> String {
    if scale == 0.0 {
        return option_caption(
            language,
            "options.generic_value",
            "options.fovEffectScale",
            language.get("options.off"),
        );
    }
    let percent = (f64::from(scale) * 100.0).trunc();
    option_caption(
        language,
        "options.percent_value",
        "options.fovEffectScale",
        &percent.to_string(),
    )
}
