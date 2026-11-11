use uuid::Uuid;

use crate::Xuid;

const MCRS_MARKER: u64 = 0x6d63_7273;
const LOW_62_BITS: u64 = (1 << 62) - 1;
const RFC_VARIANT: u64 = 0b10 << 62;
const VERSION_8: u64 = 0x8000;

/// How the UUID of a Bedrock player derives from its XUID.
#[derive(Copy, Clone, Debug, Default, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BedrockUuidScheme {
    /// `00000000-0000-0000-xxxx-xxxxxxxxxxxx`: data and proxies move between mcrs and a
    /// Geyser server without conversion. The version and variant are zero, so a strict
    /// UUID validator can reject it.
    #[default]
    FloodgateCompatible,
    /// A version 8 UUID: the `mcrs` marker and the two high XUID bits in the high half,
    /// the variant and the 62 low XUID bits in the low half.
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

    /// `None` for a UUID this scheme does not produce, which includes every Java v3 and v4 UUID.
    #[must_use]
    pub fn xuid(self, uuid: Uuid) -> Option<Xuid> {
        let (msb, lsb) = uuid.as_u64_pair();
        match self {
            // The nil UUID stands for no player, and Xbox Live issues no XUID 0.
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
