use crate::button_sprite::ButtonSprite;
use crate::input_device::InputDevice;
use crate::pointer_shape::PointerShape;
use crate::tab_sprite::TabSprite;
use crate::tooltip_request::{TooltipAnchor, TooltipRequest};
use crate::widget::Widget;
use crate::widget_kind::WidgetKind;
use crate::widget_placement::WidgetPlacement;
use crate::widget_sprite::WidgetSprite;
use crate::widgets::{Drag, Widgets};

const FOCUSED_ROW_OUTLINE: u32 = 0xFFFF_FFFF;
const ROW_OUTLINE: u32 = 0xFF80_8080;

impl Widgets {
    /// Where `widget` draws in this frame (SCRL-AREA-003, SCRL-PANEL-001, LIST-BG-003).
    #[must_use]
    pub fn placement(&self, widget: &Widget) -> WidgetPlacement {
        let Some(member) = self.member(widget) else {
            return WidgetPlacement::Screen;
        };
        let clip = member.area.rect;
        let shift = member.area.shift();
        let bounds = match &member.widget.kind {
            WidgetKind::WidgetList(list) => list.row_rect(clip, member.row),
            _ => Some(widget.rect),
        };
        let shown = bounds.is_some_and(|bounds| {
            let bounds = bounds.shifted_up(shift);
            bounds.bottom() >= clip.y && bounds.y <= clip.bottom()
        });
        if member.widget.visible && shown {
            WidgetPlacement::Area { clip, shift }
        } else {
            WidgetPlacement::Hidden
        }
    }

    /// Whether `widget` is under the mouse in this frame, inside the clip of its scroll area
    /// (WID-STATE-002, LIST-WID-001).
    #[must_use]
    pub fn is_hovered(&self, widget: &Widget) -> bool {
        let mouse = self.input.mouse();
        widget.visible
            && self.input.device() == InputDevice::Mouse
            && match self.placement(widget) {
                WidgetPlacement::Screen => widget.rect.contains_point(mouse),
                WidgetPlacement::Area { clip, shift } => {
                    clip.contains_point(mouse)
                        && widget.rect.shifted_up(shift).contains_point(mouse)
                }
                WidgetPlacement::Hidden => false,
            }
    }

    /// Whether `widget` draws its highlighted sprite in this frame (WID-STATE-002).
    #[must_use]
    pub fn is_highlighted(&self, widget: &Widget) -> bool {
        match self.input.device() {
            InputDevice::Mouse => self.is_hovered(widget),
            InputDevice::Tab | InputDevice::Arrow | InputDevice::Activation => {
                self.is_focused(widget)
            }
            InputDevice::None => false,
        }
    }

    /// The row of a list under the mouse in this frame (LIST-HOVER-001).
    #[must_use]
    pub(crate) fn hovered_row(&self, widget: &Widget) -> Option<usize> {
        let mouse = self.input.mouse();
        let (x, y) = (f64::from(mouse.x), f64::from(mouse.y));
        match &widget.kind {
            WidgetKind::SelectionList(list) if widget.visible => list.row_under(widget.rect, x, y),
            WidgetKind::WidgetList(list) if widget.visible => list.row_under(widget.rect, x, y),
            _ => None,
        }
    }

    /// The colour of the outline around the black field of row `row` of a selection list in this
    /// frame, when the row draws one (LIST-SEL-001).
    #[must_use]
    pub fn row_outline(&self, widget: &Widget, row: usize) -> Option<u32> {
        let WidgetKind::SelectionList(list) = &widget.kind else {
            return None;
        };
        if list.selected() == Some(row) {
            Some(if self.is_focused(widget) {
                FOCUSED_ROW_OUTLINE
            } else {
                ROW_OUTLINE
            })
        } else if self.input.device() == InputDevice::Mouse && self.hovered_row(widget) == Some(row)
        {
            Some(ROW_OUTLINE)
        } else {
            None
        }
    }

    /// The tab of a tab bar under the mouse in this frame (TAB-BAR-001).
    #[must_use]
    pub(crate) fn hovered_tab(&self, widget: &Widget) -> Option<usize> {
        let WidgetKind::TabBar(bar) = &widget.kind else {
            return None;
        };
        if !self.is_hovered(widget) {
            return None;
        }
        bar.tab_at(widget.rect, f64::from(self.input.mouse().x))
    }

    /// The sprite of tab `tab` of a tab bar in this frame; a focused bar highlights its current
    /// tab (TAB-MENU-002, TAB-FOCUS-001).
    #[must_use]
    pub fn tab_sprite(&self, widget: &Widget, tab: usize) -> Option<TabSprite> {
        let WidgetKind::TabBar(bar) = &widget.kind else {
            return None;
        };
        bar.tabs().get(tab)?;
        let current = bar.current() == tab;
        let highlighted = match self.input.device() {
            InputDevice::Mouse => self.hovered_tab(widget) == Some(tab),
            InputDevice::Tab | InputDevice::Arrow | InputDevice::Activation => {
                current && self.is_focused(widget)
            }
            InputDevice::None => false,
        };
        Some(TabSprite {
            current,
            highlighted,
        })
    }

