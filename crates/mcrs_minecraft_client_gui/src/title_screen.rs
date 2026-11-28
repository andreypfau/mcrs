use mcrs_minecraft_client_widget::{
    BUTTON_HEIGHT, GuiExtent, GuiRect, IconForm, TooltipDelay, Widget, WidgetId, WidgetKind,
};
use mcrs_minecraft_core::VERSION;

use crate::gui_quad::WHITE;
use crate::gui_sprite_id::GuiSpriteId;
use crate::menu::{WidgetIcon, WidgetTooltip};
use mcrs_minecraft_font::font::{Font, Span};
use mcrs_minecraft_font::language::ClientLanguage;

pub(crate) const SINGLEPLAYER: WidgetId = WidgetId(0);
pub(crate) const ACCESSIBILITY: WidgetId = WidgetId(1);
pub(crate) const OPTIONS: WidgetId = WidgetId(2);
pub(crate) const QUIT: WidgetId = WidgetId(3);

const ROW: i32 = 24;
const FULL_WIDTH: i32 = 200;
const HALF_WIDTH: i32 = 98;
const ICON_BUTTON: i32 = 20;
const ICON: i32 = 15;
const SMALLEST_WIDTH: i32 = 320;
const EDGE: i32 = 2;
const VERSION_GAP: i32 = 4;
const TEXT_LINE: i32 = 10;
const NOTICE_LINE: i32 = 9;

const NOTICE: [&[&str]; 2] = [
    &["mcrs", "is", "licensed", "under", "MIT OR Apache-2.0."],
    &[
        "Not",
        "an",
        "official",
        "Minecraft",
        "product.",
        "Not",
        "approved",
        "by",
        "or",
        "associated",
        "with",
        "Mojang",
        "or",
        "Microsoft.",
    ],
];

pub(crate) fn title_screen(screen: GuiExtent, language: &ClientLanguage) -> [Widget; 4] {
    let top = screen.height / 4 + 48;
    let centre = screen.width / 2;
    let button =
        |id, rect, key| Widget::new(id, rect, language.get(key).to_owned(), WidgetKind::Button);
    let single = button(
        SINGLEPLAYER,
        GuiRect::new(centre - 100, top, FULL_WIDTH, BUTTON_HEIGHT),
        "menu.singleplayer",
    );
    let mut accessibility = Widget::new(
        ACCESSIBILITY,
        GuiRect::new(
            centre - ICON_BUTTON / 2,
            top + ROW,
            ICON_BUTTON,
            ICON_BUTTON,
        ),
        language.get("options.accessibility").to_owned(),
        WidgetKind::IconButton {
            icon: GuiExtent::new(ICON, ICON),
            form: IconForm::IconOnly,
        },
    );
    accessibility.tooltip = Some(TooltipDelay::default());
    [
        single,
        accessibility,
        button(
            OPTIONS,
            GuiRect::new(centre - 100, top + 2 * ROW, HALF_WIDTH, BUTTON_HEIGHT),
            "menu.options",
        ),
        button(
            QUIT,
            GuiRect::new(centre + 2, top + 2 * ROW, HALF_WIDTH, BUTTON_HEIGHT),
            "menu.quit",
        ),
    ]
}

pub(crate) const ACCESSIBILITY_ICON: WidgetIcon = WidgetIcon {
    widget: ACCESSIBILITY,
    sprite: GuiSpriteId::IconAccessibility,
};

