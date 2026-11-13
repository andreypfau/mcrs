#[derive(Copy, Clone, Debug, PartialEq, Eq, Hash, Default)]
#[repr(u8)]
pub enum Reliability {
    #[default]
    Unreliable = 0,
    UnreliableSequenced = 1,
    Reliable = 2,
    ReliableOrdered = 3,
    ReliableSequenced = 4,
    UnreliableWithAckReceipt = 5,
    ReliableWithAckReceipt = 6,
    ReliableOrderedWithAckReceipt = 7,
}

impl Reliability {
    #[inline]
    #[must_use]
    pub const fn from_u8(val: u8) -> Self {
        match val & 0x07 {
            0 => Self::Unreliable,
            1 => Self::UnreliableSequenced,
            2 => Self::Reliable,
            3 => Self::ReliableOrdered,
            4 => Self::ReliableSequenced,
            5 => Self::UnreliableWithAckReceipt,
            6 => Self::ReliableWithAckReceipt,
            _ => Self::ReliableOrderedWithAckReceipt,
        }
    }

    #[inline]
    #[must_use]
    pub const fn is_reliable(self) -> bool {
        matches!(
            self,
            Self::Reliable
                | Self::ReliableOrdered
                | Self::ReliableSequenced
                | Self::ReliableWithAckReceipt
                | Self::ReliableOrderedWithAckReceipt
        )
    }

    #[inline]
    #[must_use]
    pub const fn is_ordered(self) -> bool {
        matches!(
            self,
            Self::ReliableOrdered | Self::ReliableOrderedWithAckReceipt
        )
    }

    #[inline]
    #[must_use]
    pub const fn is_sequenced(self) -> bool {
        matches!(self, Self::UnreliableSequenced | Self::ReliableSequenced)
    }

    #[inline]
    #[must_use]
    pub const fn has_ack_receipt(self) -> bool {
        matches!(
            self,
            Self::UnreliableWithAckReceipt
                | Self::ReliableWithAckReceipt
                | Self::ReliableOrderedWithAckReceipt
        )
    }
}
