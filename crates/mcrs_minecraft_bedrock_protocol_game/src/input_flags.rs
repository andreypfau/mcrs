#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct InputFlags(u64);

impl InputFlags {
    pub const ASCEND: Self = Self(1 << 0);

    pub const DESCEND: Self = Self(1 << 1);

    pub const NORTH_JUMP: Self = Self(1 << 2);

    pub const JUMP_KEY_DOWN: Self = Self(1 << 3);

    pub const SPRINT_KEY_DOWN: Self = Self(1 << 4);

    pub const CHANGE_HEIGHT: Self = Self(1 << 5);

    pub const JUMPING: Self = Self(1 << 6);

    pub const AUTO_JUMPING_IN_WATER: Self = Self(1 << 7);

    pub const SNEAKING: Self = Self(1 << 8);

    pub const SNEAK_KEY_DOWN: Self = Self(1 << 9);

    pub const UP: Self = Self(1 << 10);

    pub const DOWN: Self = Self(1 << 11);

    pub const LEFT: Self = Self(1 << 12);

    pub const RIGHT: Self = Self(1 << 13);

    pub const UP_AND_LEFT: Self = Self(1 << 14);

    pub const UP_AND_RIGHT: Self = Self(1 << 15);

    pub const WANT_UP: Self = Self(1 << 16);

    pub const WANT_DOWN: Self = Self(1 << 17);

    pub const WANT_DOWN_SLOW: Self = Self(1 << 18);

    pub const WANT_UP_SLOW: Self = Self(1 << 19);

    pub const SPRINTING: Self = Self(1 << 20);

    pub const ASCEND_BLOCK: Self = Self(1 << 21);

    pub const DESCEND_BLOCK: Self = Self(1 << 22);

    pub const SNEAK_TOGGLE_DOWN: Self = Self(1 << 23);

    pub const PERSIST_SNEAK: Self = Self(1 << 24);

    pub const START_SPRINTING: Self = Self(1 << 25);

    pub const STOP_SPRINTING: Self = Self(1 << 26);

    pub const START_SNEAKING: Self = Self(1 << 27);

    pub const STOP_SNEAKING: Self = Self(1 << 28);

    pub const START_SWIMMING: Self = Self(1 << 29);

    pub const STOP_SWIMMING: Self = Self(1 << 30);

    pub const START_JUMPING: Self = Self(1 << 31);

    pub const START_GLIDING: Self = Self(1 << 32);

    pub const STOP_GLIDING: Self = Self(1 << 33);

    pub const HANDLED_TELEPORT: Self = Self(1 << 37);

    pub const EMOTING: Self = Self(1 << 38);

    pub const MISSED_SWING: Self = Self(1 << 39);

    pub const START_CRAWLING: Self = Self(1 << 40);

    pub const STOP_CRAWLING: Self = Self(1 << 41);

    pub const START_FLYING: Self = Self(1 << 42);

    pub const STOP_FLYING: Self = Self(1 << 43);

    pub const CLIENT_ACKNOWLEDGED_SERVER_DATA: Self = Self(1 << 44);

    pub const PADDLING_LEFT: Self = Self(1 << 46);

    pub const PADDLING_RIGHT: Self = Self(1 << 47);

    pub const BLOCK_BREAKING_DELAY_ENABLED: Self = Self(1 << 48);

    pub const HORIZONTAL_COLLISION: Self = Self(1 << 49);

    pub const VERTICAL_COLLISION: Self = Self(1 << 50);

    pub const DOWN_AND_LEFT: Self = Self(1 << 51);

    pub const DOWN_AND_RIGHT: Self = Self(1 << 52);

    pub(crate) const ITEM_INTERACTION: u64 = 1 << 34;
    pub(crate) const BLOCK_ACTIONS: u64 = 1 << 35;
    pub(crate) const ITEM_STACK_REQUEST: u64 = 1 << 36;
    pub(crate) const CLIENT_PREDICTED_VEHICLE: u64 = 1 << 45;
    const PART_BITS: u64 = Self::ITEM_INTERACTION
        | Self::BLOCK_ACTIONS
        | Self::ITEM_STACK_REQUEST
        | Self::CLIENT_PREDICTED_VEHICLE;

    #[must_use]
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits & !Self::PART_BITS)
    }

    #[must_use]
    pub const fn bits(self) -> u64 {
        self.0
    }

    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
