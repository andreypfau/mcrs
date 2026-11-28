#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlayerFacts {
    pub spectator: bool,
    pub passenger: bool,
    pub vehicle_takes_jump: bool,
    pub sleeping: bool,
    pub fall_flying: bool,
    pub spin_attack: bool,
    pub fed: bool,
    pub sprint_window: u8,
}

impl Default for PlayerFacts {
    fn default() -> Self {
        Self {
            spectator: false,
            passenger: false,
            vehicle_takes_jump: false,
            sleeping: false,
            fall_flying: false,
            spin_attack: false,
            fed: true,
            sprint_window: 7,
        }
    }
}
