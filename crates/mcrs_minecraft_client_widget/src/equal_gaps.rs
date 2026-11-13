use crate::arithmetic;
use crate::axis::Axis;
use crate::child::Child;
use crate::gui_extent::GuiExtent;
use crate::gui_rect::GuiRect;

/// Children in a row or a column of a fixed length, with the free length split evenly into the
/// gaps between neighbours.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct EqualGaps<'a> {
    pub(crate) axis: Axis,
    pub(crate) length: i32,
    pub(crate) min_cross: i32,
    pub(crate) children: &'a [Child],
}

impl<'a> EqualGaps<'a> {
    /// A row `width` wide and at least `min_height` high.
    #[must_use]
    pub const fn row(width: i32, min_height: i32, children: &'a [Child]) -> Self {
        Self {
            axis: Axis::Horizontal,
            length: width,
            min_cross: min_height,
            children,
        }
    }

    /// A column `height` high and at least `min_width` wide.
    #[must_use]
    pub const fn column(height: i32, min_width: i32, children: &'a [Child]) -> Self {
        Self {
            axis: Axis::Vertical,
            length: height,
            min_cross: min_width,
            children,
        }
    }

    /// The fixed length along the layout, and the largest of the minimum and the cells across it.
    #[must_use]
    pub fn size(self) -> GuiExtent {
        let cross = self.cross();
        match self.axis {
            Axis::Horizontal => GuiExtent::new(self.length, cross),
            Axis::Vertical => GuiExtent::new(cross, self.length),
        }
    }

    /// The bounds of the children, in order, for the layout at (`x`, `y`); the cells overlap when
    /// they are longer together than the layout.
    pub fn rects(self, x: i32, y: i32) -> impl Iterator<Item = GuiRect> + 'a {
        let axis = self.axis;
        let cross = self.cross();
        let free = self
            .length
            .saturating_sub(cell_lengths(self.children, axis));
        let gaps = i32::try_from(self.children.len().saturating_sub(1)).unwrap_or(i32::MAX);
        let mut start: i32 = 0;
        self.children
            .iter()
            .zip(1..=i32::MAX)
            .map(move |(child, gap)| {
                let length = child.cell_size().along(axis);
                let area = match axis {
                    Axis::Horizontal => GuiRect::new(x.saturating_add(start), y, length, cross),
                    Axis::Vertical => GuiRect::new(x, y.saturating_add(start), cross, length),
                };
                start = start
                    .saturating_add(length)
                    .saturating_add(arithmetic::even_split(free, gaps, gap));
                child.cell.place(child.size, area)
            })
    }

    fn cross(self) -> i32 {
        self.children
            .iter()
            .map(|child| child.cell_size().along(self.axis.cross()))
            .fold(self.min_cross, i32::max)
    }
}

pub(crate) fn cell_lengths(children: &[Child], axis: Axis) -> i32 {
    children
        .iter()
        .map(|child| child.cell_size().along(axis))
        .fold(0, i32::saturating_add)
}
