use crate::child::Child;
use crate::frame::Frame;
use crate::gui_extent::GuiExtent;
use crate::gui_rect::GuiRect;
use crate::standard_sizes::{CONTENT_TOP_MARGIN, FOOTER_HEIGHT, HEADER_HEIGHT};

/// A screen in three frames as wide as the screen: the header at the top, the footer at the
/// bottom, and the content a margin below the header or as low as the footer lets it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct HeaderContentFooter<'a> {
    pub header_height: i32,
    pub footer_height: i32,
    pub content_top_margin: i32,
    pub header: &'a [Child],
    pub content: &'a [Child],
    pub footer: &'a [Child],
}

impl<'a> HeaderContentFooter<'a> {
    /// The bounds of the children of the header, then of the content, then of the footer, on a
    /// screen of the size `screen`.
    pub fn rects(self, screen: GuiExtent) -> impl Iterator<Item = GuiRect> + 'a {
        let header = Frame {
            min_size: GuiExtent::new(screen.width, self.header_height),
            children: self.header,
        };
        let content = Frame {
            min_size: GuiExtent::new(screen.width, 0),
            children: self.content,
        };
        let footer = Frame {
            min_size: GuiExtent::new(screen.width, self.footer_height),
            children: self.footer,
        };
        let footer_top = screen.height.saturating_sub(self.footer_height);
        let content_top = self
            .header_height
            .saturating_add(self.content_top_margin)
            .min(footer_top.saturating_sub(content.size().height));
        header
            .rects(0, 0)
            .chain(content.rects(0, content_top))
            .chain(footer.rects(0, footer_top))
    }
}

impl Default for HeaderContentFooter<'_> {
    fn default() -> Self {
        Self {
            header_height: HEADER_HEIGHT,
            footer_height: FOOTER_HEIGHT,
            content_top_margin: CONTENT_TOP_MARGIN,
            header: &[],
            content: &[],
            footer: &[],
        }
    }
}
