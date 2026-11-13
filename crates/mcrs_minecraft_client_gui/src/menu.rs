use bevy::prelude::Resource;
use mcrs_minecraft_client_widget::{WidgetId, Widgets};

use crate::font::Span;
use crate::gui_sprite_id::GuiSpriteId;

/// The open menu screen. A screen inserts it when it opens and removes it when it closes. The
/// builder of the screen alone writes the layout; the menu input changes the values and the focus.
#[derive(Resource, Debug)]
pub struct Menu {
    pub widgets: Widgets,
    pub background: MenuBackground,
    pub separators: Option<MenuSeparators>,
    /// The text of each widget whose `tooltip` gives a delay (WID-TIP-002).
    pub tooltips: Vec<WidgetTooltip>,
    /// The hint of each edit box whose `has_hint` is on (EDIT-HINT-001).
    pub hints: Vec<WidgetHint>,
    /// The content of each row of a selection list (LIST-SEL-001, step 3).
    pub row_texts: Vec<RowText>,
    pub icons: Vec<WidgetIcon>,
    pub images: Vec<WidgetImage>,
}

impl Menu {
    /// A menu of `widgets` over `background`, with no separators, tooltip, hint, row text, icon
    /// or image.
    #[must_use]
    pub fn new(widgets: Widgets, background: MenuBackground) -> Self {
        Self {
            widgets,
            background,
            separators: None,
            tooltips: Vec::new(),
            hints: Vec::new(),
            row_texts: Vec::new(),
            icons: Vec::new(),
            images: Vec::new(),
        }
    }
}

/// What a menu draws under its widgets (WID-SCR-004).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuBackground {
    None,
    /// The gradient of an in-game screen.
    Gradient,
    /// The menu background texture, in its in-world form while the client is in a world.
    Texture,
}

/// The header and the footer heights of a screen that draws the separators (WID-SCR-006).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuSeparators {
    pub header_height: i32,
    pub footer_height: i32,
}

/// The tooltip text of a widget, before the wrap.
#[derive(Clone, Debug, PartialEq)]
pub struct WidgetTooltip {
    pub widget: WidgetId,
    pub text: Vec<Span>,
}

/// The hint text of an edit box; a span with no colour of its own has
/// [`mcrs_minecraft_client_widget::EditBoxLayout::HINT_COLOR`] (EDIT-HINT-001).
#[derive(Clone, Debug, PartialEq)]
pub struct WidgetHint {
    pub widget: WidgetId,
    pub text: Vec<Span>,
}

/// The line of text that a row of a selection list draws in its inner box.
// chisle: a row's content is one white line; a row that draws icons or several lines needs a
// row content type of its own.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowText {
    pub list: WidgetId,
    pub row: usize,
    pub text: String,
}

/// The icon sprite of an icon button (WID-ICON-001).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WidgetIcon {
    pub widget: WidgetId,
    pub sprite: GuiSpriteId,
}

/// The four sprites of an image button (WID-ICON-002).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WidgetImage {
    pub widget: WidgetId,
    pub active: GuiSpriteId,
    pub active_highlighted: GuiSpriteId,
    pub inactive: GuiSpriteId,
    pub inactive_highlighted: GuiSpriteId,
}