    /// The sprites of `widget` in this frame; a text widget draws none (WID-BTN-002,
    /// WID-ICON-002, WID-CHK-002).
    #[must_use]
    pub fn sprite(&self, widget: &Widget) -> Option<WidgetSprite> {
        let active = widget.active;
        let highlighted = self.is_highlighted(widget);
        match widget.kind {
            WidgetKind::Button | WidgetKind::IconButton { .. } => {
                Some(WidgetSprite::Button(ButtonSprite::of(active, highlighted)))
            }
            WidgetKind::ImageButton => Some(WidgetSprite::Image {
                active,
                highlighted,
            }),
            WidgetKind::CycleButton(_) => {
                Some(WidgetSprite::Button(ButtonSprite::of(active, highlighted)))
            }
            WidgetKind::Slider(slider) => {
                let keyboard = self.input.device().is_keyboard();
                let focused = self.is_focused(widget);
                let change_mode = focused && self.change_mode;
                Some(WidgetSprite::Slider {
                    bar_highlighted: active && focused && !change_mode && keyboard,
                    handle_highlighted: active
                        && (self.is_hovered(widget) || (change_mode && keyboard)),
                    handle_rect: slider.handle(widget.rect),
                })
            }
            WidgetKind::Checkbox { selected } => Some(WidgetSprite::Checkbox {
                selected,
                highlighted,
            }),
            WidgetKind::Text { .. }
            | WidgetKind::MultiLineText { .. }
            | WidgetKind::SelectionList(_)
            | WidgetKind::WidgetList(_)
            | WidgetKind::ScrollPanel(_)
            | WidgetKind::SectionLabel
            | WidgetKind::TabBar(_) => None,
            WidgetKind::EditBox(ref edit) => edit.border.then_some(WidgetSprite::TextField {
                highlighted: edit.editing || highlighted,
            }),
        }
    }

    /// The shape of the mouse pointer in this frame; the last hovered widget or scrollbar in the
    /// order of the draw sets it (WID-STATE-004, SCRL-BAR-001, EDIT-PTR-004).
    #[must_use]
    pub fn pointer_shape(&self) -> PointerShape {
        if matches!(self.drag, Drag::Scrollbar { .. }) {
            return PointerShape::ResizeVertical;
        }
        let mouse = self.input.mouse();
        let mut shape = PointerShape::Normal;
        for widget in &self.widgets {
            if let Some(area) = widget.kind.scroll_area(widget.rect) {
                if widget.visible && area.on_scrollbar(f64::from(mouse.x), f64::from(mouse.y)) {
                    shape = PointerShape::Hand;
                }
                continue;
            }
            if widget.kind.is_text() || !self.is_hovered(widget) {
                continue;
            }
            let dragged_slider = self.drag == Drag::Widget
                && self.is_focused(widget)
                && matches!(widget.kind, WidgetKind::Slider(_));
            shape = if let WidgetKind::EditBox(edit) = &widget.kind {
                if edit.editable {
                    PointerShape::Text
                } else {
                    PointerShape::NotAllowed
                }
            } else if !widget.active {
                PointerShape::NotAllowed
            } else if dragged_slider {
                PointerShape::ResizeHorizontal
            } else {
                PointerShape::Hand
            };
        }
        shape
    }

    /// The tooltip request of the widgets in the frame at `now_ms`; call it once in each frame,
    /// because it keeps the time at which each widget starts to want its tooltip (WID-TIP-001,
    /// WID-TIP-005, LIST-WID-001).
    pub fn tooltip(&mut self, now_ms: u64) -> Option<TooltipRequest> {
        let keyboard = self.input.device().is_keyboard();
        let mouse = self.input.mouse();
        let screen = self.screen;
        let mut request: Option<TooltipRequest> = None;
        for index in 0..self.widgets.len() {
            let Some(widget) = self.widgets.get(index) else {
                continue;
            };
            let placement = self.placement(widget);
            let hovered = self.is_hovered(widget);
            let bounds = match placement {
                WidgetPlacement::Area { shift, .. } => widget.rect.shifted_up(shift),
                WidgetPlacement::Screen | WidgetPlacement::Hidden => widget.rect,
            };
            let is_focused = self.focused == Some(index);
            let wants =
                placement != WidgetPlacement::Hidden && (hovered || (is_focused && keyboard));
            let Some(widget) = self.widgets.get_mut(index) else {
                continue;
            };
            let Some(tooltip) = widget.tooltip.filter(|_| widget.visible && wants) else {
                widget.tooltip_start_ms = None;
                continue;
            };
            let start = *widget.tooltip_start_ms.get_or_insert(now_ms);
            if now_ms.saturating_sub(start) <= u64::from(tooltip.delay_ms) {
                continue;
            }
            if is_focused || request.is_none() {
                request = Some(TooltipRequest {
                    widget: widget.id(),
                    bounds,
                    screen,
                    anchor: if hovered {
                        TooltipAnchor::Mouse(mouse)
                    } else {
                        TooltipAnchor::Focus
                    },
                });
            }
        }
        request
    }
}
