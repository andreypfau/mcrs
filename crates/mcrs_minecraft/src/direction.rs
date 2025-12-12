use bevy_math::IVec3;
use bevy_reflect::Reflect;
use serde::{Deserialize, Serialize};
use std::fmt::Debug;
use std::ops::BitAndAssign;

#[derive(Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Direction {
    // https://bugs.mojang.com/browse/MC-274772
    #[serde(alias = "bottom")]
    Down,
    Up,
    North,
    South,
    West,
    East,
}

impl Direction {
    pub fn normal(&self) -> IVec3 {
        match self {
            Direction::Down => IVec3::NEG_Y,
            Direction::Up => IVec3::Y,
            Direction::North => IVec3::NEG_Z,
            Direction::South => IVec3::Z,
            Direction::West => IVec3::NEG_X,
            Direction::East => IVec3::X,
        }
    }

    pub fn id(&self) -> usize {
        match self {
            Direction::Down => 0,
            Direction::Up => 1,
            Direction::North => 2,
            Direction::South => 3,
            Direction::West => 4,
            Direction::East => 5,
        }
    }

    pub fn opposite(&self) -> Direction {
        match self {
            Direction::Down => Direction::Up,
            Direction::Up => Direction::Down,
            Direction::North => Direction::South,
            Direction::South => Direction::North,
            Direction::West => Direction::East,
            Direction::East => Direction::West,
        }
    }

    pub fn all() -> [Direction; 6] {
        [
            Direction::Down,
            Direction::Up,
            Direction::North,
            Direction::South,
            Direction::West,
            Direction::East,
        ]
    }
}

impl From<usize> for Direction {
    fn from(id: usize) -> Self {
        match id {
            0 => Direction::Down,
            1 => Direction::Up,
            2 => Direction::North,
            3 => Direction::South,
            4 => Direction::West,
            5 => Direction::East,
            _ => panic!("Invalid block direction id"),
        }
    }
}

impl From<Direction> for usize {
    #[inline]
    fn from(dir: Direction) -> Self {
        dir.id()
    }
}

impl From<IVec3> for Direction {
    fn from(vec: IVec3) -> Self {
        match vec {
            IVec3::NEG_Y => Direction::Down,
            IVec3::Y => Direction::Up,
            IVec3::NEG_Z => Direction::North,
            IVec3::Z => Direction::South,
            IVec3::NEG_X => Direction::West,
            IVec3::X => Direction::East,
            _ => panic!("Invalid block direction vector"),
        }
    }
}

impl From<Direction> for IVec3 {
    #[inline]
    fn from(dir: Direction) -> Self {
        dir.normal()
    }
}

#[derive(Default, Clone, Copy, PartialEq, Eq, Reflect)]
pub struct DirectionSet(u8);

impl Debug for DirectionSet {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut first = true;
        write!(f, "{:b}", self.0)?;
        write!(f, "[")?;
        for direction in Direction::all().iter() {
            if self.contains(*direction) {
                if !first {
                    write!(f, ", ")?;
                }
                write!(f, "{:?}", direction)?;
                first = false;
            }
        }
        write!(f, "]")
    }
}

impl DirectionSet {
    #[inline]
    pub fn all() -> Self {
        Self(0b111111)
    }

    #[inline]
    pub fn insert(&mut self, direction: Direction) {
        self.0 |= 1 << direction as u8;
    }

    #[inline]
    pub fn extend(&mut self, other: DirectionSet) {
        self.0 |= other.0;
    }

    #[inline]
    pub fn remove(&mut self, direction: Direction) {
        self.0 &= !(1 << direction as u8);
    }

    #[inline]
    pub fn contains(&self, direction: Direction) -> bool {
        self.0 & (1 << direction as u8) != 0
    }

    #[inline]
    pub fn is_empty(&self) -> bool {
        self.0 == 0
    }
}

impl IntoIterator for DirectionSet {
    type Item = Direction;
    type IntoIter = DirectionSetIter;

    fn into_iter(self) -> Self::IntoIter {
        DirectionSetIter(self, 0)
    }
}

pub struct DirectionSetIter(DirectionSet, usize);

impl Iterator for DirectionSetIter {
    type Item = Direction;

    fn next(&mut self) -> Option<Self::Item> {
        while self.1 < 6 {
            let direction = Direction::from(self.1);
            self.1 += 1;
            if self.0.contains(direction) {
                return Some(direction);
            }
        }
        None
    }
}

impl From<Direction> for DirectionSet {
    #[inline]
    fn from(direction: Direction) -> Self {
        Self(1 << direction as u8)
    }
}

impl BitAndAssign for DirectionSet {
    fn bitand_assign(&mut self, rhs: Self) {
        self.0 &= rhs.0;
    }
}
