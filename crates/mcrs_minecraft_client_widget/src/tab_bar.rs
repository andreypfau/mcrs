use alloc::vec::Vec;

use crate::caption_layout::{self, CaptionLayout};
use crate::gui_extent::GuiExtent;
use crate::gui_point::GuiPoint;
use crate::gui_rect::GuiRect;
use crate::navigation::TabDirection;
use crate::saturate;
use crate::tab::Tab;

const BAR_HEIGHT: i32 = 24;
const MAX_BAR_WIDTH: i32 = 400;
const BAR_MARGIN: i32 = 28;
const LABEL_INSET: i32 = 1;
const OTHER_LABEL_TOP: i32 = 3;
const BACKGROUND_INSET: i32 = 2;
const LINE_ABOVE_BOTTOM: i32 = 2;
const SEPARATOR_HEIGHT: i32 = 2;
const CONTENT_TOP_SHARE: i32 = 6;

/// A row of tabs with one current tab; the bounds of its widget cover the tabs, which share
/// them in equal widths (TAB-BAR-001, TAB-MENU-001).
#[derive(Clone, Debug, PartialEq)]
pub struct TabBar {
    tabs: Vec<Tab>,
    pub(crate) current: usize,
}

impl TabBar {
    /// A bar of `tabs` with the first tab current.
    #[must_use]
    pub const fn new(tabs: Vec<Tab>) -> Self {
        Self { tabs, current: 0 }
    }

    /// The tabs in order.
    #[must_use]
    pub fn tabs(&self) -> &[Tab] {
        &self.tabs
    }

    /// The current tab.
    #[must_use]
    pub const fn current(&self) -> usize {
        self.current
    }

    /// Sets the current tab when the screen opens, with no event (TAB-BAR-001).
    pub fn set_current(&mut self, tab: usize) {
        if tab < self.tabs.len() {
            self.current = tab;
        }
    }

    /// The bounds of a menu tab bar on a screen `screen_width` wide (TAB-MENU-001).
    #[must_use]
    pub fn menu_rect(&self, screen_width: i32) -> GuiRect {
        let bar_width = (i64::from(screen_width.min(MAX_BAR_WIDTH)) - i64::from(BAR_MARGIN)).max(0);
        let count = i64::try_from(self.tabs.len()).unwrap_or(i64::MAX);
        let tab_width = even(bar_width.checked_div_euclid(count).unwrap_or(0));
        let left = even((i64::from(screen_width) - bar_width).div_euclid(2));
        GuiRect::new(
            saturate::saturate(left),
            0,
            saturate::saturate(tab_width.saturating_mul(count)),
            BAR_HEIGHT,
        )
    }

    /// The rectangle of tab `tab` of a bar at `bar` (TAB-MENU-001).
    #[must_use]
    pub fn tab_rect(&self, bar: GuiRect, tab: usize) -> Option<GuiRect> {
        self.tabs.get(tab)?;
        let width = self.tab_width(bar);
        let left = i64::from(bar.x) + i64::try_from(tab).ok()? * i64::from(width);
        Some(GuiRect::new(
            saturate::saturate(left),
            bar.y,
            width,
            bar.height,
        ))
    }

    /// Where the label of tab `tab` with a width of `text_width` draws at `now_ms`
    /// (TAB-MENU-004).
    #[must_use]
    pub fn label_layout(
        &self,
        bar: GuiRect,
        tab: usize,
        text_width: i32,
        now_ms: u64,
    ) -> Option<CaptionLayout> {
        let rect = self.tab_rect(bar, tab)?;
        let top = if tab == self.current {
            0
        } else {
            OTHER_LABEL_TOP
        };
        let area = GuiRect::new(
            rect.x.saturating_add(LABEL_INSET),
            rect.y.saturating_add(top),
            rect.width.saturating_sub(LABEL_INSET.saturating_mul(2)),
            rect.height.saturating_sub(top),
        );
        let centre =
            saturate::saturate((i64::from(area.x) + i64::from(area.right())).div_euclid(2));
        Some(caption_layout::label(area, centre, text_width, now_ms))
    }

    /// The rectangle of the background of the current tab, with the tile origin at its top left
    /// (TAB-MENU-003).
    #[must_use]
    pub fn current_background(&self, bar: GuiRect) -> Option<GuiRect> {
        let rect = self.tab_rect(bar, self.current)?;
        Some(GuiRect::new(
            rect.x.saturating_add(BACKGROUND_INSET),
            rect.y.saturating_add(BACKGROUND_INSET),
            rect.width
                .saturating_sub(BACKGROUND_INSET.saturating_mul(2)),
            rect.height.saturating_sub(BACKGROUND_INSET),
        ))
    }

