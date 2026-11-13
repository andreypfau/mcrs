use mcrs_minecraft_client_options::{
    ClientOptions, MAX_FRAMERATE_RANGE, MAX_FRAMERATE_STEP, VIEW_DISTANCE_DEFAULT,
    VIEW_DISTANCE_RANGE,
};
use mcrs_minecraft_client_widget::{
    BUTTON_HEIGHT, CycleButton, DONE_BUTTON_WIDTH, GuiExtent, GuiRect, KeyModifiers, Slider,
    TooltipDelay, Widget, WidgetId, WidgetKind,
};

use crate::font::{Font, Span};
use crate::item_decorations::WHITE;
use crate::language::ClientLanguage;
use crate::menu::WidgetTooltip;
use crate::option_caption::option_caption;
use crate::option_sub_screen::{OptionList, option_sub_screen};

pub(crate) const DISPLAY: WidgetId = WidgetId(3);
pub(crate) const GUI_SCALE: WidgetId = WidgetId(4);
pub(crate) const BRIGHTNESS: WidgetId = WidgetId(5);
pub(crate) const QUALITY: WidgetId = WidgetId(6);
pub(crate) const SMOOTH_LIGHTING: WidgetId = WidgetId(7);
pub(crate) const CLOUDS: WidgetId = WidgetId(8);
pub(crate) const MAX_FRAMERATE: WidgetId = WidgetId(9);
pub(crate) const VSYNC: WidgetId = WidgetId(10);
pub(crate) const FULLSCREEN: WidgetId = WidgetId(11);
pub(crate) const RENDER_DISTANCE: WidgetId = WidgetId(12);
pub(crate) const IMPROVED_TRANSPARENCY: WidgetId = WidgetId(13);

/// The "Video Settings" screen on a screen of the size `screen` with the values of `options`,
/// where `max_gui_scale` is the scale of Auto (MENU-VID-002).
pub(crate) fn video_settings_screen(
    screen: GuiExtent,
    language: &ClientLanguage,
    font: &Font,
    options: &ClientOptions,
    max_gui_scale: u32,
) -> Option<[Widget; 14]> {
    let [title, done] = option_sub_screen(
        screen,
        language,
        font,
        "options.videoTitle",
        GuiExtent::new(DONE_BUTTON_WIDTH, BUTTON_HEIGHT),
    )?;
    let gui_scale_values = usize::try_from(max_gui_scale).ok()?.checked_add(1)?;
    let gui_scale_index = usize::try_from(options.gui_scale.min(max_gui_scale)).ok()?;
    let framerate_slider = Slider::range(
        i32::from(MAX_FRAMERATE_RANGE.start() / MAX_FRAMERATE_STEP),
        i32::from(MAX_FRAMERATE_RANGE.end() / MAX_FRAMERATE_STEP),
        i32::from(options.max_framerate / MAX_FRAMERATE_STEP),
    )?;
    let view_distance = options.view_distance.unwrap_or(VIEW_DISTANCE_DEFAULT);
    let render_distance_slider = Slider::range(
        i32::from(*VIEW_DISTANCE_RANGE.start()),
        i32::from(*VIEW_DISTANCE_RANGE.end()),
        i32::from(view_distance),
    )?
    .deferred();
    let boolean_button = |id, name_key, value| {
        Widget::new(
            id,
            GuiRect::default(),
            boolean_caption(language, name_key, value),
            WidgetKind::CycleButton(CycleButton::boolean(value)),
        )
    };
    let mut list = OptionList::new(screen);
    let display =
        list.section_label(DISPLAY, language.get("options.video.display.header"), font)?;
    let [max_framerate, vsync] = list.option_row([
        Widget::new(
            MAX_FRAMERATE,
            GuiRect::default(),
            max_framerate_caption(language, options.max_framerate),
            WidgetKind::Slider(framerate_slider),
        ),
        boolean_button(VSYNC, "options.vsync", options.vsync),
    ])?;
    let [gui_scale, fullscreen] = list.option_row([
        Widget::new(
            GUI_SCALE,
            GuiRect::default(),
            gui_scale_caption(language, options.gui_scale),
            WidgetKind::CycleButton(CycleButton::new(gui_scale_values, gui_scale_index)?),
        ),
        boolean_button(FULLSCREEN, "options.fullscreen", options.fullscreen),
    ])?;
    let [brightness] = list.option_row([Widget::new(
        BRIGHTNESS,
        GuiRect::default(),
        brightness_caption(language, options.brightness),
        WidgetKind::Slider(Slider::continuous(f64::from(options.brightness))),
    )])?;
    let quality =
        list.section_label(QUALITY, language.get("options.video.quality.header"), font)?;
    // chisle: Simulation Distance is missing at the index 1 of this row; it comes with the
    // simulation distance option.
    let [render_distance] = list.option_row([Widget::new(
        RENDER_DISTANCE,
        GuiRect::default(),
        render_distance_caption(language, i32::from(view_distance)),
        WidgetKind::Slider(render_distance_slider),
    )])?;
    let [smooth_lighting, clouds] = list.option_row([
        boolean_button(SMOOTH_LIGHTING, "options.ao", options.smooth_light),
        Widget::new(
            CLOUDS,
            GuiRect::default(),
            clouds_caption(language, options.clouds),
            WidgetKind::CycleButton(CycleButton::new(2, usize::from(options.clouds))?),
        ),
    ])?;
    let mut improved_transparency = boolean_button(
        IMPROVED_TRANSPARENCY,
        "options.improvedTransparency",
        options.improved_transparency,
    );
    improved_transparency.tooltip = Some(TooltipDelay::default());
    let [improved_transparency] = list.option_row([improved_transparency])?;
    Some([
        title,
        list.into_widget(),
        display,
        max_framerate,
        vsync,
        gui_scale,
        fullscreen,
        brightness,
        quality,
        render_distance,
        smooth_lighting,
        clouds,
        improved_transparency,
        done,
    ])
}

