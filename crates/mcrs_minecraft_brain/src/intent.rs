use crate::Point;

/// The number of a path intent of one mob. The host reports the ticket of the last intent that it
/// applied, so the brain can tell a pending intent from an applied one.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ticket(pub u32);

impl Ticket {
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

/// The guidance changes of one decision. Each channel holds the last write of the goals in the
/// evaluation; an empty channel keeps the guidance as it is.
#[derive(Clone, Copy, Debug, Default, PartialEq)]
pub struct Intents {
    pub path: Option<PathIntent>,
    pub look: Option<LookIntent>,
    pub panic: Option<Panic>,
}

impl Intents {
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.path.is_none() && self.look.is_none() && self.panic.is_none()
    }
}

/// A change of the path of the mob. The host reports the ticket of the last path intent that it
/// applied, and a dropped intent as applied and ended.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathIntent {
    /// Walk to `target` at `speed_modifier` times the movement speed. The route is a straight line
    /// on walkable ground until a path search fills it.
    Walk {
        ticket: Ticket,
        target: Point,
        speed_modifier: f32,
    },
    /// The mob has no path: the request found none, or a goal cleared it.
    Clear { ticket: Ticket },
}

impl PathIntent {
    #[must_use]
    pub const fn ticket(&self) -> Ticket {
        match self {
            Self::Walk { ticket, .. } | Self::Clear { ticket } => *ticket,
        }
    }
}

/// A change of the look target of the mob.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LookIntent {
    /// Turn the head to `point` for `hold` ticks after the application.
    At {
        point: Point,
        turn_speed: f32,
        pitch_limit: f32,
        hold: u8,
    },
    /// From the application on, in each tick, look at the point one block from the eye in the
    /// horizontal direction `(cos, sin)`, until [`LookIntent::EndAround`] or another look intent.
    Around { cos: f32, sin: f32 },
    /// Stop the look-around.
    EndAround,
}

/// The in-panic state that the snapshot publishes for the other mobs.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Panic {
    #[default]
    Calm,
    Panicking,
}
