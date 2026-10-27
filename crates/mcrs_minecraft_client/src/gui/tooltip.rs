use std::collections::HashMap;

use bevy::math::{IRect, IVec2, Rect};
use serde::Deserialize;

use super::font::{Font, Span, draw_text};
use super::item_decorations::WHITE;
use super::scene::{GuiQuad, GuiSize};

const LINE: i32 = 10;
const OFFSET: i32 = 12;
const BACKGROUND: &str = "background";
const FRAME: &str = "frame";

#[derive(Deserialize)]
struct SpriteMeta {
    gui: GuiMeta,
}

#[derive(Deserialize)]
struct GuiMeta {
    scaling: Scaling,
}

#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Scaling {
    NineSlice {
        width: u32,
        height: u32,
        border: u32,
        #[serde(default)]
        stretch_inner: bool,
    },
}

/// A GUI sprite with `nine_slice` scaling, placed in the GUI atlas (TIP-BG-002).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct NineSlice {
    texels: Rect,
    size: IVec2,
    border: i32,
    stretch_inner: bool,
}

impl NineSlice {
    pub fn parse(mcmeta: &[u8], path: &str) -> Result<Self, String> {
        let meta: SpriteMeta =
            serde_json::from_slice(mcmeta).map_err(|error| format!("{path}: {error}"))?;
        let Scaling::NineSlice {
            width,
            height,
            border,
            stretch_inner,
        } = meta.gui.scaling;
        let int =
            |value: u32| i32::try_from(value).map_err(|_| format!("{path}: {value} is too large"));
        let size = IVec2::new(int(width)?, int(height)?);
        if size.min_element() == 0 {
            return Err(format!("{path}: a nine-slice sprite has no area"));
        }
        Ok(Self {
            texels: Rect::default(),
            size,
            border: int(border)?,
            stretch_inner,
        })
    }

    pub fn placed(self, texels: IRect) -> Self {
        Self {
            texels: texels.as_rect(),
            ..self
        }
    }

    pub fn draw(&self, target: IRect, out: &mut Vec<GuiQuad>) {
        let size = target.size();
        let border = IVec2::splat(self.border).min(size / 2);
        let spans = |axis: usize| {
            let (b, whole, sprite) = (border[axis], size[axis], self.size[axis]);
            [
                (0, b, 0, b),
                (b, whole - 2 * b, b, sprite - 2 * b),
                (whole - b, b, sprite - b, b),
            ]
        };
        for (column, (x, w, u, uw)) in spans(0).into_iter().enumerate() {
            for (row, (y, h, v, vh)) in spans(1).into_iter().enumerate() {
                let area = IRect::new(x, y, x + w, y + h);
                let part = IRect::new(u, v, u + uw, v + vh);
                let inner = column == 1 || row == 1;
                self.fill(
                    IRect::from_corners(target.min + area.min, target.min + area.max),
                    part,
                    inner && self.stretch_inner,
                    out,
                );
            }
        }
    }

    fn fill(&self, area: IRect, part: IRect, stretch: bool, out: &mut Vec<GuiQuad>) {
        if area.width() <= 0 || area.height() <= 0 || part.width() <= 0 || part.height() <= 0 {
            return;
        }
        if stretch {
            out.push(self.quad(area, part));
            return;
        }
        for y in (area.min.y..area.max.y).step_by(part.height() as usize) {
            for x in (area.min.x..area.max.x).step_by(part.width() as usize) {
                let tile = IVec2::new(area.max.x - x, area.max.y - y).min(part.size());
                out.push(self.quad(
                    IRect::from_corners(IVec2::new(x, y), IVec2::new(x, y) + tile),
                    IRect::from_corners(part.min, part.min + tile),
                ));
            }
        }
    }

    fn quad(&self, rect: IRect, part: IRect) -> GuiQuad {
        let scale = self.texels.size() / self.size.as_vec2();
        let texel = |at: IVec2| self.texels.min + at.as_vec2() * scale;
        GuiQuad::Texels {
            rect,
            texels: Rect::from_corners(texel(part.min), texel(part.max)),
            color: WHITE,
        }
    }
}

/// Which pair of sprites a file under `<namespace>/textures/gui/sprites/tooltip/` belongs to:
/// `None` for the default pair, else the tooltip style `namespace:prefix` (TIP-BG-001).
pub fn tooltip_sprite(id: &str) -> Option<(Option<String>, bool)> {
    let (namespace, name) = id.split_once(':')?;
    if namespace == "minecraft" && (name == BACKGROUND || name == FRAME) {
        return Some((None, name == FRAME));
    }
    let (prefix, layer) = name.rsplit_once('_')?;
    match layer {
        BACKGROUND | FRAME => Some((Some(format!("{namespace}:{prefix}")), layer == FRAME)),
        _ => None,
    }
}

