bitflags::bitflags! {
    /// The flags of a grid row that the push reads.
    #[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
    pub struct GridFlags: u8 {
        /// Other entities can push the entity.
        const PUSHABLE = 1 << 0;
        const SPECTATOR = 1 << 1;
        const NO_PHYSICS = 1 << 2;
        const CARRIES_PASSENGER = 1 << 3;
        const IS_PASSENGER = 1 << 4;
        const PLAYER = 1 << 5;
    }
}
