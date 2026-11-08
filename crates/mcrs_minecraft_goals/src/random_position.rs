use mcrs_minecraft_brain::BlockPos;
use mcrs_minecraft_brain::KeyedRandom;
use mcrs_minecraft_brain::Point;

use crate::CellFlags;
use crate::GoalWorld;

/// The horizontal and the vertical range of a random position search.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct SearchRange {
    pub horizontal: u32,
    pub vertical: u32,
}

impl SearchRange {
    #[must_use]
    pub const fn new(horizontal: u32, vertical: u32) -> Self {
        Self {
            horizontal,
            vertical,
        }
    }
}

const TRIES: u32 = 10;

/// The default search `D(h, v)` of GOAL-POS-002, for a mob with no home: the bottom centre of the
/// candidate with the largest walk value among 10 tries, the earlier try on a tie.
pub fn default_search<W: GoalWorld + ?Sized>(
    world: &W,
    feet: Point,
    range: SearchRange,
    random: &mut KeyedRandom,
) -> Option<Point> {
    best_of_tries(feet, range, random, |candidate| {
        let cell = world.cell(candidate);
        let valid = world.build_height().contains(candidate.y)
            && cell.flags.contains(CellFlags::STABLE_FLOOR)
            && cell.malus == 0.0;
        valid.then_some((candidate, cell.walk_value))
    })
}

/// The land search `L(h, v)` of GOAL-POS-003, for a mob with no home: as the default search, but a
/// candidate in a solid block moves up out of it, and water rejects it.
pub fn land_search<W: GoalWorld + ?Sized>(
    world: &W,
    feet: Point,
    range: SearchRange,
    random: &mut KeyedRandom,
) -> Option<Point> {
    let height = world.build_height();
    best_of_tries(feet, range, random, |mut candidate| {
        if !height.contains(candidate.y) {
            return None;
        }
        let mut cell = world.cell(candidate);
        if !cell.flags.contains(CellFlags::STABLE_FLOOR) {
            return None;
        }
        while cell.flags.contains(CellFlags::LEGACY_SOLID) && candidate.y <= height.max {
            candidate = candidate.offset(0, 1, 0);
            cell = world.cell(candidate);
        }
        let valid = !cell.flags.contains(CellFlags::WATER) && cell.malus == 0.0;
        valid.then_some((candidate, cell.walk_value))
    })
}

fn best_of_tries(
    feet: Point,
    range: SearchRange,
    random: &mut KeyedRandom,
    mut judge: impl FnMut(BlockPos) -> Option<(BlockPos, f32)>,
) -> Option<Point> {
    let mut best: Option<(BlockPos, f32)> = None;
    for _ in 0..TRIES {
        let a = random.symmetric(range.horizontal);
        let b = random.symmetric(range.vertical);
        let c = random.symmetric(range.horizontal);
        let candidate = Point::new(
            feet.x + f64::from(a),
            feet.y + f64::from(b),
            feet.z + f64::from(c),
        )
        .block();
        let Some((position, value)) = judge(candidate) else {
            continue;
        };
        if best.is_none_or(|(_, best_value)| value > best_value) {
            best = Some((position, value));
        }
    }
    best.map(|(position, _)| position.bottom_centre())
}
