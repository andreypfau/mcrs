use core::fmt;

use crate::attribute_keys::Attribute;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub struct AttributeId(pub i32);

impl AttributeId {
    #[must_use]
    pub fn resolve(self) -> Option<Attribute> {
        let number = u16::try_from(self.0).ok()?;
        Attribute::from_protocol_id(number)
    }
}

impl From<Attribute> for AttributeId {
    fn from(attribute: Attribute) -> Self {
        Self(i32::from(attribute.id().number()))
    }
}

impl fmt::Display for AttributeId {
    fn fmt(&self, formatter: &mut fmt::Formatter<'_>) -> fmt::Result {
        fmt::Display::fmt(&self.0, formatter)
    }
}

#[cfg(feature = "wire")]
impl mcrs_minecraft_wire::Encode for AttributeId {
    fn encode(&self, w: impl std::io::Write) -> mcrs_minecraft_wire::Result<()> {
        mcrs_minecraft_wire::VarInt(self.0).encode(w)
    }
}

#[cfg(feature = "wire")]
impl mcrs_minecraft_wire::Decode<'_> for AttributeId {
    fn decode(r: &mut &[u8]) -> mcrs_minecraft_wire::Result<Self> {
        let mcrs_minecraft_wire::VarInt(raw) = mcrs_minecraft_wire::VarInt::decode(r)?;
        Ok(Self(raw))
    }
}
