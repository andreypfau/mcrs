use glam::DVec3;
use mcrs_minecraft_physics::Aabb;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Target {
    First,
    Second,
}

#[must_use]
pub fn merge_due(tick_count: u32, moved_block: bool) -> bool {
    let interval = if moved_block { 2 } else { 40 };
    tick_count.is_multiple_of(interval)
}

#[must_use]
pub fn merge_box(item: Aabb) -> Aabb {
    item.inflate(DVec3::new(0.5, 0.0, 0.5))
}

#[must_use]
pub fn merge_target(first: u8, second: u8) -> Target {
    if second > first {
        Target::Second
    } else {
        Target::First
    }
}
