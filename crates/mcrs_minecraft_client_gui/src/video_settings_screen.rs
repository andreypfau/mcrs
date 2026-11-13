use mcrs_minecraft_client_options::ClientOptions;
use mcrs_minecraft_client_widget::{
    BUTTON_HEIGHT, CycleButton, DONE_BUTTON_WIDTH, GuiExtent, GuiRect, KeyModifiers, Slider,
    Widget, WidgetId, WidgetKind,
};

use crate::font::Font;
use crate::language::ClientLanguage;
use crate::option_caption::option_caption;
use crate::option_sub_screen::{OptionList, option_sub_screen};

pub(crate) const DISPLAY: WidgetId = WidgetId(3);
pub(crate) const GUI_SCALE: WidgetId = WidgetId(4);
pub(crate) const BRIGHTNESS: WidgetId = WidgetId(5);
pub(crate) const QUALITY: WidgetId = WidgetId(6);
pub(crate) const SMOOTH_LIGHTING: WidgetId = WidgetId(7);
pub(crate) const CLOUDS: WidgetId = WidgetId(8);

/// The "Video Settings" screen on a screen of the size `screen` with the values of `options`,
/// where `max_gui_scale` is the scale of Auto (MENU-VID-002).
pub(crate) fn video_settings_screen(
    screen: GuiExtent,
    language: &ClientLanguage,
    font: &Font,
    options: &ClientOptions,
    max_gui_scale: u32,
) -> Option<[Widget; 9]> {
    let [title, done] = option_sub_screen(
        screen,
        language,
        font,
        "options.videoTitle",
        GuiExtent::new(DONE_BUTTON_WIDTH, BUTTON_HEIGHT),
    )?;
    let gui_scale_values = usize::try_from(max_gui_scale).ok()?.checked_add(1)?;
    let gui_scale_index = usize::try_from(options.gui_scale.min(max_gui_scale)).ok()?;
    let mut list = OptionList::new(screen);
    let display =
        list.section_label(DISPLAY, language.get("options.video.display.header"), font)?;
    let [gui_scale, brightness] = list.option_row([
        Widget::new(
            GUI_SCALE,
            GuiRect::default(),
            gui_scale_caption(language, options.gui_scale),
            WidgetKind::CycleButton(CycleButton::new(gui_scale_values, gui_scale_index)?),
        ),
        Widget::new(
            BRIGHTNESS,
            GuiRect::default(),
            brightness_caption(language, options.brightness),
            WidgetKind::Slider(Slider::continuous(f64::from(options.brightness))),
        ),
    ])?;
    let quality =
        list.section_label(QUALITY, language.get("options.video.quality.header"), font)?;
    let [smooth_lighting, clouds] = list.option_row([
        Widget::new(
            SMOOTH_LIGHTING,
            GuiRect::default(),
            smooth_lighting_caption(language, options.smooth_light),
            WidgetKind::CycleButton(CycleButton::boolean(options.smooth_light)),
        ),
        Widget::new(
            CLOUDS,
            GuiRect::default(),
            clouds_caption(language, options.clouds),
            WidgetKind::CycleButton(CycleButton::new(2, usize::from(options.clouds))?),
        ),
    ])?;
    Some([
        title,
        list.into_widget(),
        display,
        gui_scale,
        brightness,
        quality,
        smooth_lighting,
        clouds,
        done,
    ])
}

/// Stores the value of `widget` after its change. A cycle button steps from the stored value,
/// with `max_gui_scale` and `modifiers` of the moment of the activation (MENU-VAL-003 to
/// MENU-VAL-006, MENU-SCALE-002).
pub(crate) fn write_option(
    options: &mut ClientOptions,
    widget: &Widget,
    max_gui_scale: u32,
    modifiers: KeyModifiers,
) {
    match (widget.id(), &widget.kind) {
        (BRIGHTNESS, WidgetKind::Slider(slider)) => options.brightness = slider.position() as f32,
        (SMOOTH_LIGHTING, WidgetKind::CycleButton(_)) => {
            options.smooth_light = !options.smooth_light;
        }
        (CLOUDS, WidgetKind::CycleButton(_)) => options.clouds = !options.clouds,
        (GUI_SCALE, WidgetKind::CycleButton(_)) => {
            options.gui_scale = activated_gui_scale(options.gui_scale, max_gui_scale, modifiers);
        }
        _ => {}
    }
}

/// The value that an activation of the GUI Scale button selects after the stored value `stored`
/// (MENU-SCALE-002).
fn activated_gui_scale(stored: u32, max: u32, modifiers: KeyModifiers) -> u32 {
    if modifiers.shift {
        stored
            .checked_sub(1)
            .filter(|_| stored <= max)
            .unwrap_or(max)
    } else {
        stored
            .checked_add(1)
            .filter(|next| *next <= max)
            .unwrap_or(0)
    }
}

/// The value that a wheel event of the vertical value `y` with Ctrl down selects after the stored
/// value `stored`, or none when the value stays (MENU-SCALE-003).
pub(crate) fn wheel_gui_scale(stored: u32, max: u32, y: f32) -> Option<u32> {
    let old = if stored == 0 {
        max.checked_add(1)?
    } else {
        stored
    };
    let new = if y > 0.0 {
        old.checked_add(1)?
    } else if y < 0.0 {
        old.checked_sub(1)?
    } else {
        return None;
    };
    (1..=max).contains(&new).then_some(new)
}

fn gui_scale_caption(language: &ClientLanguage, gui_scale: u32) -> String {
    let value = match gui_scale {
        0 => language.get("options.guiScale.auto").to_owned(),
        scale => scale.to_string(),
    };
    option_caption(
        language,
        "options.generic_value",
        "options.guiScale",
        &value,
    )
}

fn brightness_caption(language: &ClientLanguage, brightness: f32) -> String {
    let percent = (f64::from(brightness) * 100.0).trunc();
    let value = if percent == 0.0 {
        language.get("options.gamma.min").to_owned()
    } else if percent == 50.0 {
        language.get("options.gamma.default").to_owned()
    } else if percent == 100.0 {
        language.get("options.gamma.max").to_owned()
    } else {
        percent.to_string()
    };
    option_caption(language, "options.generic_value", "options.gamma", &value)
}

fn smooth_lighting_caption(language: &ClientLanguage, smooth_light: bool) -> String {
    let value = if smooth_light {
        "options.on"
    } else {
        "options.off"
    };
    option_caption(
        language,
        "options.generic_value",
        "options.ao",
        language.get(value),
    )
}

/// The caption of Clouds: OFF, or Fancy, the boxes 4 blocks high that the sky draws
/// (MENU-VAL-005, step 4).
fn clouds_caption(language: &ClientLanguage, clouds: bool) -> String {
    let value = if clouds {
        "options.clouds.fancy"
    } else {
        "options.off"
    };
    option_caption(
        language,
        "options.generic_value",
        "options.renderClouds",
        language.get(value),
    )
}
