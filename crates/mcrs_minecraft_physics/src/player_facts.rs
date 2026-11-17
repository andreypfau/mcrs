#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlayerFacts {
    pub spectator: bool,
    pub passenger: bool,
    pub vehicle_takes_jump: bool,
    pub sleeping: bool,
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
            fed: true,
            sprint_window: 7,
        }
    }
}
