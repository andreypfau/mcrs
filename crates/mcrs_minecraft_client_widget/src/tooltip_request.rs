use crate::gui_extent::GuiExtent;
use crate::gui_point::GuiPoint;
use crate::gui_rect::GuiRect;
use crate::saturate;
use crate::widget_id::WidgetId;

/// The tooltip that a widget asks for in a frame (WID-TIP-001, WID-TIP-005).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TooltipRequest {
    pub widget: WidgetId,
    pub bounds: GuiRect,
    pub screen: GuiExtent,
    pub anchor: TooltipAnchor,
}

/// What places the tooltip of a widget.
#[non_exhaustive]
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum TooltipAnchor {
    /// The mouse position over the hovered widget (WID-TIP-003).
    Mouse(GuiPoint),
    /// The bounds of the focused widget (WID-TIP-004).
    Focus,
}

impl TooltipRequest {
    /// The top left corner of a content box of the size `content`.
    #[must_use]
    pub fn origin(&self, content: GuiExtent) -> GuiPoint {
        let width = i64::from(content.width);
        let height = i64::from(content.height);
        let screen_width = i64::from(self.screen.width);
        let screen_height = i64::from(self.screen.height);
        let left = i64::from(self.bounds.x);
        let top = i64::from(self.bounds.y);
        let bounds_width = i64::from(self.bounds.width);
        let bounds_height = i64::from(self.bounds.height);
        let (x, y) = match self.anchor {
            TooltipAnchor::Mouse(mouse) => {
                let mouse_x = i64::from(mouse.x);
                let mut x = mouse_x + 12;
                if x + width > screen_width - 5 {
                    x = (mouse_x - 12 - width).max(9);
                }
                let y = i64::from(mouse.y) + 3;
                let offset = |edge: i64| {
                    let near = (y - edge).abs().min(bounds_height) as f64;
                    let rows = bounds_height as f64;
                    let first = (bounds_height - 3) as f64;
                    saturate::round(first + near / rows * (5.0 - first))
                };
                if top + 2 * bounds_height + height + 6 <= screen_height - 5 {
                    (x, y + offset(top))
                } else {
                    (x, y - (height + 6 + offset(top + bounds_height)))
                }
            }
            TooltipAnchor::Focus => {
                let mut x = left + 3;
                let mut y = top + bounds_height + 4;
                if y + height + 3 > screen_height {
                    y = top - height - 4;
                }
                if x + width > screen_width {
                    x = (left + bounds_width - width - 3).max(4);
                }
                (x, y)
            }
        };
        GuiPoint::new(saturate::saturate(x), saturate::saturate(y))
    }
}
