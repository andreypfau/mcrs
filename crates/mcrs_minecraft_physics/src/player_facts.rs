/// What the tick of a player reads besides its state, its keys and the world.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct PlayerFacts {
    pub spectator: bool,
    pub passenger: bool,
    /// The vehicle of a passenger takes the jump of its rider, as a horse does.
    pub vehicle_takes_jump: bool,
    pub sleeping: bool,
    /// The food level is more than 6.
    pub fed: bool,
    /// The sprint window of the client options, 7 by default.
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
