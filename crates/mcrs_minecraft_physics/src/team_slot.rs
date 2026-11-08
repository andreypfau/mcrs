/// The team of an entity and the collision rule of that team.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TeamSlot {
    /// The team, or `None` without one. Two entities of one team are allies.
    pub team: Option<u16>,
    pub rule: CollisionRule,
}

/// The collision rule of a team.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CollisionRule {
    #[default]
    Always,
    Never,
    PushOtherTeams,
    PushOwnTeam,
}
