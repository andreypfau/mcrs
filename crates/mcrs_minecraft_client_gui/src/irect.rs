use bevy::math::{IRect, IVec2};
use mcrs_minecraft_client_widget::{GuiPoint, GuiRect};

pub(crate) fn irect(rect: GuiRect) -> IRect {
    IRect::new(
        rect.x,
        rect.y,
        rect.x.saturating_add(rect.width),
        rect.y.saturating_add(rect.height),
    )
}

pub(crate) fn point(point: GuiPoint) -> IVec2 {
    IVec2::new(point.x, point.y)
}
