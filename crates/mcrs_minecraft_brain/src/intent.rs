use glam::DVec3;

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
///
/// A channel comes with the first goal that writes it, so that a decision stays small. The wolf
/// adds the attack target (CGOAL-NEAR-001), which the hot tick revalidates at the application
/// (AIT-LAG-005), and the start and the stop of the melee and the ranged attack reflexes
/// (AIT-RFX-003). With them, the view gains the owner and the anger state (CGOAL-OWNER,
/// CGOAL-RESET-ANGER-001), and the damage record gains the attacker (CGOAL-HURT-001). A decision
/// writes only the state of its own mob (AIT-DEC-007), so a pair action (AIT-CPL-001) adds a
/// proposal channel that the merge point grants; it comes with the first pair action.
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
///
/// A path search is a decision and the follower is a reflex (PATH-DET-002), so a walk of a
/// searched path carries the nodes of the path to the host. The straight route has one node, its
/// end, which `target` holds; the node list comes with the search, beside the decision, so that
/// the intents stay small and `Copy`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathIntent {
    /// Walk to `target` at `speed_modifier` times the movement speed. The route is a straight line
    /// on walkable ground until a path search fills it.
    Walk {
        ticket: Ticket,
        target: DVec3,
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
    /// Turn the head to `point` for `hold` ticks after the application. A `None` speed or limit is
    /// the one of the mob type.
    At {
        point: DVec3,
        turn_speed: Option<f32>,
        pitch_limit: Option<f32>,
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
