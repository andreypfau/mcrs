use alloc::vec::Vec;

use crate::anchor::Anchor;
use crate::gui_point::GuiPoint;
use crate::input_device::InputDevice;
use crate::key_modifiers::KeyModifiers;
use crate::pointer_button::PointerButton;
use crate::pointer_press::PointerPress;
use crate::slider;
use crate::widget_event::WidgetEvent;
use crate::widget_kind::WidgetKind;
use crate::widget_placement::WidgetPlacement;
use crate::widgets::{Drag, Press, Widgets, activate, set_offset};
use crate::widgets_focus::FocusEntry;

const DOUBLE_CLICK_MS: u64 = 250;

#[derive(Clone, Copy, Debug, PartialEq)]
struct Click {
    x: f64,
    modifiers: KeyModifiers,
    now_ms: u64,
    double: bool,
}

impl Widgets {
    /// A move of the mouse to the mouse position `mouse` (WID-IN-001). A move with the left button
    /// down also calls [`Self::mouse_drag`].
    pub fn mouse_move(&mut self, mouse: GuiPoint) {
        self.input.move_mouse(mouse);
    }

    /// A press of a pointer button; `width` is the text width of the font (WID-PTR-002,
    /// WID-SLD-002, SCRL-BAR-003, LIST-SEL-002, LIST-WID-001, TAB-CLICK-001, EDIT-PTR-001,
    /// EDIT-PTR-002).
    pub fn mouse_press(
        &mut self,
        press: PointerPress,
        width: &impl Fn(&str) -> i32,
        events: &mut Vec<WidgetEvent>,
    ) {
        let PointerPress {
            x,
            y,
            button,
            modifiers,
            now_ms,
        } = press;
        self.input.set_device(InputDevice::Mouse);
        let double = self.last_press.is_some_and(|press| {
            press.button == button && now_ms.saturating_sub(press.at_ms) <= DOUBLE_CLICK_MS
        });
        self.last_press = Some(Press {
            button,
            at_ms: now_ms,
        });
        if button != PointerButton::Left {
            return;
        }
        let before = self.focused;
        let click = Click {
            x,
            modifiers,
            now_ms,
            double,
        };
        self.press(y, click, width, events);
        self.settle_focus(before, now_ms, FocusEntry::Edit);
    }

    fn press(
        &mut self,
        y: f64,
        click: Click,
        width: &impl Fn(&str) -> i32,
        events: &mut Vec<WidgetEvent>,
    ) {
        let x = click.x;
        let Some(index) = self.top_level_under(x, y) else {
            return;
        };
        let Some(widget) = self.widgets.get(index) else {
            return;
        };
        if let WidgetKind::TabBar(bar) = &widget.kind {
            if let Some(tab) = bar.tab_at(widget.rect, x).filter(|tab| bar.is_active(*tab)) {
                self.set_tab(index, tab, events);
                self.focus(Some(index), events);
            }
            return;
        }
        let Some(area) = widget.kind.scroll_area(widget.rect) else {
            self.press_widget(index, click, width, events);
            return;
        };
        if area.on_scrollbar(x, y) {
            self.focus(Some(index), events);
            self.drag = Drag::Scrollbar {
                area: index,
                last_y: y,
            };
            return;
        }
        if let WidgetKind::SelectionList(list) = &widget.kind {
            let id = widget.id();
            let Some(row) = list.row_under(widget.rect, x, y) else {
                return;
            };
            self.select(index, Some(row), events);
            self.focus(Some(index), events);
            if click.double {
                events.push(WidgetEvent::RowActivated { widget: id, row });
            }
            self.drag = Drag::Widget;
        } else if let Some(member) = self.member_under(index, x, y) {
            self.press_widget(member, click, width, events);
        }
    }

