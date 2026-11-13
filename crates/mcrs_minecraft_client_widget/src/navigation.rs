use crate::gui_extent::GuiExtent;
use crate::gui_rect::GuiRect;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum TabDirection {
    Forward,
    Backward,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum ArrowDirection {
    Up,
    Down,
    Left,
    Right,
}

/// The order of Tab: by group, then by the widget of the screen, then by the row of a widget
/// list and the widget in it.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) struct TabKey {
    pub(crate) group: i32,
    pub(crate) owner: usize,
    pub(crate) row: usize,
    pub(crate) index: usize,
}

#[derive(Clone, Copy)]
struct Bounds {
    x: i64,
    y: i64,
    width: i64,
    height: i64,
}

#[derive(Clone, Copy)]
struct Point {
    x: i64,
    y: i64,
}

impl Bounds {
    fn right(self) -> i64 {
        self.x + self.width
    }

    fn bottom(self) -> i64 {
        self.y + self.height
    }

    fn centre_x(self) -> i64 {
        (self.x + self.right() - 1) / 2
    }

    fn centre_y(self) -> i64 {
        (self.y + self.bottom() - 1) / 2
    }
}

impl From<GuiRect> for Bounds {
    fn from(rect: GuiRect) -> Self {
        Self {
            x: i64::from(rect.x),
            y: i64::from(rect.y),
            width: i64::from(rect.width),
            height: i64::from(rect.height),
        }
    }
}

/// The stop that a Tab navigation from `current` focuses, with the wrap of WID-FOC-004 step 5.
pub(crate) fn tab_target(
    stops: impl Iterator<Item = (TabKey, usize)> + Clone,
    current: Option<TabKey>,
    direction: TabDirection,
) -> Option<usize> {
    let focusable = || stops.clone();
    let target = match direction {
        TabDirection::Forward => current
            .and_then(|current| focusable().filter(|(key, _)| *key > current).min())
            .or_else(|| focusable().min()),
        TabDirection::Backward => current
            .and_then(|current| focusable().filter(|(key, _)| *key < current).max())
            .or_else(|| focusable().max()),
    };
    target.map(|(_, index)| index)
}

/// The candidate that an arrow navigation from the bounds `from` focuses, with the fallback of
/// WID-FOC-006.
pub(crate) fn arrow_target(
    candidates: impl Iterator<Item = (usize, GuiRect)> + Clone,
    from: Option<GuiRect>,
    screen: GuiExtent,
    direction: ArrowDirection,
) -> Option<usize> {
    let strip = strip(from.map(Bounds::from), screen, direction);
    let others = || {
        candidates
            .clone()
            .map(|(index, rect)| (index, Bounds::from(rect)))
    };
    let nearest = others()
        .filter(|(_, bounds)| is_candidate(strip, *bounds, direction))
        .min_by_key(|(index, bounds)| (nearness(*bounds, direction), *index))
        .map(|(index, _)| index);
    nearest.or_else(|| {
        let start = strip_point(strip, direction);
        others()
            .filter_map(|(index, bounds)| {
                let near = near_point(bounds, direction);
                is_after(near, start, direction).then(|| (squared_distance(start, near), index))
            })
            .min()
            .map(|(_, index)| index)
    })
}

fn strip(focused: Option<Bounds>, screen: GuiExtent, direction: ArrowDirection) -> Bounds {
    let width = i64::from(screen.width);
    let height = i64::from(screen.height);
    let (x, y, strip_width, strip_height) = match (focused, direction) {
        (Some(bounds), ArrowDirection::Down) => (bounds.x, bounds.bottom(), bounds.width, 1),
        (Some(bounds), ArrowDirection::Up) => (bounds.x, bounds.y - 1, bounds.width, 1),
        (Some(bounds), ArrowDirection::Right) => (bounds.right(), bounds.y, 1, bounds.height),
        (Some(bounds), ArrowDirection::Left) => (bounds.x - 1, bounds.y, 1, bounds.height),
        (None, ArrowDirection::Down) => (0, -1, width, 1),
        (None, ArrowDirection::Up) => (0, height, width, 1),
        (None, ArrowDirection::Right) => (-1, 0, 1, height),
        (None, ArrowDirection::Left) => (width, 0, 1, height),
    };
    Bounds {
        x,
        y,
        width: strip_width,
        height: strip_height,
    }
}

fn is_candidate(strip: Bounds, bounds: Bounds, direction: ArrowDirection) -> bool {
    let across_x = bounds.x < strip.right() && strip.x < bounds.right();
    let across_y = bounds.y < strip.bottom() && strip.y < bounds.bottom();
    match direction {
        ArrowDirection::Down => {
            across_x && (bounds.y > strip.y || (bounds.y == strip.y && bounds.height >= 2))
        }
        ArrowDirection::Up => {
            across_x
                && (bounds.bottom() < strip.y + 1
                    || (bounds.bottom() == strip.y + 1 && bounds.y < strip.y))
        }
        ArrowDirection::Right => {
            across_y && (bounds.x > strip.x || (bounds.x == strip.x && bounds.width >= 2))
        }
        ArrowDirection::Left => {
            across_y
                && (bounds.right() < strip.x + 1
                    || (bounds.right() == strip.x + 1 && bounds.x < strip.x))
        }
    }
}

fn nearness(bounds: Bounds, direction: ArrowDirection) -> (i64, i64) {
    match direction {
        ArrowDirection::Down => (bounds.y, bounds.x),
        ArrowDirection::Up => (-bounds.bottom(), bounds.x),
        ArrowDirection::Right => (bounds.x, bounds.y),
        ArrowDirection::Left => (-bounds.right(), bounds.y),
    }
}

fn strip_point(strip: Bounds, direction: ArrowDirection) -> Point {
    match direction {
        ArrowDirection::Down | ArrowDirection::Up => Point {
            x: strip.centre_x(),
            y: strip.y,
        },
        ArrowDirection::Right | ArrowDirection::Left => Point {
            x: strip.x,
            y: strip.centre_y(),
        },
    }
}

fn near_point(bounds: Bounds, direction: ArrowDirection) -> Point {
    match direction {
        ArrowDirection::Down => Point {
            x: bounds.centre_x(),
            y: bounds.y,
        },
        ArrowDirection::Up => Point {
            x: bounds.centre_x(),
            y: bounds.bottom() - 1,
        },
        ArrowDirection::Right => Point {
            x: bounds.x,
            y: bounds.centre_y(),
        },
        ArrowDirection::Left => Point {
            x: bounds.right() - 1,
            y: bounds.centre_y(),
        },
    }
}

fn is_after(point: Point, start: Point, direction: ArrowDirection) -> bool {
    match direction {
        ArrowDirection::Down => point.y > start.y,
        ArrowDirection::Up => point.y < start.y,
        ArrowDirection::Right => point.x > start.x,
        ArrowDirection::Left => point.x < start.x,
    }
}

fn squared_distance(start: Point, point: Point) -> i128 {
    let across = i128::from(point.x - start.x);
    let down = i128::from(point.y - start.y);
    across * across + down * down
}
