//! The tooltip of a frame: one request, drawn over the rest of the GUI.

use bevy::math::{IRect, IVec2};

use super::font::{self, Font, Span};
use super::scene::{GuiAtlasData, GuiQuad, GuiSize};
use super::sprite::GuiSprite;

const LINE: i32 = 10;
const OFFSET: i32 = 12;

/// The background and the outline sprites, when the pack has them.
#[derive(Clone, Copy, Debug, Default)]
pub struct TooltipSprites {
    pub background: Option<GuiSprite>,
    pub frame: Option<GuiSprite>,
}

/// The tooltip request of the current frame. The buffers stay between frames.
#[derive(Debug, Default)]
pub struct Tooltip {
    lines: Vec<Vec<Span>>,
    at: IVec2,
    requested: bool,
}

impl Tooltip {
    /// Forgets the request of the previous frame.
    pub fn clear(&mut self) {
        self.requested = false;
    }

    /// Makes a request at the mouse position `at`. `lines` writes its lines, and runs only when
    /// no request of this frame came first. A request with no lines has no effect.
    pub fn request(&mut self, at: IVec2, lines: impl FnOnce(&mut Vec<Vec<Span>>)) {
        if self.requested {
            return;
        }
        self.lines.clear();
        lines(&mut self.lines);
        if self.lines.is_empty() {
            return;
        }
        self.requested = true;
        self.at = at;
    }

    /// The lines of the request of this frame, if there is one.
    pub fn lines(&self) -> Option<&[Vec<Span>]> {
        self.requested.then_some(self.lines.as_slice())
    }
}

// chisle: no gap after the first line; only an item tooltip sets that flag, and none exists.
/// The width and the height of the content box.
pub fn content_size(font: &Font, lines: &[Vec<Span>]) -> IVec2 {
    let width = lines.iter().map(|line| font.width(line)).max().unwrap_or(0);
    IVec2::new(width, LINE * lines.len() as i32 - 2)
}

/// The top left corner of the content box.
pub fn content_origin(mouse: IVec2, size: IVec2, gui: IVec2) -> IVec2 {
    let mut at = mouse + IVec2::new(OFFSET, -OFFSET);
    if at.x + size.x > gui.x {
        at.x = (at.x - 2 * OFFSET - size.x).max(4);
    }
    if at.y + size.y + 3 > gui.y {
        at.y = gui.y - size.y - 3;
    }
    at
}

// chisle: always the default sprites; only an item's `tooltip_style` selects others, and item
// tooltips do not exist yet.
/// The background, the outline and the lines of the request, if there is one. A sprite the pack
/// lacks draws as the missing sprite.
pub fn draw(tooltip: &Tooltip, gui: GuiSize, atlas: &GuiAtlasData, out: &mut Vec<GuiQuad>) {
    let Some(lines) = tooltip.lines() else {
        return;
    };
    let font = &atlas.font;
    let size = content_size(font, lines);
    let at = content_origin(tooltip.at, size, IVec2::new(gui.width, gui.height));
    let background = IRect::from_corners(at - OFFSET, at + size + OFFSET);
    for sprite in [atlas.tooltip.background, atlas.tooltip.frame] {
        sprite.unwrap_or(atlas.missing).draw(background, out);
    }
    for (index, line) in lines.iter().enumerate() {
        font::draw_text(
            font,
            IVec2::new(at.x, at.y + LINE * index as i32),
            line,
            out,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_request_with_lines_wins_until_the_frame_clears() {
        use super::super::item_decorations::WHITE;
        fn line(text: &'static str) -> impl FnOnce(&mut Vec<Vec<Span>>) {
            move |rows| rows.push(vec![Span::new(text, WHITE)])
        }
        let shown = |tooltip: &Tooltip| tooltip.lines().map(|lines| lines[0][0].text.clone());
        let mut tooltip = Tooltip::default();
        tooltip.request(IVec2::new(1, 1), |_| {});
        tooltip.request(IVec2::new(2, 2), line("tag"));
        tooltip.request(IVec2::new(3, 3), line("suggestion"));
        tooltip.request(IVec2::new(4, 4), line("hover"));
        assert_eq!(
            (shown(&tooltip).as_deref(), tooltip.at),
            (Some("tag"), IVec2::new(2, 2))
        );
        tooltip.clear();
        assert!(tooltip.lines().is_none());
        tooltip.request(IVec2::new(5, 5), line("hover"));
        assert_eq!(shown(&tooltip).as_deref(), Some("hover"));
    }

    #[test]
    fn the_box_flips_left_at_the_right_edge_and_rises_at_the_bottom() {
        let gui = IVec2::new(427, 240);
        for (mouse, size, origin) in [
            ((100, 100), (50, 8), (112, 88)),
            ((400, 100), (50, 8), (338, 88)),
            ((10, 100), (420, 8), (4, 88)),
            ((100, 235), (50, 28), (112, 209)),
            ((100, 5), (50, 8), (112, -7)),
        ] {
            assert_eq!(
                content_origin(IVec2::from(mouse), IVec2::from(size), gui),
                IVec2::from(origin),
                "mouse {mouse:?}, size {size:?}"
            );
        }
    }
}
