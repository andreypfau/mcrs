use core::str::FromStr;

use crate::UnknownOperation;

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[cfg_attr(
    feature = "serde",
    derive(serde::Serialize, serde::Deserialize),
    serde(rename_all = "snake_case")
)]
pub enum ModifierOperation {
    AddValue = 0,
    AddMultipliedBase = 1,
    AddMultipliedTotal = 2,
}

impl ModifierOperation {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::AddValue => "add_value",
            Self::AddMultipliedBase => "add_multiplied_base",
            Self::AddMultipliedTotal => "add_multiplied_total",
        }
    }
}

impl From<ModifierOperation> for u8 {
    fn from(operation: ModifierOperation) -> Self {
        operation as u8
    }
}

impl TryFrom<u8> for ModifierOperation {
    type Error = UnknownOperation;

    fn try_from(id: u8) -> Result<Self, Self::Error> {
        match id {
            0 => Ok(Self::AddValue),
            1 => Ok(Self::AddMultipliedBase),
            2 => Ok(Self::AddMultipliedTotal),
            _ => Err(UnknownOperation),
        }
    }
}

impl FromStr for ModifierOperation {
    type Err = UnknownOperation;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "add_value" => Ok(Self::AddValue),
            "add_multiplied_base" => Ok(Self::AddMultipliedBase),
            "add_multiplied_total" => Ok(Self::AddMultipliedTotal),
            _ => Err(UnknownOperation),
        }
    }
}

#[cfg(feature = "wire")]
impl mcrs_minecraft_wire::Encode for ModifierOperation {
    fn encode(&self, w: impl std::io::Write) -> mcrs_minecraft_wire::Result<()> {
        mcrs_minecraft_wire::VarInt(i32::from(u8::from(*self))).encode(w)
    }
}

#[cfg(feature = "wire")]
impl mcrs_minecraft_wire::Decode<'_> for ModifierOperation {
    fn decode(r: &mut &[u8]) -> mcrs_minecraft_wire::Result<Self> {
        use mcrs_minecraft_wire::Context;
        use mcrs_minecraft_wire::ErrorKind;
        use mcrs_minecraft_wire::VarInt;

        let tag = VarInt::decode(r)
            .context("failed to decode enum discriminant in `ModifierOperation`")?
            .0;
        u8::try_from(tag)
            .ok()
            .and_then(|id| Self::try_from(id).ok())
            .ok_or_else(|| {
                ErrorKind::UnknownTag {
                    ty: "ModifierOperation",
                    tag,
                }
                .into()
            })
    }
}
