use std::io::Write;

use alloc::vec::Vec;

use mcrs_minecraft_wire::Decode;
use mcrs_minecraft_wire::Encode;

use crate::ItemStack;
use crate::Kind;
use crate::NameLookup;
use crate::WireError;
use crate::wire::Entries;
use crate::wire::read_wire;
use crate::wire::read_wire_not_empty;
use crate::wire::write_wire;
use crate::wire_frame::WireFrame;
use crate::wire_frame::codec_error;

#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub struct WireStack(WireFrame);

impl WireStack {
    pub const EMPTY: Self = Self(WireFrame::EMPTY);

    pub fn write<L: NameLookup>(stack: Option<&ItemStack>, names: &L) -> Result<Self, WireError> {
        let mut bytes = Vec::new();
        write_wire(stack, names, &mut bytes)?;
        WireFrame::written(bytes).map(Self)
    }

    pub fn read<L: NameLookup>(&self, names: &L) -> Result<Option<ItemStack>, WireError> {
        self.0.read(|input| read_wire(input, names))
    }

    pub fn read_not_empty<L: NameLookup>(&self, names: &L) -> Result<ItemStack, WireError> {
        self.0.read(|input| read_wire_not_empty(input, names))
    }

    #[must_use]
    pub fn count(&self) -> i32 {
        self.0.count()
    }

    #[must_use]
    pub fn kind(&self) -> Option<Kind> {
        self.0.kind()
    }

    #[must_use]
    pub fn bytes(&self) -> &[u8] {
        self.0.bytes()
    }
}

impl Encode for WireStack {
    fn encode(&self, mut w: impl Write) -> mcrs_minecraft_wire::Result<()> {
        Ok(w.write_all(self.bytes())?)
    }
}

impl Decode<'_> for WireStack {
    fn decode(r: &mut &[u8]) -> mcrs_minecraft_wire::Result<Self> {
        WireFrame::scan(r, Entries::Bare)
            .map(Self)
            .map_err(codec_error)
    }
}