pub(crate) fn accessibility_tooltip(language: &ClientLanguage) -> WidgetTooltip {
    WidgetTooltip {
        widget: ACCESSIBILITY,
        text: vec![Span::new(
            language.get("options.accessibility.tooltip"),
            WHITE,
        )],
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct TitleText {
    pub(crate) text: String,
    pub(crate) at: GuiRect,
}

pub(crate) fn version_text(screen: GuiExtent, font: &Font) -> TitleText {
    let text = format!("Minecraft {}", VERSION.name);
    let width = font.str_width(&text);
    let at = GuiRect::new(EDGE, screen.height - TEXT_LINE, width, NOTICE_LINE);
    TitleText { text, at }
}

pub(crate) fn notice_lines(screen: GuiExtent, font: &Font) -> Vec<TitleText> {
    let version = version_text(screen, font).at.width;
    let limit = SMALLEST_WIDTH - EDGE - VERSION_GAP - (EDGE + version);
    let mut lines: Vec<String> = Vec::new();
    for part in NOTICE {
        let mut line = String::new();
        for piece in part {
            let longer = if line.is_empty() {
                (*piece).to_owned()
            } else {
                format!("{line} {piece}")
            };
            if line.is_empty() || font.str_width(&longer) <= limit {
                line = longer;
            } else {
                lines.push(std::mem::replace(&mut line, (*piece).to_owned()));
            }
        }
        lines.push(line);
    }
    let count = lines.len() as i32;
    lines
        .into_iter()
        .zip(0..)
        .map(|(line, index)| {
            let width = font.str_width(&line);
            let top = screen.height - TEXT_LINE - NOTICE_LINE * (count - 1 - index);
            let at = GuiRect::new(screen.width - EDGE - width, top, width, NOTICE_LINE);
            TitleText { text: line, at }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    mod jar {
        use mcrs_minecraft_client_widget::GuiPoint;

        use super::super::*;
        use crate::corpus_font::corpus_font;
        use crate::corpus_language::corpus_language;
        use crate::menu_test_app::GUI;

        #[test]
        fn the_title_widgets_and_the_notice_lines_sit_where_the_title_screen_places_them() {
            let language = corpus_language();
            let font = corpus_font();
            let version = format!("Minecraft {}", mcrs_minecraft_core::VERSION.name);
            let cases = [
                (
                    GUI,
                    [
                        ("Singleplayer", GuiRect::new(113, 108, 200, 20)),
                        ("Accessibility Settings...", GuiRect::new(203, 132, 20, 20)),
                        ("Options...", GuiRect::new(113, 156, 98, 20)),
                        ("Quit Game", GuiRect::new(215, 156, 98, 20)),
                        ("mcrs is licensed under", GuiRect::new(310, 194, 115, 9)),
                        ("MIT OR Apache-2.0.", GuiRect::new(331, 203, 94, 9)),
                        (
                            "Not an official Minecraft product.",
                            GuiRect::new(257, 212, 168, 9),
                        ),
                        (
                            "Not approved by or associated with",
                            GuiRect::new(245, 221, 180, 9),
                        ),
                        ("Mojang or Microsoft.", GuiRect::new(320, 230, 105, 9)),
                        (&version, GuiRect::new(2, 230, 131, 9)),
                    ],
                ),
                (
                    GuiExtent::new(320, 240),
                    [
                        ("Singleplayer", GuiRect::new(60, 108, 200, 20)),
                        ("Accessibility Settings...", GuiRect::new(150, 132, 20, 20)),
                        ("Options...", GuiRect::new(60, 156, 98, 20)),
                        ("Quit Game", GuiRect::new(162, 156, 98, 20)),
                        ("mcrs is licensed under", GuiRect::new(203, 194, 115, 9)),
                        ("MIT OR Apache-2.0.", GuiRect::new(224, 203, 94, 9)),
                        (
                            "Not an official Minecraft product.",
                            GuiRect::new(150, 212, 168, 9),
                        ),
                        (
                            "Not approved by or associated with",
                            GuiRect::new(138, 221, 180, 9),
                        ),
                        ("Mojang or Microsoft.", GuiRect::new(213, 230, 105, 9)),
                        (&version, GuiRect::new(2, 230, 131, 9)),
                    ],
                ),
            ];
            for (extent, expected) in cases {
                let widgets = title_screen(extent, language);
                let placed: Vec<(String, GuiRect)> = widgets
                    .iter()
                    .map(|widget| (widget.caption.clone(), widget.rect()))
                    .chain(
                        notice_lines(extent, font)
                            .into_iter()
                            .chain([version_text(extent, font)])
                            .map(|line| (line.text, line.at)),
                    )
                    .collect();
                let expected = expected.map(|(text, rect)| (text.to_owned(), rect));
                assert_eq!(placed, expected, "at {extent:?}");
            }
            let [_, accessibility, ..] = title_screen(GUI, language);
            assert_eq!(accessibility.icon_origin(0), Some(GuiPoint::new(206, 135)));
        }
    }
}
