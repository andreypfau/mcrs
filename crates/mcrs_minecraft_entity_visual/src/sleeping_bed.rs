use bevy_ecs::component::Component;
use mcrs_minecraft_core::Direction;

#[derive(Component, Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct SleepingBed(pub Option<Direction>);

pub(crate) const fn bed_y_rotation(facing: Direction) -> Option<f32> {
    match facing {
        Direction::South => Some(90.0),
        Direction::West => Some(0.0),
        Direction::North => Some(270.0),
        Direction::East => Some(180.0),
        Direction::Down | Direction::Up => None,
    }
}
