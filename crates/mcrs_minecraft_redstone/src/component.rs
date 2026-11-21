use mcrs_minecraft_block::definition::schema::NoteBlockInstrument;
use mcrs_minecraft_core::{Axis, Direction};

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
pub enum Half {
    Lower,
    Upper,
}

impl Half {
    pub(crate) fn parse(text: &str) -> Option<Self> {
        match text {
            "lower" | "bottom" => Some(Self::Lower),
            "upper" | "top" => Some(Self::Upper),
            _ => None,
        }
    }

    #[must_use]
    pub fn other_half(self) -> Direction {
        match self {
            Self::Lower => Direction::Up,
            Self::Upper => Direction::Down,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailShape {
    Straight(Axis),
    Ascending(Direction),
    Curve,
}

impl RailShape {
    pub(crate) fn parse(text: &str) -> Option<Self> {
        match text {
            "north_south" => Some(Self::Straight(Axis::Z)),
            "east_west" => Some(Self::Straight(Axis::X)),
            "ascending_east" => Some(Self::Ascending(Direction::East)),
            "ascending_west" => Some(Self::Ascending(Direction::West)),
            "ascending_north" => Some(Self::Ascending(Direction::North)),
            "ascending_south" => Some(Self::Ascending(Direction::South)),
            "south_east" | "south_west" | "north_west" | "north_east" => Some(Self::Curve),
            _ => None,
        }
    }

    #[must_use]
    pub fn axis(self) -> Option<Axis> {
        match self {
            Self::Straight(axis) => Some(axis),
            Self::Ascending(direction) => Some(direction.axis()),
            Self::Curve => None,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Trigger {
    Dispenser,
    Dropper,
    Crafter,
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
    Door { wooden: bool },
    Trapdoor { wooden: bool },
    FenceGate,
    Rail { activator: bool },
    NoteBlock,
    Hopper,
    Trigger(Trigger),
    CopperBulb,
    TripwireHook,
    Tripwire,
    DaylightDetector,
}

#[must_use]
pub(crate) fn instrument_named(text: &str) -> Option<NoteBlockInstrument> {
    use NoteBlockInstrument::*;
    Some(match text {
        "harp" => Harp,
        "basedrum" => Basedrum,
        "snare" => Snare,
        "hat" => Hat,
        "bass" => Bass,
        "flute" => Flute,
        "bell" => Bell,
        "guitar" => Guitar,
        "chime" => Chime,
        "xylophone" => Xylophone,
        "iron_xylophone" => IronXylophone,
        "cow_bell" => CowBell,
        "didgeridoo" => Didgeridoo,
        "bit" => Bit,
        "banjo" => Banjo,
        "pling" => Pling,
        "trumpet" => Trumpet,
        "trumpet_exposed" => TrumpetExposed,
        "trumpet_oxidized" => TrumpetOxidized,
        "trumpet_weathered" => TrumpetWeathered,
        "zombie" => Zombie,
        "skeleton" => Skeleton,
        "creeper" => Creeper,
        "dragon" => Dragon,
        "wither_skeleton" => WitherSkeleton,
        "piglin" => Piglin,
        "custom_head" => CustomHead,
        _ => return None,
    })
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
