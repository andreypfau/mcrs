use uuid::Uuid;

use crate::Xuid;

const MCRS_MARKER: u64 = 0x6d63_7273;
const LOW_62_BITS: u64 = (1 << 62) - 1;
const RFC_VARIANT: u64 = 0b10 << 62;
const VERSION_8: u64 = 0x8000;

#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BedrockUuidScheme {
    #[default]
    FloodgateCompatible,
    Rfc9562,
}

impl BedrockUuidScheme {
    #[must_use]
    pub fn to_uuid(self, xuid: Xuid) -> Uuid {
        match self {
            Self::FloodgateCompatible => Uuid::from_u64_pair(0, xuid.0),
            Self::Rfc9562 => Uuid::from_u64_pair(
                MCRS_MARKER << 32 | VERSION_8 | xuid.0 >> 62,
                RFC_VARIANT | xuid.0 & LOW_62_BITS,
            ),
        }
    }

    #[must_use]
    pub fn xuid(self, uuid: Uuid) -> Option<Xuid> {
        let (msb, lsb) = uuid.as_u64_pair();
        match self {
            Self::FloodgateCompatible => (msb == 0 && lsb != 0).then_some(Xuid(lsb)),
            Self::Rfc9562 => (msb & !0b11 == MCRS_MARKER << 32 | VERSION_8
                && lsb & !LOW_62_BITS == RFC_VARIANT)
                .then_some(Xuid((msb & 0b11) << 62 | lsb & LOW_62_BITS)),
        }
    }

    #[must_use]
    pub fn detect(uuid: Uuid) -> Option<(Xuid, Self)> {
        [Self::FloodgateCompatible, Self::Rfc9562]
            .into_iter()
            .find_map(|scheme| scheme.xuid(uuid).map(|xuid| (xuid, scheme)))
    }
}