    /// A move with the left button down to the event position `x` at `now_ms`, which goes to
    /// the focused widget while the screen drags (WID-PTR-003, WID-SLD-002, SCRL-BAR-003,
    /// EDIT-PTR-003).
    pub fn mouse_drag(
        &mut self,
        x: f64,
        now_ms: u64,
        width: &impl Fn(&str) -> i32,
        events: &mut Vec<WidgetEvent>,
    ) {
        match self.drag {
            Drag::None => {}
            Drag::Widget => {
                let Some(widget) = self.focused.and_then(|index| self.widgets.get_mut(index))
                else {
                    return;
                };
                let (id, rect) = (widget.id(), widget.rect);
                match &mut widget.kind {
                    WidgetKind::Slider(slider) => {
                        events.extend(slider.move_to(id, slider::position_at(rect, x), now_ms));
                    }
                    WidgetKind::EditBox(edit) => edit.drag(rect, x, width),
                    _ => {}
                }
            }
            Drag::Scrollbar { area, last_y } => {
                // chisle: the scrollbar drag reads the integer mouse position of the last
                // mouse_move; an event y in mouse_drag lifts it.
                let y = f64::from(self.input.mouse().y);
                self.drag = Drag::Scrollbar { area, last_y: y };
                if let Some(widget) = self.widgets.get_mut(area)
                    && let Some(scroll) = widget.kind.scroll_area(widget.rect)
                {
                    set_offset(widget, scroll.dragged(y, y - last_y));
                }
            }
        }
    }

    /// A release of `button`; the left button ends the drag, and a slider that applies at once
    /// moves its handle to the position of its value (WID-PTR-003, WID-SLD-007).
    pub fn mouse_release(&mut self, button: PointerButton) {
        if button != PointerButton::Left {
            return;
        }
        if self.drag == Drag::Widget
            && let Some(widget) = self.focused.and_then(|index| self.widgets.get_mut(index))
            && let WidgetKind::Slider(slider) = &mut widget.kind
            && !slider.is_deferred()
        {
            slider.snap();
        }
        self.drag = Drag::None;
    }

    /// A wheel event with the amount down `down` at the event position (`x`, `y`)
    /// (WID-PTR-004, WID-CYC-002, SCRL-AREA-002).
    pub fn mouse_wheel(&mut self, x: f64, y: f64, down: f64, events: &mut Vec<WidgetEvent>) {
        self.input.set_device(InputDevice::Mouse);
        let Some(widget) = self
            .top_level_under(x, y)
            .and_then(|index| self.widgets.get_mut(index))
        else {
            return;
        };
        if let Some(scroll) = widget.kind.scroll_area(widget.rect) {
            set_offset(widget, scroll.wheel(down));
            return;
        }
        let id = widget.id();
        if let WidgetKind::CycleButton(cycle) = &mut widget.kind {
            if down > 0.0 {
                cycle.select_previous();
            } else if down < 0.0 {
                cycle.select_next();
            } else {
                return;
            }
            events.push(WidgetEvent::ValueChanged { widget: id });
        }
    }

    fn top_level_under(&self, x: f64, y: f64) -> Option<usize> {
        self.widgets
            .iter()
            .position(|widget| self.member(widget).is_none() && widget.is_under(x, y, 0))
    }

    fn member_under(&self, area: usize, x: f64, y: f64) -> Option<usize> {
        let area_widget = self.widgets.get(area)?;
        let row = match &area_widget.kind {
            WidgetKind::WidgetList(list) => Some(list.row_under(area_widget.rect, x, y)?),
            _ => None,
        };
        let shift = area_widget.kind.scroll_area(area_widget.rect)?.shift();
        self.members(area).find(|index| {
            self.widgets.get(*index).is_some_and(|widget| {
                row.is_none_or(|row| widget.area.is_some_and(|slot| slot.row == row))
                    && self.placement(widget) != WidgetPlacement::Hidden
                    && self.accepts(*index)
                    && widget.is_under(x, y, shift)
            })
        })
    }

    fn press_widget(
        &mut self,
        index: usize,
        click: Click,
        width: &impl Fn(&str) -> i32,
        events: &mut Vec<WidgetEvent>,
    ) {
        if let Some(widget) = self.widgets.get_mut(index) {
            let (id, rect) = (widget.id(), widget.rect);
            if let WidgetKind::Slider(slider) = &mut widget.kind {
                let position = slider::position_at(rect, click.x);
                events.extend(slider.move_to(id, position, click.now_ms));
            } else if let WidgetKind::EditBox(edit) = &mut widget.kind {
                if click.double {
                    edit.select_word(rect, click.x, width);
                } else {
                    let anchor = if click.modifiers.shift {
                        Anchor::Keep
                    } else {
                        Anchor::Move
                    };
                    edit.click(rect, click.x, anchor, width);
                }
            } else {
                activate(widget, click.modifiers, events);
            }
        }
        self.focus(Some(index), events);
        self.drag = Drag::Widget;
    }
}