    /// The line under the label of the current tab, whose label is `label_width` wide; it draws
    /// in the label colour of the tab (TAB-MENU-003).
    #[must_use]
    pub fn current_line(&self, bar: GuiRect, label_width: i32) -> Option<GuiRect> {
        let rect = self.tab_rect(bar, self.current)?;
        let width = label_width.min(
            rect.width
                .saturating_sub(BACKGROUND_INSET.saturating_mul(2)),
        );
        let left = i64::from(rect.x) + (i64::from(rect.width) - i64::from(width)).div_euclid(2);
        Some(GuiRect::new(
            saturate::saturate(left),
            rect.bottom().saturating_sub(LINE_ABOVE_BOTTOM),
            width,
            1,
        ))
    }

    /// The two parts of the line of a menu tab bar at `bar` on a screen `screen_width` wide,
    /// left and right of the tabs; each tiles from its own left edge (TAB-MENU-005).
    #[must_use]
    pub fn separators(bar: GuiRect, screen_width: i32) -> [GuiRect; 2] {
        let top = bar.bottom().saturating_sub(LINE_ABOVE_BOTTOM);
        let right = bar.right();
        [
            GuiRect::new(0, top, bar.x, SEPARATOR_HEIGHT),
            GuiRect::new(
                right,
                top,
                screen_width.saturating_sub(right),
                SEPARATOR_HEIGHT,
            ),
        ]
    }

    /// The tab content area of a screen of the size `screen` with a footer `footer_height` high
    /// (TAB-AREA-001).
    #[must_use]
    pub fn content_area(screen: GuiExtent, footer_height: i32) -> GuiRect {
        GuiRect::new(
            0,
            BAR_HEIGHT,
            screen.width,
            screen
                .height
                .saturating_sub(footer_height)
                .saturating_sub(BAR_HEIGHT),
        )
    }

    /// The top left corner of the one layout of size `layout` of a tab (TAB-AREA-001).
    #[must_use]
    pub fn content_origin(screen: GuiExtent, footer_height: i32, layout: GuiExtent) -> GuiPoint {
        let area = Self::content_area(screen, footer_height);
        GuiPoint::new(
            area.width.saturating_sub(layout.width) / 2,
            area.y
                .saturating_add(area.height.saturating_sub(layout.height) / CONTENT_TOP_SHARE),
        )
    }

    pub(crate) fn tab_at(&self, bar: GuiRect, x: f64) -> Option<usize> {
        let width = self.tab_width(bar);
        if width <= 0 {
            return None;
        }
        let tab = libm::floor((x - f64::from(bar.x)) / f64::from(width));
        (tab >= 0.0 && tab < self.tabs.len() as f64).then_some(tab as usize)
    }

    pub(crate) fn is_active(&self, tab: usize) -> bool {
        self.tabs.get(tab).is_some_and(|tab| tab.active)
    }

    /// TAB-KEY-001: Ctrl and the digit `digit`.
    pub(crate) fn digit_target(&self, digit: u8) -> Option<usize> {
        let index = usize::from(digit).checked_sub(1).unwrap_or(9);
        let tab = index.min(self.tabs.len().checked_sub(1)?);
        self.is_active(tab).then_some(tab)
    }

    /// TAB-KEY-001: Ctrl+Tab and Ctrl+Shift+Tab, with wrap.
    pub(crate) fn cycled(&self, direction: TabDirection) -> Option<usize> {
        let count = self.tabs.len();
        (1..count)
            .map(|step| match direction {
                TabDirection::Forward => (self.current + step) % count,
                TabDirection::Backward => (self.current + count - step) % count,
            })
            .find(|tab| self.is_active(*tab))
    }

    /// TAB-FOCUS-001: the next active tab on one side of the current tab, without wrap.
    pub(crate) fn beside(&self, direction: TabDirection) -> Option<usize> {
        match direction {
            TabDirection::Forward => {
                (self.current.saturating_add(1)..self.tabs.len()).find(|tab| self.is_active(*tab))
            }
            TabDirection::Backward => (0..self.current).rev().find(|tab| self.is_active(*tab)),
        }
    }

    fn tab_width(&self, bar: GuiRect) -> i32 {
        let count = i32::try_from(self.tabs.len()).unwrap_or(i32::MAX);
        bar.width.checked_div_euclid(count).unwrap_or(0)
    }
}

fn even(value: i64) -> i64 {
    value.saturating_add(value.rem_euclid(2))
}