/// The tooltip of Improved Transparency (MENU-VAL-012, step 2).
pub(crate) fn improved_transparency_tooltip(language: &ClientLanguage) -> WidgetTooltip {
    WidgetTooltip {
        widget: IMPROVED_TRANSPARENCY,
        text: vec![Span::new(
            language.get("options.improvedTransparency.oitWithImprovedFog.tooltip"),
            WHITE,
        )],
    }
}

/// Stores the value of `widget` after its change. A cycle button steps from the stored value,
/// with `max_gui_scale` and `modifiers` of the moment of the activation (MENU-VAL-003 to
/// MENU-VAL-007, MENU-VAL-009 to MENU-VAL-012, MENU-SCALE-002).
pub(crate) fn write_option(
    options: &mut ClientOptions,
    widget: &Widget,
    max_gui_scale: u32,
    modifiers: KeyModifiers,
) {
    match (widget.id(), &widget.kind) {
        (BRIGHTNESS, WidgetKind::Slider(slider)) => options.brightness = slider.position() as f32,
        (MAX_FRAMERATE, WidgetKind::Slider(slider)) => {
            if let Some(framerate) = slider
                .value()
                .and_then(|steps| u16::try_from(steps).ok())
                .and_then(|steps| steps.checked_mul(MAX_FRAMERATE_STEP))
            {
                options.max_framerate = framerate;
            }
        }
        (RENDER_DISTANCE, WidgetKind::Slider(slider)) => {
            // The value that `None` reads as is no change (MENU-DEV-011).
            if let Some(columns) = slider
                .value()
                .and_then(|columns| u8::try_from(columns).ok())
                && columns != options.view_distance.unwrap_or(VIEW_DISTANCE_DEFAULT)
            {
                options.view_distance = Some(columns);
            }
        }
        (VSYNC, WidgetKind::CycleButton(_)) => options.vsync = !options.vsync,
        (FULLSCREEN, WidgetKind::CycleButton(_)) => options.fullscreen = !options.fullscreen,
        (SMOOTH_LIGHTING, WidgetKind::CycleButton(_)) => {
            options.smooth_light = !options.smooth_light;
        }
        (CLOUDS, WidgetKind::CycleButton(_)) => options.clouds = !options.clouds,
        (IMPROVED_TRANSPARENCY, WidgetKind::CycleButton(_)) => {
            options.improved_transparency = !options.improved_transparency;
        }
        (GUI_SCALE, WidgetKind::CycleButton(_)) => {
            options.gui_scale = activated_gui_scale(options.gui_scale, max_gui_scale, modifiers);
        }
        _ => {}
    }
}

/// The caption of `widget` at the value that it waits to store (WID-SLD-007, step 3).
pub(crate) fn pending_caption(language: &ClientLanguage, widget: &Widget) -> Option<String> {
    match (widget.id(), &widget.kind) {
        (RENDER_DISTANCE, WidgetKind::Slider(slider)) => {
            Some(render_distance_caption(language, slider.value()?))
        }
        _ => None,
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

/// The caption of a Boolean option named `name_key`: its name and ON or OFF (WID-CYC-001).
fn boolean_caption(language: &ClientLanguage, name_key: &str, value: bool) -> String {
    let value = if value { "options.on" } else { "options.off" };
    option_caption(
        language,
        "options.generic_value",
        name_key,
        language.get(value),
    )
}

/// The caption of Max Framerate at `max_framerate` frames per second (MENU-VAL-009, step 4).
fn max_framerate_caption(language: &ClientLanguage, max_framerate: u16) -> String {
    let value: String = if max_framerate == *MAX_FRAMERATE_RANGE.end() {
        language.get("options.framerateLimit.max").to_owned()
    } else {
        let frames = [vec![Span::new(max_framerate.to_string(), 0)]];
        language
            .translate("options.framerate", 0, &frames)
            .into_iter()
            .map(|span| span.text)
            .collect()
    };
    option_caption(
        language,
        "options.generic_value",
        "options.framerateLimit",
        &value,
    )
}

/// The caption of Render Distance at `columns` (MENU-VAL-007, step 4).
fn render_distance_caption(language: &ClientLanguage, columns: i32) -> String {
    let chunks = [vec![Span::new(columns.to_string(), 0)]];
    let value: String = language
        .translate("options.chunks", 0, &chunks)
        .into_iter()
        .map(|span| span.text)
        .collect();
    option_caption(
        language,
        "options.generic_value",
        "options.renderDistance",
        &value,
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
