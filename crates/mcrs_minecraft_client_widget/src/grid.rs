use crate::axis::Axis;
use crate::grid_child::GridChild;
use crate::gui_extent::GuiExtent;
use crate::gui_rect::GuiRect;

/// Children in rows and columns: a row is as high as the largest share of a cell in it, a
/// column as wide as the largest share, and neighbour rows and columns are a gap apart.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Grid<'a> {
    pub row_gap: i32,
    pub column_gap: i32,
    pub children: &'a [GridChild],
}

impl<'a> Grid<'a> {
    /// The width and the height of the grid; 0 × 0 without a child.
    #[must_use]
    pub fn size(self) -> GuiExtent {
        GuiExtent::new(
            self.tracks(Axis::Horizontal).total(),
            self.tracks(Axis::Vertical).total(),
        )
    }

    /// The bounds of the children, in their order, for the grid at (`x`, `y`).
    pub fn rects(self, x: i32, y: i32) -> impl Iterator<Item = GuiRect> + 'a {
        let columns = self.tracks(Axis::Horizontal);
        let rows = self.tracks(Axis::Vertical);
        self.children.iter().map(move |child| {
            let area = GuiRect::new(
                x.saturating_add(columns.start(child.first_track(Axis::Horizontal))),
                y.saturating_add(rows.start(child.first_track(Axis::Vertical))),
                columns.covered_by(*child),
                rows.covered_by(*child),
            );
            child.child.cell.place(child.child.size, area)
        })
    }

    fn tracks(self, axis: Axis) -> Tracks<'a> {
        let gap = match axis {
            Axis::Horizontal => self.column_gap,
            Axis::Vertical => self.row_gap,
        };
        Tracks {
            children: self.children,
            axis,
            gap,
        }
    }
}

// chisle: a track length rescans the children, O(children² · tracks) per call; a caller-owned
// buffer of track lengths lifts it when a grid holds hundreds of children.
#[derive(Clone, Copy)]
struct Tracks<'a> {
    children: &'a [GridChild],
    axis: Axis,
    gap: i32,
}

impl Tracks<'_> {
    fn count(self) -> i32 {
        self.children
            .iter()
            .map(|child| {
                child
                    .first_track(self.axis)
                    .saturating_add(child.span(self.axis))
            })
            .max()
            .unwrap_or(0)
    }

    fn length(self, track: i32) -> i32 {
        self.children
            .iter()
            .filter_map(|child| child.share_in(self.axis, self.gap, track))
            .max()
            .unwrap_or(0)
    }

    fn lengths(self, first: i32, count: i32) -> i32 {
        (first..first.saturating_add(count))
            .map(|track| self.length(track))
            .fold(0, i32::saturating_add)
    }

    fn start(self, track: i32) -> i32 {
        self.lengths(0, track)
            .saturating_add(self.gap.saturating_mul(track))
    }

    fn covered_by(self, child: GridChild) -> i32 {
        let span = child.span(self.axis);
        self.lengths(child.first_track(self.axis), span)
            .saturating_add(self.gap.saturating_mul(span.saturating_sub(1)))
    }

    fn total(self) -> i32 {
        match self.count() {
            0 => 0,
            count => self
                .lengths(0, count)
                .saturating_add(self.gap.saturating_mul(count.saturating_sub(1))),
        }
    }
}
