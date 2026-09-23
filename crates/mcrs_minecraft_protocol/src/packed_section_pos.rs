use crate::{Decode, Encode};
use bitfield_struct::bitfield;
use derive_more::From;
use mcrs_minecraft_core::SectionPos;
use std::io::Write;
use thiserror::Error;

impl Encode for SectionPos {
    fn encode(&self, w: impl Write) -> anyhow::Result<()> {
        match PackedSectionPos::try_from(*self) {
            Ok(p) => p.encode(w),
            Err(e) => anyhow::bail!("{e}: {self}"),
        }
    }
}

impl<'a> Decode<'a> for SectionPos {
    fn decode(r: &mut &[u8]) -> anyhow::Result<Self> {
        PackedSectionPos::decode(r).map(Into::into)
    }
}

#[bitfield(u64)]
#[derive(PartialEq, Eq, Ord, PartialOrd, Encode, Decode)]
pub struct PackedSectionPos {
    #[bits(20)]
    pub y: i32,
    #[bits(22)]
    pub z: i32,
    #[bits(22)]
    pub x: i32,
}

impl From<PackedSectionPos> for SectionPos {
    fn from(pos: PackedSectionPos) -> Self {
        Self::new(pos.x(), pos.y(), pos.z())
    }
}

impl TryFrom<SectionPos> for PackedSectionPos {
    type Error = Error;

    fn try_from(pos: SectionPos) -> Result<Self, Self::Error> {
        match (pos.x, pos.y, pos.z) {
            (-2097152..=2097151, -524288..=524287, -2097152..=2097151) => {
                Ok(PackedSectionPos::new()
                    .with_x(pos.x)
                    .with_y(pos.y)
                    .with_z(pos.z))
            }
            _ => Err(Error(pos)),
        }
    }
}

#[derive(Copy, Clone, PartialEq, Eq, Debug, Error, From)]
#[error("packed section position of {0} is out of range")]
pub struct Error(pub SectionPos);
