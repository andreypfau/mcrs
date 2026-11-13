use alloc::string::String;

use crate::area_slot::AreaSlot;
use crate::caption_layout::{self, CaptionLayout};
use crate::gui_point::GuiPoint;
use crate::gui_rect::GuiRect;
use crate::icon_form::IconForm;
use crate::list_frame::ListFrame;
use crate::overflow::Overflow;
use crate::saturate;
use crate::scrollbar::Scrollbar;
use crate::standard_sizes::{BUTTON_LABEL_MARGIN, LINE_HEIGHT};
use crate::tooltip_delay::TooltipDelay;
use crate::widget_id::WidgetId;
use crate::widget_kind::WidgetKind;

pub(crate) const WHITE: u32 = 0xFFFF_FFFF;
pub(crate) const INACTIVE_CAPTION: u32 = 0xFFA0_A0A0;
const SECTION_LABEL_TEXT_TOP: i32 = 4;

/// One widget of a screen: its bounds, its states and its kind (WID-STATE-001).
#[derive(Clone, Debug)]
pub struct Widget {
    id: WidgetId,
    pub(crate) rect: GuiRect,
    /// The plain text that the widget shows; the screen sets it after each value change.
    pub caption: String,
    /// A widget that is not visible does not draw and takes no input.
    pub visible: bool,
    /// An inactive widget takes no press and no key, and draws in its inactive form.
    pub active: bool,
    /// The group that sorts the widgets for Tab, the smallest first (WID-FOC-004).
    pub tab_group: i32,
    /// The tooltip, when the screen gives the widget a tooltip text.
    pub tooltip: Option<TooltipDelay>,
    pub kind: WidgetKind,
    /// The scroll area that holds the widget; none for a widget of the screen itself.
    pub area: Option<AreaSlot>,
    pub(crate) tooltip_start_ms: Option<u64>,
}

impl Widget {
    /// A visible widget in tab group 0 with no tooltip; it is active unless it is a text widget.
    #[must_use]
    pub fn new(id: WidgetId, rect: GuiRect, caption: String, kind: WidgetKind) -> Self {
        Self {
            id,
            rect,
            caption,
            visible: true,
            active: !kind.is_text(),
            tab_group: 0,
            tooltip: None,
            kind,
            area: None,
            tooltip_start_ms: None,
        }
    }

    /// The widget with the bounds `rect`, before the screen adds it; after that
    /// [`crate::Widgets::set_rect`] moves it.
    #[must_use]
    pub const fn with_rect(mut self, rect: GuiRect) -> Self {
        self.rect = rect;
        self
    }

    /// The bounds in GUI units.
    #[must_use]
    pub const fn rect(&self) -> GuiRect {
        self.rect
    }

    /// The id that the screen gave the widget.
    #[must_use]
    pub const fn id(&self) -> WidgetId {
        self.id
    }

    /// The ARGB colour of the caption before the alpha of the widget and the colours of its style
    /// (WID-STATE-003).
    #[must_use]
    pub fn caption_color(&self) -> u32 {
        let greys = !matches!(self.kind, WidgetKind::Checkbox { .. }) && !self.kind.is_text();
        if greys && !self.active {
            INACTIVE_CAPTION
        } else {
            WHITE
        }
    }

    /// Where a caption of `text_width` draws at `now_ms`, for a widget whose caption draws on one
    /// line (WID-BTN-004, WID-BTN-005, WID-ICON-001, WID-TXT-001).
    #[must_use]
    pub fn caption_layout(&self, text_width: i32, now_ms: u64) -> Option<CaptionLayout> {
        let rect = self.rect;
        match self.kind {
            WidgetKind::Button | WidgetKind::CycleButton(_) | WidgetKind::Slider(_) => {
                let area = self.label_area(0);
                let centre = area.x.saturating_add(area.width / 2);
                Some(caption_layout::label(area, centre, text_width, now_ms))
            }
            WidgetKind::IconButton {
                form: IconForm::IconOnly,
                ..
            }
            | WidgetKind::ImageButton
            | WidgetKind::Checkbox { .. }
            | WidgetKind::MultiLineText { .. }
            | WidgetKind::SelectionList(_)
            | WidgetKind::WidgetList(_)
            | WidgetKind::ScrollPanel(_)
            | WidgetKind::TabBar(_)
            | WidgetKind::EditBox(_) => None,
            WidgetKind::SectionLabel => {
                let left = i64::from(rect.x) + (i64::from(rect.width) - i64::from(text_width)) / 2;
                Some(CaptionLayout {
                    origin: GuiPoint::new(
                        saturate::saturate(left),
                        rect.y.saturating_add(SECTION_LABEL_TEXT_TOP),
                    ),
                    clip: None,
                })
            }
            WidgetKind::IconButton { icon, .. } => {
                let area = self.label_area(icon.width.saturating_add(BUTTON_LABEL_MARGIN));
                let centre = rect.x.saturating_add(rect.width / 2);
                Some(caption_layout::label(area, centre, text_width, now_ms))
            }
            WidgetKind::Text {
                max_width,
                overflow,
            } => {
                let limit = max_width.unwrap_or(rect.width);
                if overflow == Overflow::Scroll && text_width > limit {
                    return Some(caption_layout::scrolled(
                        self.label_area(0),
                        text_width,
                        now_ms,
                    ));
                }
                let top = rect
                    .y
                    .saturating_add(rect.height.saturating_sub(LINE_HEIGHT) / 2);
                Some(CaptionLayout {
                    origin: GuiPoint::new(rect.x, top),
                    clip: None,
                })
            }
        }
    }

