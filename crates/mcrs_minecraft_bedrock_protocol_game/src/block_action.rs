use alloc::vec::Vec;

use mcrs_minecraft_bedrock_wire::{
    BedrockDecode, BedrockEncode, Error, NetworkSignedBlockPos, VarI32, decode_items,
};

/// One block action, the part of input flag 35 of [`PlayerAuthInput`](crate::PlayerAuthInput).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
#[non_exhaustive]
pub enum BlockAction {
    /// An action that carries a position and a face.
    Targeted {
        /// The action.
        action: TargetedBlockAction,
        /// The block.
        position: NetworkSignedBlockPos,
        /// The face of the block.
        face: VarI32,
    },
    /// An action that carries no position and no face.
    Untargeted(UntargetedBlockAction),
}

/// A block action that carries a position and a face.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum TargetedBlockAction {
    /// Action 0.
    StartDestroy,
    /// Action 1.
    AbortDestroy,
    /// Action 18.
    CrackBlock,
    /// Action 26.
    PredictDestroy,
    /// Action 27.
    ContinueDestroy,
}

/// A block action value that carries no position and no face: any value that is not a
/// [`TargetedBlockAction`].
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct UntargetedBlockAction(i32);

impl TargetedBlockAction {
    const fn wire_id(self) -> i32 {
        match self {
            Self::StartDestroy => 0,
            Self::AbortDestroy => 1,
            Self::CrackBlock => 18,
            Self::PredictDestroy => 26,
            Self::ContinueDestroy => 27,
        }
    }

    const fn from_wire_id(action: i32) -> Option<Self> {
        match action {
            0 => Some(Self::StartDestroy),
            1 => Some(Self::AbortDestroy),
            18 => Some(Self::CrackBlock),
            26 => Some(Self::PredictDestroy),
            27 => Some(Self::ContinueDestroy),
            _ => None,
        }
    }
}

impl UntargetedBlockAction {
    /// Action 2: stop destroy block.
    pub const STOP_DESTROY: Self = Self(2);

    /// The action, or `None` when the value is a [`TargetedBlockAction`].
    #[must_use]
    pub const fn new(action: i32) -> Option<Self> {
        match TargetedBlockAction::from_wire_id(action) {
            Some(_) => None,
            None => Some(Self(action)),
        }
    }

    /// The action value.
    #[must_use]
    pub const fn get(self) -> i32 {
        self.0
    }
}

impl BedrockEncode for BlockAction {
    fn encode(&self, w: &mut Vec<u8>) -> Result<(), Error> {
        match self {
            Self::Targeted {
                action,
                position,
                face,
            } => {
                VarI32(action.wire_id()).encode(w)?;
                position.encode(w)?;
                face.encode(w)
            }
            Self::Untargeted(action) => VarI32(action.get()).encode(w),
        }
    }
}

impl<'a> BedrockDecode<'a> for BlockAction {
    fn decode(r: &mut &'a [u8]) -> Result<Self, Error> {
        let VarI32(action) = VarI32::decode(r)?;
        Ok(match TargetedBlockAction::from_wire_id(action) {
            Some(action) => Self::Targeted {
                action,
                position: NetworkSignedBlockPos::decode(r)?,
                face: VarI32::decode(r)?,
            },
            None => Self::Untargeted(UntargetedBlockAction(action)),
        })
    }
}

pub(crate) fn encode_block_actions(actions: &[BlockAction], w: &mut Vec<u8>) -> Result<(), Error> {
    let count = i32::try_from(actions.len()).map_err(|_| Error::LengthOverflow)?;
    VarI32(count).encode(w)?;
    BlockAction::encode_slice(actions, w)
}

pub(crate) fn decode_block_actions(r: &mut &[u8]) -> Result<Vec<BlockAction>, Error> {
    let VarI32(count) = VarI32::decode(r)?;
    let count = usize::try_from(count).map_err(|_| Error::LengthOverflow)?;
    decode_items(count, r)
}
