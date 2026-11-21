use mcrs_minecraft_core::Direction;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Face {
    Floor,
    Wall,
    Ceiling,
}

impl Face {
    pub(crate) fn parse(text: &str) -> Option<Self> {
        match text {
            "floor" => Some(Self::Floor),
            "wall" => Some(Self::Wall),
            "ceiling" => Some(Self::Ceiling),
            _ => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DiodeMode {
    Compare,
    Subtract,
}

impl DiodeMode {
    pub(crate) fn parse(text: &str) -> Option<Self> {
        match text {
            "compare" => Some(Self::Compare),
            "subtract" => Some(Self::Subtract),
            _ => None,
        }
    }

    #[must_use]
    pub fn other(self) -> Self {
        match self {
            Self::Compare => Self::Subtract,
            Self::Subtract => Self::Compare,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Connection {
    Up,
    Side,
    None,
}

impl Connection {
    pub(crate) fn parse(text: &str) -> Option<Self> {
        match text {
            "up" => Some(Self::Up),
            "side" => Some(Self::Side),
            "none" => Some(Self::None),
            _ => None,
        }
    }

    #[must_use]
    pub fn is_connected(self) -> bool {
        self != Self::None
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum PlateKind {
    Wooden,
    Stone,
    LightWeighted,
    HeavyWeighted,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Kind {
    Wire,
    Lever,
    Button { wooden: bool },
    Plate(PlateKind),
    Torch { wall: bool },
    RedstoneBlock,
    Lamp,
    Repeater,
    Comparator,
    Observer,
    Piston { sticky: bool },
    PistonHead,
    MovingPiston,
    Target,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Stickiness {
    None,
    Slime,
    Honey,
}

impl Stickiness {
    #[must_use]
    pub fn is_sticky(self) -> bool {
        self != Self::None
    }

    #[must_use]
    pub fn sticks_to(self, other: Self) -> bool {
        !matches!(
            (self, other),
            (Self::Slime, Self::Honey) | (Self::Honey, Self::Slime)
        )
    }
}

#[must_use]
pub(crate) fn attached_direction(face: Face, facing: Direction) -> Direction {
    match face {
        Face::Floor => Direction::Down,
        Face::Ceiling => Direction::Up,
        Face::Wall => facing.opposite(),
    }
}

#[must_use]
pub(crate) fn left_of(direction: Direction) -> Direction {
    direction.clockwise().opposite()
}

#[must_use]
pub(crate) fn right_of(direction: Direction) -> Direction {
    direction.clockwise()
}
