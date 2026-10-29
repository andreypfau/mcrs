use crate::{Decode, Encode, Result};
use std::io::Write;

/// For the first 8 values set means relative value while unset means absolute
#[derive(Debug, PartialEq, Eq, Hash, Copy, Clone)]
pub enum PositionFlag {
    X,
    Y,
    Z,
    YRot,
    XRot,
    DeltaX,
    DeltaY,
    DeltaZ,
    RotateDelta,
}

impl PositionFlag {
    const ALL: [PositionFlag; 9] = [
        PositionFlag::X,
        PositionFlag::Y,
        PositionFlag::Z,
        PositionFlag::YRot,
        PositionFlag::XRot,
        PositionFlag::DeltaX,
        PositionFlag::DeltaY,
        PositionFlag::DeltaZ,
        PositionFlag::RotateDelta,
    ];

    const fn mask(self) -> u16 {
        1 << self as u16
    }
}

/// The flags of a teleport, one bit each on the wire.
#[derive(Debug, PartialEq, Eq, Hash, Clone, Copy, Default)]
pub struct PositionFlags(u16);

impl PositionFlags {
    pub fn insert(&mut self, flag: PositionFlag) {
        self.0 |= flag.mask();
    }

    pub fn is_empty(self) -> bool {
        self.0 == 0
    }

    pub fn contains(self, flag: PositionFlag) -> bool {
        self.0 & flag.mask() != 0
    }

    pub fn iter(self) -> impl Iterator<Item = PositionFlag> {
        PositionFlag::ALL
            .into_iter()
            .filter(move |&flag| self.contains(flag))
    }
}

impl FromIterator<PositionFlag> for PositionFlags {
    fn from_iter<I: IntoIterator<Item = PositionFlag>>(flags: I) -> Self {
        let mut set = Self::default();
        for flag in flags {
            set.insert(flag);
        }
        set
    }
}

impl Encode for PositionFlags {
    fn encode(&self, w: impl Write) -> Result<()> {
        i32::from(self.0).encode(w)
    }
}

impl Decode<'_> for PositionFlags {
    fn decode(r: &mut &[u8]) -> Result<Self> {
        let bitfield = i32::decode(r)?;
        Ok(PositionFlag::ALL
            .into_iter()
            .filter(|&flag| bitfield & i32::from(flag.mask()) != 0)
            .collect())
    }
}
