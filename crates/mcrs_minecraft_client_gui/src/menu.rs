use bevy::prelude::Resource;
use mcrs_minecraft_client_widget::{WidgetId, Widgets};

use crate::font::Span;
use crate::gui_sprite_id::GuiSpriteId;

#[derive(Resource, Debug)]
pub struct Menu {
    pub widgets: Widgets,
    pub background: MenuBackground,
    pub separators: Option<MenuSeparators>,
    pub tooltips: Vec<WidgetTooltip>,
    pub hints: Vec<WidgetHint>,
    pub row_texts: Vec<RowText>,
    pub icons: Vec<WidgetIcon>,
    pub images: Vec<WidgetImage>,
}

impl Menu {
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

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuBackground {
    None,
    Gradient,
    Texture,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuSeparators {
    pub header_height: i32,
    pub footer_height: i32,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WidgetTooltip {
    pub widget: WidgetId,
    pub text: Vec<Span>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct WidgetHint {
    pub widget: WidgetId,
    pub text: Vec<Span>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RowText {
    pub list: WidgetId,
    pub row: usize,
    pub text: String,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WidgetIcon {
    pub widget: WidgetId,
    pub sprite: GuiSpriteId,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct WidgetImage {
    pub widget: WidgetId,
    pub active: GuiSpriteId,
    pub active_highlighted: GuiSpriteId,
    pub inactive: GuiSpriteId,
    pub inactive_highlighted: GuiSpriteId,
}
