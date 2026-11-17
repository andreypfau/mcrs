#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct TeamSlot {
    pub team: Option<u16>,
    pub rule: CollisionRule,
}

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub enum CollisionRule {
    #[default]
    Always,
    Never,
    PushOtherTeams,
    PushOwnTeam,
}
