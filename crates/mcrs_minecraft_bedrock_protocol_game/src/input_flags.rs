/// The input flags of [`PlayerAuthInput`](crate::PlayerAuthInput) that add no field.
///
/// Bits 34, 35, 36 and 45 are never held here: the packet derives them from its optional parts.
/// A bit above 52 is kept.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash)]
pub struct InputFlags(u64);

impl InputFlags {
    /// Bit 0: ascend.
    pub const ASCEND: Self = Self(1 << 0);

    /// Bit 1: descend.
    pub const DESCEND: Self = Self(1 << 1);

    /// Bit 2: north jump (no longer used).
    pub const NORTH_JUMP: Self = Self(1 << 2);

    /// Bit 3: jump key down.
    pub const JUMP_KEY_DOWN: Self = Self(1 << 3);

    /// Bit 4: sprint key down.
    pub const SPRINT_KEY_DOWN: Self = Self(1 << 4);

    /// Bit 5: change height.
    pub const CHANGE_HEIGHT: Self = Self(1 << 5);

    /// Bit 6: jumping.
    pub const JUMPING: Self = Self(1 << 6);

    /// Bit 7: auto jumping in water.
    pub const AUTO_JUMPING_IN_WATER: Self = Self(1 << 7);

    /// Bit 8: sneaking.
    pub const SNEAKING: Self = Self(1 << 8);

    /// Bit 9: sneak key down.
    pub const SNEAK_KEY_DOWN: Self = Self(1 << 9);

    /// Bit 10: up.
    pub const UP: Self = Self(1 << 10);

    /// Bit 11: down.
    pub const DOWN: Self = Self(1 << 11);

    /// Bit 12: left.
    pub const LEFT: Self = Self(1 << 12);

    /// Bit 13: right.
    pub const RIGHT: Self = Self(1 << 13);

    /// Bit 14: up and left.
    pub const UP_AND_LEFT: Self = Self(1 << 14);

    /// Bit 15: up and right.
    pub const UP_AND_RIGHT: Self = Self(1 << 15);

    /// Bit 16: want up.
    pub const WANT_UP: Self = Self(1 << 16);

    /// Bit 17: want down.
    pub const WANT_DOWN: Self = Self(1 << 17);

    /// Bit 18: want down slow.
    pub const WANT_DOWN_SLOW: Self = Self(1 << 18);

    /// Bit 19: want up slow.
    pub const WANT_UP_SLOW: Self = Self(1 << 19);

    /// Bit 20: sprinting.
    pub const SPRINTING: Self = Self(1 << 20);

    /// Bit 21: ascend block.
    pub const ASCEND_BLOCK: Self = Self(1 << 21);

    /// Bit 22: descend block.
    pub const DESCEND_BLOCK: Self = Self(1 << 22);

    /// Bit 23: sneak toggle down.
    pub const SNEAK_TOGGLE_DOWN: Self = Self(1 << 23);

    /// Bit 24: persist sneak.
    pub const PERSIST_SNEAK: Self = Self(1 << 24);

    /// Bit 25: start sprinting.
    pub const START_SPRINTING: Self = Self(1 << 25);

    /// Bit 26: stop sprinting.
    pub const STOP_SPRINTING: Self = Self(1 << 26);

    /// Bit 27: start sneaking.
    pub const START_SNEAKING: Self = Self(1 << 27);

    /// Bit 28: stop sneaking.
    pub const STOP_SNEAKING: Self = Self(1 << 28);

    /// Bit 29: start swimming.
    pub const START_SWIMMING: Self = Self(1 << 29);

    /// Bit 30: stop swimming.
    pub const STOP_SWIMMING: Self = Self(1 << 30);

    /// Bit 31: start jumping.
    pub const START_JUMPING: Self = Self(1 << 31);

    /// Bit 32: start gliding.
    pub const START_GLIDING: Self = Self(1 << 32);

    /// Bit 33: stop gliding.
    pub const STOP_GLIDING: Self = Self(1 << 33);

    /// Bit 37: handled teleport.
    pub const HANDLED_TELEPORT: Self = Self(1 << 37);

    /// Bit 38: emoting.
    pub const EMOTING: Self = Self(1 << 38);

    /// Bit 39: missed swing.
    pub const MISSED_SWING: Self = Self(1 << 39);

    /// Bit 40: start crawling.
    pub const START_CRAWLING: Self = Self(1 << 40);

    /// Bit 41: stop crawling.
    pub const STOP_CRAWLING: Self = Self(1 << 41);

    /// Bit 42: start flying.
    pub const START_FLYING: Self = Self(1 << 42);

    /// Bit 43: stop flying.
    pub const STOP_FLYING: Self = Self(1 << 43);

    /// Bit 44: client acknowledged server data.
    pub const CLIENT_ACKNOWLEDGED_SERVER_DATA: Self = Self(1 << 44);

    /// Bit 46: paddling left.
    pub const PADDLING_LEFT: Self = Self(1 << 46);

    /// Bit 47: paddling right.
    pub const PADDLING_RIGHT: Self = Self(1 << 47);

    /// Bit 48: block breaking delay enabled.
    pub const BLOCK_BREAKING_DELAY_ENABLED: Self = Self(1 << 48);

    /// Bit 49: horizontal collision.
    pub const HORIZONTAL_COLLISION: Self = Self(1 << 49);

    /// Bit 50: vertical collision.
    pub const VERTICAL_COLLISION: Self = Self(1 << 50);

    /// Bit 51: down and left.
    pub const DOWN_AND_LEFT: Self = Self(1 << 51);

    /// Bit 52: down and right.
    pub const DOWN_AND_RIGHT: Self = Self(1 << 52);

    pub(crate) const ITEM_INTERACTION: u64 = 1 << 34;
    pub(crate) const BLOCK_ACTIONS: u64 = 1 << 35;
    pub(crate) const ITEM_STACK_REQUEST: u64 = 1 << 36;
    pub(crate) const CLIENT_PREDICTED_VEHICLE: u64 = 1 << 45;
    const PART_BITS: u64 = Self::ITEM_INTERACTION
        | Self::BLOCK_ACTIONS
        | Self::ITEM_STACK_REQUEST
        | Self::CLIENT_PREDICTED_VEHICLE;

    /// The flags of `bits` without bits 34, 35, 36 and 45.
    #[must_use]
    pub const fn from_bits(bits: u64) -> Self {
        Self(bits & !Self::PART_BITS)
    }

    /// The bits of the flags.
    #[must_use]
    pub const fn bits(self) -> u64 {
        self.0
    }

    /// Whether each flag of `other` is set in `self`.
    #[must_use]
    pub const fn contains(self, other: Self) -> bool {
        self.0 & other.0 == other.0
    }
}
