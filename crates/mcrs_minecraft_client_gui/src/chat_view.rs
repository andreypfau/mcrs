use bevy::math::IVec2;

use mcrs_minecraft_chat_client::Tag;
use mcrs_minecraft_client_widget::GuiRect;

use crate::gui_size::GuiSize;
use mcrs_minecraft_font::font::{self, Font, Span};

pub(crate) const LINE: i32 = 9;
pub(crate) const TEXT_OFFSET: i32 = 8;
pub(crate) const BOTTOM: i32 = 40;
const FOCUSED_HEIGHT: f32 = 1.0;
const UNFOCUSED_HEIGHT: f32 = 0.4375;
pub(crate) const CHAT_WIDTH: i32 = 320;
const ICON_WIDTH: i32 = 15;

pub(crate) const GRAY: u32 = 0xFFAA_AAAA;
pub(crate) const RED: u32 = 0xFFFF_5555;

#[derive(Clone, Copy, PartialEq, Eq)]
pub(crate) enum ChatFocus {
    Focused,
    Unfocused,
}

pub(crate) fn per_page(focus: ChatFocus) -> usize {
    let height = match focus {
        ChatFocus::Focused => FOCUSED_HEIGHT,
        ChatFocus::Unfocused => UNFOCUSED_HEIGHT,
    };
    ((height * 160.0 + 20.0).floor() as i32 / LINE) as usize
}

pub(crate) fn wrap_width(tag: Option<Tag>) -> i32 {
    if tag.is_some_and(Tag::has_icon) {
        CHAT_WIDTH - ICON_WIDTH
    } else {
        CHAT_WIDTH
    }
}

pub(crate) fn input_bounds(size: GuiSize) -> GuiRect {
    GuiRect::new(4, size.height - 12, (size.width - 8).max(0), 12)
}

pub(crate) fn open_chat_span_at<'c>(
    size: GuiSize,
    lines: &'c mcrs_minecraft_chat_client::ChatLines<Vec<Span>>,
    at: IVec2,
    font: &Font,
) -> Option<&'c Span> {
    let page = per_page(ChatFocus::Focused);
    text_under(lines, page, |slot| text_origin(size, slot), at, font).map(|under| under.span)
}

pub(crate) fn text_origin(size: GuiSize, slot: usize) -> IVec2 {
    IVec2::new(4, size.height - BOTTOM - slot as i32 * LINE - TEXT_OFFSET)
}

pub(crate) struct TextUnder<'c> {
    pub(crate) span: &'c Span,
    pub(crate) message: usize,
}

pub(crate) fn text_under<'c>(
    lines: &'c mcrs_minecraft_chat_client::ChatLines<Vec<Span>>,
    page: usize,
    origin: impl Fn(usize) -> IVec2,
    at: IVec2,
    font: &Font,
) -> Option<TextUnder<'c>> {
    let mut message = 0;
    for (slot, line) in lines.visible(page) {
        message += usize::from(line.last);
        if let Some(span) = font::span_at(font, origin(slot), &line.content, at) {
            return Some(TextUnder { span, message });
        }
    }
    None
}
