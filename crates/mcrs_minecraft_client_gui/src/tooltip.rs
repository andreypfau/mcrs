use bevy::math::{IRect, IVec2};
use mcrs_minecraft_client_widget::{GuiExtent, TooltipRequest, WidgetId};

use crate::font::{self, Font, Span};
use crate::gui_sprite_id::GuiSpriteId;
use crate::scene::{GuiAtlasData, GuiQuad, GuiSize};

const LINE: i32 = 10;
const OFFSET: i32 = 12;
const FIRST_LINE_GAP: i32 = 2;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum LineGap {
    #[default]
    Uniform,
    AfterFirst,
}

#[derive(Debug, Default)]
pub struct Tooltip {
    lines: Vec<Vec<Span>>,
    at: IVec2,
    widget: Option<TooltipRequest>,
    gap: LineGap,
    requested: bool,
}

impl Tooltip {
    pub fn clear(&mut self) {
        self.requested = false;
    }

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
        self.widget = None;
        self.gap = LineGap::Uniform;
    }

    pub fn request_lines(&mut self, at: IVec2, lines: &[Vec<Span>]) {
        if self.requested || lines.is_empty() {
            return;
        }
        if self.lines != lines {
            self.lines.clear();
            self.lines.extend_from_slice(lines);
        }
        self.requested = true;
        self.at = at;
        self.widget = None;
        self.gap = LineGap::Uniform;
    }

    pub fn request_widget(
        &mut self,
        widget: TooltipRequest,
        focused: Option<WidgetId>,
        gap: LineGap,
        lines: impl FnOnce(&mut Vec<Vec<Span>>),
    ) {
        let replaces = focused == Some(widget.widget);
        if self.requested && !replaces {
            return;
        }
        if !self.requested {
            self.lines.clear();
        }
        let kept = self.lines.len();
        lines(&mut self.lines);
        if self.lines.len() == kept {
            return;
        }
        self.lines.drain(..kept);
        self.requested = true;
        self.widget = Some(widget);
        self.gap = gap;
    }

    pub fn request_widget_lines(
        &mut self,
        widget: TooltipRequest,
        focused: Option<WidgetId>,
        gap: LineGap,
        lines: &[Vec<Span>],
    ) {
        if lines.is_empty() || (self.requested && focused != Some(widget.widget)) {
            return;
        }
        if self.lines != lines {
            self.lines.clear();
            self.lines.extend_from_slice(lines);
        }
        self.requested = true;
        self.widget = Some(widget);
        self.gap = gap;
    }

    pub fn lines(&self) -> Option<&[Vec<Span>]> {
        self.requested.then_some(self.lines.as_slice())
    }
}

pub fn content_size(font: &Font, lines: &[Vec<Span>], gap: LineGap) -> IVec2 {
    let width = lines.iter().map(|line| font.width(line)).max().unwrap_or(0);
    let rows = LINE * lines.len() as i32;
    let height = match gap {
        LineGap::AfterFirst if lines.len() > 1 => rows,
        LineGap::AfterFirst | LineGap::Uniform => rows - 2,
    };
    IVec2::new(width, height)
}

fn line_top(index: usize, gap: LineGap) -> i32 {
    let shift = match gap {
        LineGap::AfterFirst if index > 0 => FIRST_LINE_GAP,
        LineGap::AfterFirst | LineGap::Uniform => 0,
    };
    LINE * index as i32 + shift
}

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

pub fn draw(tooltip: &Tooltip, gui: GuiSize, atlas: &GuiAtlasData, out: &mut Vec<GuiQuad>) {
    let Some(lines) = tooltip.lines() else {
        return;
    };
    let font = &atlas.font;
    let size = content_size(font, lines, tooltip.gap);
    let at = match tooltip.widget {
        Some(widget) => {
            let origin = widget.origin(GuiExtent::new(size.x, size.y));
            IVec2::new(origin.x, origin.y)
        }
        None => content_origin(tooltip.at, size, IVec2::new(gui.width, gui.height)),
    };
    let background = IRect::from_corners(at - OFFSET, at + size + OFFSET);
    for sprite in [GuiSpriteId::TooltipBackground, GuiSpriteId::TooltipFrame] {
        atlas.sprite(sprite).draw(background, out);
    }
    for (index, line) in lines.iter().enumerate() {
        font::draw_text(
            font,
            IVec2::new(at.x, at.y + line_top(index, tooltip.gap)),
            line,
            out,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_height_of_each_gap() {
        let font = Font::default();
        let rows = |count: usize| vec![Vec::new(); count];
        let heights = [
            (LineGap::Uniform, 1, 8),
            (LineGap::Uniform, 2, 18),
            (LineGap::AfterFirst, 1, 8),
            (LineGap::AfterFirst, 2, 20),
        ];
        for (gap, count, height) in heights {
            assert_eq!(content_size(&font, &rows(count), gap).y, height);
        }
    }

    #[test]
    fn the_first_request_with_lines_wins_until_the_frame_clears() {
        use crate::item_decorations::WHITE;
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
    fn only_the_request_of_the_focused_widget_replaces_the_first_request() {
        use crate::item_decorations::WHITE;
        use mcrs_minecraft_client_widget::{GuiRect, TooltipAnchor};
        fn line(text: &'static str) -> impl FnOnce(&mut Vec<Vec<Span>>) {
            move |rows| rows.push(vec![Span::new(text, WHITE)])
        }
        let request = |id| TooltipRequest {
            widget: WidgetId(id),
            bounds: GuiRect::new(0, 0, 20, 20),
            screen: GuiExtent::new(100, 100),
            anchor: TooltipAnchor::Focus,
        };
        let shown = |tooltip: &Tooltip| {
            tooltip
                .lines()
                .map(|lines| (lines.len(), lines[0][0].text.clone()))
        };
        let focused = Some(WidgetId(2));
        let mut tooltip = Tooltip::default();
        tooltip.request(IVec2::new(1, 1), line("hover"));
        tooltip.request_widget(
            request(1),
            focused,
            LineGap::Uniform,
            line("hovered widget"),
        );
        tooltip.request_widget(request(2), focused, LineGap::Uniform, |_| {});
        assert_eq!(shown(&tooltip), Some((1, "hover".into())));
        assert_eq!(tooltip.widget, None);
        tooltip.request_widget(
            request(2),
            focused,
            LineGap::Uniform,
            line("focused widget"),
        );
        assert_eq!(shown(&tooltip), Some((1, "focused widget".into())));
        assert_eq!(tooltip.widget, Some(request(2)));
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
