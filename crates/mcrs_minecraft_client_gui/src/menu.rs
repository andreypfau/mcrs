use bevy::math::IVec2;
use bevy::prelude::Resource;
use mcrs_minecraft_client_widget::{WidgetId, Widgets};

use crate::font::Span;
use crate::gui_sprite_id::GuiSpriteId;
use crate::gui_texture_id::GuiTextureId;

#[derive(Resource, Debug)]
pub struct Menu {
    pub widgets: Widgets,
    pub background: MenuBackground,
    pub separators: Option<MenuSeparators>,
    pub tooltips: Vec<WidgetTooltip>,
    pub hints: Vec<WidgetHint>,
    pub rows: Vec<RowContent>,
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
            rows: Vec::new(),
            icons: Vec::new(),
            images: Vec::new(),
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuBackground {
    None,
    Gradient,
    ErrorGradient,
    Texture,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuSeparators {
    pub header_height: Option<i32>,
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

#[derive(Clone, Debug, PartialEq)]
pub struct RowContent {
    pub list: WidgetId,
    pub row: usize,
    pub icon: Option<GuiTextureId>,
    pub lines: Vec<RowLine>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct RowLine {
    pub offset: IVec2,
    pub spans: Vec<Span>,
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
