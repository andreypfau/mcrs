use alloc::vec::Vec;

use mcrs_minecraft_bedrock_wire::{BedrockDecode, BedrockEncode, Error, VarU32};

#[derive(Clone, Copy, Debug, PartialEq, BedrockEncode, BedrockDecode)]
pub struct GameRule<'a> {
    pub name: &'a str,
    pub editable: bool,
    pub value: GameRuleValue,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum GameRuleValue {
    Bool(bool),
    Int(u32),
    Float(f32),
}

impl BedrockEncode for GameRuleValue {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        match *self {
            Self::Bool(value) => {
                VarU32(1).encode(w)?;
                value.encode(w)
            }
            Self::Int(value) => {
                VarU32(2).encode(w)?;
                VarU32(value).encode(w)
            }
            Self::Float(value) => {
                VarU32(3).encode(w)?;
                value.encode(w)
            }
        }
    }
}

impl<'a> BedrockDecode<'a> for GameRuleValue {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        Ok(match VarU32::decode(r)? {
            VarU32(1) => Self::Bool(bool::decode(r)?),
            VarU32(2) => Self::Int(VarU32::decode(r)?.0),
            VarU32(3) => Self::Float(f32::decode(r)?),
            VarU32(other) => return Err(Error::UnknownEnumValue(i64::from(other))),
        })
    }
}
