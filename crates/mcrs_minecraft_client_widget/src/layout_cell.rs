use crate::alignment::Alignment;
use crate::arithmetic;
use crate::gui_extent::GuiExtent;
use crate::gui_rect::GuiRect;
use crate::padding::Padding;

#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct LayoutCell {
    pub padding: Padding,
    pub alignment: Alignment,
}

impl LayoutCell {
    pub const CENTER: Self = Self {
        padding: Padding {
            left: 0,
            top: 0,
            right: 0,
            bottom: 0,
        },
        alignment: Alignment::CENTER,
    };

    #[must_use]
    pub fn place(self, size: GuiExtent, area: GuiRect) -> GuiRect {
        let padding = self.padding;
        let right = area
            .width
            .saturating_sub(size.width)
            .saturating_sub(padding.right);
        let bottom = area
            .height
            .saturating_sub(size.height)
            .saturating_sub(padding.bottom);
        GuiRect::new(
            area.x
                .saturating_add(arithmetic::lerp(self.alignment.x, padding.left, right) as i32),
            area.y.saturating_add(arithmetic::round(arithmetic::lerp(
                self.alignment.y,
                padding.top,
                bottom,
            ))),
            size.width,
            size.height,
        )
    }
}
