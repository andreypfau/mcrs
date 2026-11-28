use bevy::math::IRect;

use crate::gui_quad::GuiQuad;

pub(crate) const BLACK: u32 = 0xFF00_0000;

pub(crate) fn draw_outlined_box(rect: IRect, outline: u32, out: &mut Vec<GuiQuad>) {
    let IRect { min, max } = rect;
    let fill = IRect::new(min.x.saturating_add(1), min.y, max.x, max.y);
    out.push(GuiQuad::Fill {
        rect: fill,
        color: BLACK,
    });
    let edges = [
        IRect::new(min.x, min.y, max.x, min.y.saturating_add(1)),
        IRect::new(min.x, max.y.saturating_sub(1), max.x, max.y),
        IRect::new(min.x, min.y, min.x.saturating_add(1), max.y),
        IRect::new(max.x.saturating_sub(1), min.y, max.x, max.y),
    ];
    for rect in edges {
        out.push(GuiQuad::Fill {
            rect,
            color: outline,
        });
    }
}