    /// The top left corner of the icon of an icon button with a caption of `text_width`
    /// (WID-ICON-001).
    #[must_use]
    pub fn icon_origin(&self, text_width: i32) -> Option<GuiPoint> {
        let WidgetKind::IconButton { icon, form } = self.kind else {
            return None;
        };
        let rect = self.rect;
        let x = i64::from(rect.x);
        let width = i64::from(rect.width);
        let icon_width = i64::from(icon.width);
        let top = i64::from(rect.y) + i64::from(rect.height / 2) - i64::from(icon.height / 2);
        let right = x + width - icon_width - 2;
        let area_width = width - icon_width - 6;
        let text_width = i64::from(text_width);
        let left = match form {
            IconForm::IconOnly => x + width / 2 - icon_width / 2,
            IconForm::IconRight => right,
            IconForm::IconAfterText if text_width < area_width + 2 => {
                x + width / 2 + text_width / 2 + 4
            }
            IconForm::IconAfterText => right,
        };
        Some(GuiPoint::new(
            saturate::saturate(left),
            saturate::saturate(top),
        ))
    }

    /// The start of line `line`, from 0, of `line_width` in a multi-line text widget
    /// (WID-TXT-002).
    #[must_use]
    pub fn line_origin(&self, line: i32, line_width: i32) -> Option<GuiPoint> {
        let WidgetKind::MultiLineText { centred, .. } = self.kind else {
            return None;
        };
        let rect = self.rect;
        let x = if centred {
            i64::from(rect.x) + i64::from(rect.width / 2) - i64::from(line_width / 2)
        } else {
            i64::from(rect.x)
        };
        let y = i64::from(rect.y) + i64::from(LINE_HEIGHT) * i64::from(line);
        Some(GuiPoint::new(saturate::saturate(x), saturate::saturate(y)))
    }

    /// The scroll offset of a scroll area (SCRL-AREA-001).
    #[must_use]
    pub fn scroll_offset(&self) -> Option<f64> {
        self.kind.scroll_area(self.rect).map(|area| area.offset())
    }

    /// The scrollbar of a scroll area that is scrollable (SCRL-BAR-001, SCRL-BAR-002).
    #[must_use]
    pub fn scrollbar(&self) -> Option<Scrollbar> {
        self.kind.scroll_area(self.rect)?.scrollbar()
    }

    /// The background and the separators of a list (LIST-BG-001, LIST-BG-002).
    #[must_use]
    pub fn list_frame(&self) -> Option<ListFrame> {
        match self.kind {
            WidgetKind::SelectionList(_) | WidgetKind::WidgetList(_) => {
                let area = self.kind.scroll_area(self.rect)?;
                Some(ListFrame::new(self.rect, area.shift()))
            }
            _ => None,
        }
    }

    pub(crate) fn takes_input(&self) -> bool {
        self.visible && self.active && !self.kind.is_text()
    }

    pub(crate) fn is_under(&self, x: f64, y: f64, shift: i32) -> bool {
        self.takes_input() && self.rect.shifted_up(shift).contains_f64(x, y)
    }

    fn label_area(&self, right_inset: i32) -> GuiRect {
        let rect = self.rect;
        let margins = BUTTON_LABEL_MARGIN.saturating_mul(2);
        GuiRect::new(
            rect.x.saturating_add(BUTTON_LABEL_MARGIN),
            rect.y,
            rect.width
                .saturating_sub(margins)
                .saturating_sub(right_inset),
            rect.height,
        )
    }
}