#[derive(Clone, Copy, Debug, Default)]
pub struct TooltipPair {
    pub background: Option<NineSlice>,
    pub frame: Option<NineSlice>,
}

/// The background sprites of every tooltip style the resource pack has.
#[derive(Debug, Default)]
pub struct TooltipSprites {
    default: TooltipPair,
    styles: HashMap<String, TooltipPair>,
}

impl TooltipSprites {
    pub fn insert(&mut self, style: Option<String>, frame: bool, sprite: NineSlice) {
        let pair = match style {
            None => &mut self.default,
            Some(style) => self.styles.entry(style).or_default(),
        };
        if frame {
            pair.frame = Some(sprite);
        } else {
            pair.background = Some(sprite);
        }
    }

    pub fn get(&self, style: Option<&str>) -> TooltipPair {
        match style {
            None => self.default,
            Some(style) => self.styles.get(style).copied().unwrap_or_default(),
        }
    }
}

/// The tooltip request of the current frame (TIP-REQ-001). The buffers stay between frames.
#[derive(Debug, Default)]
pub struct Tooltip {
    lines: Vec<Vec<Span>>,
    at: IVec2,
    gap: bool,
    style: String,
    styled: bool,
    requested: bool,
}

impl Tooltip {
    pub fn clear(&mut self) {
        self.requested = false;
    }

    /// Makes a request at the mouse position `at`. `lines` writes its lines, and runs only when
    /// no request of this frame came first. A request with no lines has no effect.
    pub fn request(
        &mut self,
        at: IVec2,
        style: Option<&str>,
        gap: bool,
        lines: impl FnOnce(&mut Vec<Vec<Span>>),
    ) {
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
        self.gap = gap;
        self.style.clear();
        self.style.push_str(style.unwrap_or_default());
        self.styled = style.is_some();
    }

    pub fn lines(&self) -> Option<&[Vec<Span>]> {
        self.requested.then_some(self.lines.as_slice())
    }

    pub fn style(&self) -> Option<&str> {
        self.styled.then_some(self.style.as_str())
    }
}

/// The width and the height of the content box (TIP-BOX-001).
pub fn content_size(font: &Font, lines: &[Vec<Span>], gap: bool) -> IVec2 {
    let width = lines.iter().map(|line| font.width(line)).max().unwrap_or(0);
    let count = lines.len() as i32;
    let height = if gap && count > 1 {
        LINE * count
    } else {
        LINE * count - 2
    };
    IVec2::new(width, height)
}

/// The top left corner of the content box (TIP-POS-001).
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

/// The background, the outline and the lines of the request, if there is one (TIP-BG-001,
/// TIP-BOX-002).
pub fn draw(
    tooltip: &Tooltip,
    gui: GuiSize,
    font: &Font,
    sprites: &TooltipSprites,
    out: &mut Vec<GuiQuad>,
) {
    let Some(lines) = tooltip.lines() else {
        return;
    };
    let size = content_size(font, lines, tooltip.gap);
    let at = content_origin(tooltip.at, size, IVec2::new(gui.width, gui.height));
    let pair = sprites.get(tooltip.style());
    let background = IRect::from_corners(at - OFFSET, at + size + OFFSET);
    for sprite in [pair.background, pair.frame].into_iter().flatten() {
        sprite.draw(background, out);
    }
    let mut top = at.y;
    for (index, line) in lines.iter().enumerate() {
        let gap = if tooltip.gap && index == 1 { 2 } else { 0 };
        top += gap;
        draw_text(font, IVec2::new(at.x, top), line, out);
        top += LINE;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_first_request_with_lines_wins_until_the_frame_clears() {
        fn line(text: &'static str) -> impl FnOnce(&mut Vec<Vec<Span>>) {
            move |rows| rows.push(vec![Span::new(text, WHITE)])
        }
        let mut tooltip = Tooltip::default();
        tooltip.request(IVec2::new(1, 1), None, false, |_| {});
        tooltip.request(IVec2::new(2, 2), None, false, line("tag"));
        tooltip.request(IVec2::new(3, 3), Some("a:b"), false, line("suggestion"));
        tooltip.request(IVec2::new(4, 4), None, false, line("hover"));
        let shown = tooltip.lines().map(|lines| lines[0][0].text.clone());
        assert_eq!(
            (shown.as_deref(), tooltip.at, tooltip.style()),
            (Some("tag"), IVec2::new(2, 2), None)
        );
        tooltip.clear();
        assert!(tooltip.lines().is_none());
        tooltip.request(IVec2::new(5, 5), Some("a:b"), false, line("hover"));
        assert_eq!(tooltip.style(), Some("a:b"));
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
