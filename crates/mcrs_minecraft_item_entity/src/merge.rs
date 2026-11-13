use glam::DVec3;
use mcrs_minecraft_physics::Aabb;

/// The partner of a merge that takes the items of the other (IEN-MRG-005).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    First,
    Second,
}

/// True when an item entity looks for merge partners in this tick: when its tick count is a
/// multiple of 2 after a change of its block position in the tick, else of 40 (IEN-MRG-001).
#[must_use]
pub fn merge_due(tick_count: u32, moved_block: bool) -> bool {
    let interval = if moved_block { 2 } else { 40 };
    tick_count.is_multiple_of(interval)
}

/// The box whose strict intersection with the box of another item entity makes it a partner of
/// the item entity with the box `item` (IEN-MRG-003).
#[must_use]
pub fn merge_box(item: Aabb) -> Aabb {
    item.inflate(DVec3::new(0.5, 0.0, 0.5))
}

/// The partner with the larger count. Equal counts give the first; the specification leaves that
/// case open (IEN-MRG-005).
#[must_use]
pub fn merge_target(first: u8, second: u8) -> Target {
    if second > first {
        Target::Second
    } else {
        Target::First
    }
}
