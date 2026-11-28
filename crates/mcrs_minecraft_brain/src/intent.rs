use glam::DVec3;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct Ticket(pub u32);

impl Ticket {
    #[must_use]
    pub const fn next(self) -> Self {
        Self(self.0.wrapping_add(1))
    }
}

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

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathIntent {
    Walk {
        ticket: Ticket,
        target: DVec3,
        speed_modifier: f32,
    },
    Clear {
        ticket: Ticket,
    },
}

impl PathIntent {
    #[must_use]
    pub const fn ticket(&self) -> Ticket {
        match self {
            Self::Walk { ticket, .. } | Self::Clear { ticket } => *ticket,
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum LookIntent {
    At {
        point: DVec3,
        turn_speed: Option<f32>,
        pitch_limit: Option<f32>,
        hold: u8,
    },
    Around {
        cos: f32,
        sin: f32,
    },
    EndAround,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum Panic {
    #[default]
    Calm,
    Panicking,
}
